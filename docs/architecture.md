# A2VM — Apple II Emulator Architecture

## Overview

Rust 后端（CPU/Memory/Bus 核心模拟） + Swift 前端（macOS 原生 UI）混合架构。

```
┌─────────────────────────────────┐
│  Swift / SwiftUI macOS App      │
│  ┌───────────┐  ┌────────────┐  │
│  │ MTKView   │  │ AudioUnit  │  │
│  └─────▲─────┘  └─────▲──────┘  │
│        │              │          │
│  ══════╪══════════════╪═══════  │
│        │   C FFI      │          │
│  ┌─────┴──────────────┴──────┐  │
│  │      a2vm-core (Rust)     │  │
│  │  ┌─────┐ ┌────┐ ┌─────┐  │  │
│  │  │ CPU │─│ Bus│─│ Mem │  │  │
│  │  └─────┘ └──┬─┘ └─────┘  │  │
│  │        ┌────┴────┐        │  │
│  │        │ Devices │        │  │
│  │        └─────────┘        │  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

## 项目结构

```
a2vm/
├── Cargo.toml                    # workspace root
├── docs/                         # 设计文档
├── a2vm-core/                    # Rust 核心库
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs                # crate root
│   │   ├── bus.rs                # Bus trait
│   │   ├── cpu/
│   │   │   ├── mod.rs            # Cpu struct, step(), execute()
│   │   │   ├── opcodes.rs        # 256-entry opcode lookup table
│   │   │   ├── addressing.rs     # 13 种寻址模式解析
│   │   │   └── status.rs         # Status register (P) 位操作
│   │   ├── memory.rs             # FlatMemory + AppleIIMemory
│   │   ├── keyboard.rs           # 键盘锁存/选通
│   │   ├── softswitch.rs         # $C000-$C0FF 软开关
│   │   ├── video.rs              # 视频渲染
│   │   ├── audio.rs              # 扬声器
│   │   ├── disk.rs               # Disk II
│   │   ├── machine.rs            # AppleII struct, impl Bus
│   │   └── ffi.rs                # extern "C" FFI for Swift
│   └── tests/
│       ├── data/                 # test binaries
│       └── klaus_dormann.rs      # CPU functional test
└── a2vm-app/                     # Swift/SwiftUI macOS app (后续)
```

## 核心模块设计

### Bus Trait

CPU 通过 Bus 与所有外部设备通信。`read` 使用 `&mut self` 因为硬件读取有副作用（如 $C030 翻转扬声器）。

```rust
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn read_word(&mut self, addr: u16) -> u16;
    fn read_word_page_wrap(&mut self, addr: u16) -> u16;  // JMP indirect bug
}
```

### CPU

```rust
pub struct Cpu {
    pub a: u8, pub x: u8, pub y: u8,
    pub sp: u8, pub pc: u16, pub p: Status,
    pub cycles: u64,
    pub irq_pending: bool, pub nmi_pending: bool,
}
```

关键方法：
- `step(bus) -> u32` — 执行一条指令，返回消耗周期
- `run(bus, target_cycles) -> u64` — 运行至少 N 个周期
- `reset(bus)` — 从 $FFFC 读取 PC

**CPU 提取模式**：`AppleII` 同时拥有 CPU 和实现 Bus。执行时用 `std::mem::replace` 临时取出 CPU，避免借用冲突，零开销且无 unsafe。

### Status Register

P 寄存器位操作封装：

| Bit | Flag | 说明 |
|-----|------|------|
| 0 | C | Carry |
| 1 | Z | Zero |
| 2 | I | Interrupt Disable |
| 3 | D | Decimal (BCD) |
| 4 | B | Break (仅在 push 时有意义) |
| 5 | U | Unused (始终 1) |
| 6 | V | Overflow |
| 7 | N | Negative |

### Addressing Modes

13 种 NMOS 6502 寻址模式：

| 模式 | 语法 | 说明 |
|------|------|------|
| Implied | `CLC` | 无操作数 |
| Accumulator | `ASL A` | 操作 A 寄存器 |
| Immediate | `LDA #$FF` | 立即数 |
| ZeroPage | `LDA $00` | 零页地址 |
| ZeroPageX | `LDA $00,X` | 零页 + X (wrap) |
| ZeroPageY | `LDX $00,Y` | 零页 + Y (wrap) |
| Absolute | `LDA $1234` | 16-bit 绝对地址 |
| AbsoluteX | `LDA $1234,X` | 绝对 + X (page penalty) |
| AbsoluteY | `LDA $1234,Y` | 绝对 + Y (page penalty) |
| Indirect | `JMP ($1234)` | 间接跳转 (page wrap bug) |
| IndirectX | `LDA ($00,X)` | (零页+X) 间接 |
| IndirectY | `LDA ($00),Y` | 零页间接 + Y (page penalty) |
| Relative | `BEQ label` | 分支偏移 (-128 to +127) |

### Opcode Table

256 项静态数组，每项包含：
- `mnemonic: Mnemonic` — 指令助记符 (ADC, AND, ... TYA, ILL)
- `mode: AddrMode` — 寻址模式
- `cycles: u32` — 基础周期数
- `page_penalty: bool` — 跨页是否额外 +1 周期

### Memory

**FlatMemory**: 纯 64K 平坦 RAM，用于独立 CPU 测试。

**AppleIIMemory** (后续):
- `$0000-$BFFF`: 48K 主 RAM
- `$C000-$C0FF`: 软开关 I/O
- `$C100-$C7FF`: Slot ROM
- `$C800-$CFFF`: Expansion ROM
- `$D000-$FFFF`: ROM / Language Card RAM

### Machine (AppleII)

`AppleII` 结构体拥有所有硬件部件并实现 `Bus` trait：

| 地址范围 | 路由目标 |
|---------|---------|
| `$0000-$BFFF` | main_ram |
| `$C000-$C00F` | keyboard latch |
| `$C010` | keyboard strobe clear |
| `$C030` | speaker toggle |
| `$C050-$C057` | 显示模式软开关 |
| `$C080-$C08F` | Language Card bank switching |
| `$C100-$C7FF` | Slot ROM |
| `$D000-$FFFF` | ROM / Language Card RAM |

### FFI (Swift 接口)

通过 cbindgen 自动生成 C header，核心 API：

```c
// 生命周期
A2VM* a2vm_create(void);
void  a2vm_destroy(A2VM* handle);
void  a2vm_reset(A2VM* handle);

// 每帧
A2VMFrameResult a2vm_run_frame(A2VM* handle);

// 输入
void a2vm_key_down(A2VM* handle, uint8_t key);
void a2vm_key_up(A2VM* handle);

// 加载
int32_t a2vm_load_rom(A2VM* handle, const char* path);
int32_t a2vm_load_disk(A2VM* handle, uint8_t slot, uint8_t drive, const char* path);

// 调试
A2VMCpuState a2vm_get_cpu_state(A2VM* handle);
uint8_t a2vm_peek(A2VM* handle, uint16_t addr);
uint32_t a2vm_step(A2VM* handle);
```

## 数据流（一帧）

```
Swift: a2vm_run_frame(handle)
  → Rust: AppleII::run_frame()
    1. 取出 CPU (mem::replace)
    2. cpu.run(self, 17030 cycles)
       └─ 循环: fetch → resolve → execute → bus.read/write
    3. 放回 CPU
    4. render framebuffer (RGBA)
    5. generate audio (44.1kHz PCM)
    6. 返回 FrameResult { framebuffer_ptr, audio_ptr }
  ← Swift: blit to MTKView + enqueue to AudioUnit
```

## 已知 6502 陷阱

- **BCD**: NMOS 6502 的 N/Z 标志基于二进制结果而非 BCD 修正结果
- **JMP ($xxFF)**: 高字节从 $xx00 读取（page wrap bug）
- **BRK**: 推入 PC+2（跳过 padding byte），推入 P 时 B=1
- **RTI vs RTS**: RTI 恢复精确 PC；RTS 恢复 PC-1 再 +1
- **PHP**: 总是推入 B=1, U=1
- **Overflow (V)**: ADC/SBC 溢出检测最容易实现错
