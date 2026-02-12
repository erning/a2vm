# A2VM 代码审查报告

**审查日期**: 2026-02-12  
**审查人**: GLM-5 (Zhipu AI)  
**项目**: A2VM - Apple II/II+ 模拟器

---

## 1. 项目概述

A2VM 是一个使用 Rust 编写的 Apple II/II+ 模拟器，采用 workspace 结构组织代码：

| Crate | 行数 | 职责 |
|-------|------|------|
| `a2vm-core` | ~2,800 | 核心 CPU、总线、磁盘、视频、音频模拟 |
| `a2vm-oxide` | ~170 | 共享资源（嵌入式 ROM、机械噪音） |
| `a2vm-tui` | ~460 | 终端界面（Braille 显示） |
| `a2vm-gui` | ~540 | 图形界面（pixels + winit） |

---

## 2. 架构评价

### 2.1 优点

#### 清晰的模块分离
```
a2vm-core/
├── cpu/          # 6502 CPU 实现
│   ├── mod.rs    # CPU 主逻辑
│   ├── opcodes.rs # 操作码表
│   ├── status.rs  # 状态寄存器
│   ├── addressing.rs # 寻址模式
│   └── tests.rs   # CPU 单元测试
├── bus.rs        # Bus trait 定义
├── machine.rs    # AppleII 整机
├── disk.rs       # Disk II 控制器
├── video.rs      # 视频渲染
├── audio.rs      # 扬声器音频
└── keyboard.rs   # 键盘映射
```

#### Bus Trait 设计（优秀）
```rust
// bus.rs
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn peek(&self, addr: u16) -> u8 { ... }  // 无副作用读取
    fn read_word_page_wrap(&mut self, addr: u16) -> u16 { ... }  // 6502 页面回绕 bug
}
```

这个设计非常优雅：
- `read/write` 可能有副作用（如 $C030 扬声器切换）
- `peek` 用于调试器/状态显示，无副作用
- `read_word_page_wrap` 正确实现了 NMOS 6502 的 JMP indirect bug

#### CPU-Bus 分离模式
```rust
// machine.rs
pub struct AppleII {
    pub cpu: Cpu,
    pub bus: BusState,  // 分离避免临时提取模式
}
```
允许 CPU 和 Bus 的同时可变借用，避免了 `mem::take` 模式。

### 2.2 改进建议

#### 考虑引入 Domain Events
当前 `BusState` 直接暴露内部状态：
```rust
pub struct BusState {
    pub display: DisplayMode,
    pub disk: DiskII,
    // ...
}
```
建议封装为只读访问器或事件通知机制，减少跨模块耦合。

---

## 3. CPU 实现评价

### 3.1 优点

#### 完整的操作码支持
支持全部 56 个官方操作码 + 8 个非法操作码（LAX, SAX, DCP, ISC, SLO, RLA, RRA, SRE）。

#### 正确的 BCD 算术
```rust
// mod.rs:807-840
fn adc_bcd(&mut self, val: u8) {
    // NMOS 6502: V 基于中间结果（低位修正后，高位修正前）
    let intermediate = ((hi << 4) | (lo & 0x0F)) as u8;
    let overflow = (!(self.a ^ val) & (self.a ^ intermediate)) & 0x80 != 0;
    
    // NMOS 6502: N 和 Z 基于二进制结果，而非 BCD 结果
    self.p.set(N, bin_sum as u8 & 0x80 != 0);
    self.p.set(Z, (bin_sum as u8) == 0);
}
```
正确实现了 NMOS 6502 BCD 模式的标志位行为。

#### 内联优化
```rust
#[inline(always)]
pub fn a(&self) -> u8 { self.a }
```
热路径访问器正确使用了 `#[inline(always)]`。

### 3.2 潜在问题

#### unreachable! 使用
```rust
// mod.rs:359
fn read_operand<B: Bus>(&self, resolved: &Resolved, bus: &mut B) -> u8 {
    match resolved.operand {
        Operand::Address(addr) => bus.read(addr),
        _ => {
            unreachable!("read_operand expected address operand")
        }
    }
}
```
**建议**: 使用 `debug_assert!` 配合默认返回值，或使用 `Option` 类型。

---

## 4. 磁盘模拟评价

### 4.1 优点

#### 完整的 Nibblization 实现
```rust
// disk.rs
fn nibblize_sector(buf: &mut Vec<u8>, track: u8, sector: u8, data: &[u8]) {
    // 4-and-4 编码地址字段
    encode_4and4(buf, volume);
    // 6-and-2 编码数据字段
    encode_6and2(buf, data);
}
```
正确实现了 DOS 3.3 的 6-and-2 GCR 编码。

#### 编译时构建反向查找表
```rust
const REVERSE_TABLE: [u8; 256] = build_reverse_table();

const fn build_reverse_table() -> [u8; 256] {
    let mut table = [0xFFu8; 256];
    let mut i = 0;
    while i < 64 {
        table[WRITE_TABLE[i] as usize] = i as u8;
        i += 1;
    }
    table
}
```
零运行时开销的查找表构建。

### 4.2 改进建议

#### Stepper Motor 边界处理
```rust
// disk.rs:279
let next = (self.half_track as i16 + delta).clamp(0, 69);
```
`half_track` 最大值 69 对应 35 磁道，这是正确的。但建议添加常量：
```rust
const MAX_HALF_TRACK: u8 = 69;
```

---

## 5. 视频渲染评价

### 5.1 优点

#### 统一的渲染管线
```rust
// video.rs:98
pub fn render(ram: &[u8], mode: &DisplayMode, flash_on: bool, bitmap: &mut [u8; BITMAP_SIZE]) {
    if mode.text {
        render_text_rows(ram, bitmap, flash_on, page_offset, 0, 24);
    } else if mode.hires {
        render_hires_scanlines(ram, bitmap, hires_base, scanlines);
    } else {
        render_lores_rows(ram, bitmap, page_offset, text_rows);
    }
}
```
TEXT/GR/HGR 共享同一 280×192 位图输出，前端只需一种格式。

#### Hi-Res NTSC 伪影颜色
```rust
// video.rs:507-533
let color = if !on {
    HIRES_BLACK
} else if prev_on || next_on {
    HIRES_WHITE  // 相邻像素合并为白色
} else if high_bit {
    if x % 2 == 0 { HIRES_BLUE } else { HIRES_ORANGE }
} else {
    if x % 2 == 0 { HIRES_PURPLE } else { HIRES_GREEN }
};
```
正确实现了 Apple II Hi-Res 的 NTSC 伪影颜色。

### 5.2 性能优化

#### fill_rgba 使用 unsafe
```rust
// video.rs:612-623
fn fill_rgba(rgba: &mut [u8], color: [u8; 4]) {
    let word = u32::from_ne_bytes(color);
    let (prefix, aligned, suffix) = unsafe { rgba.align_to_mut::<u32>() };
    aligned.fill(word);
}
```
这是合理的 unsafe 使用，用于 SIMD 友好的内存填充。

---

## 6. 音频系统评价

### 6.1 优点

#### 高通滤波器移除 DC 偏移
```rust
// audio.rs:90-94
let raw = if self.state { 0.25 } else { -0.25 };
// 高通滤波移除 1-bit 扬声器的 DC 偏移
let y = raw - self.hp_prev_x + 0.995 * self.hp_prev_y;
self.hp_prev_x = raw;
self.hp_prev_y = y;
```

#### 快进跳过机制
```rust
// audio.rs:48-62
pub fn skip_to(&mut self, cycle: u64) {
    // 处理跳过区间内的扬声器切换以保持状态正确
    while let Some(&edge) = self.toggles.front() {
        if (edge as f64) <= target {
            self.state = !self.state;
            self.toggles.pop_front();
        } else { break; }
    }
}
```
支持 turbo 模式下正确跳过音频。

---

## 7. 前端实现评价

### 7.1 代码复用

#### SharedArgs 模式
```rust
// a2vm-oxide/src/cli.rs
pub struct SharedArgs {
    pub rom: Option<PathBuf>,
    pub disk: Vec<PathBuf>,
    pub fast_disk: bool,
    pub noise: bool,
}
```
TUI 和 GUI 共享相同的 CLI 参数结构，避免重复代码。

### 7.2 资源嵌入
```rust
// a2vm-oxide/src/cli.rs
pub const DEFAULT_ROM: &[u8] = include_bytes!("../assets/apple2p.rom");
pub const MOVE_ARM_WAV: &[u8] = include_bytes!("../assets/move_arm.wav");
```
编译时嵌入 ROM 和音效，零外部依赖启动。

### 7.3 特性门控
```rust
// a2vm-gui/src/main.rs
#[cfg(feature = "audio")]
use rodio::{OutputStream, Sink};
```
音频是可选特性，可在无 ALSA 的环境编译。

---

## 8. 测试覆盖评价

### 8.1 现有测试

| 模块 | 测试数 | 覆盖范围 |
|------|--------|----------|
| CPU | 1 | Klaus Dormann 功能测试 |
| disk.rs | 15 | 编码、步进电机、I/O |
| machine.rs | 9 | RAM、ROM、键盘、扬声器、启动 |
| video.rs | 8 | 位图、Hi-Res 地址、Lo-Res |
| keyboard.rs | 4 | ASCII 映射 |
| noise.rs | 5 | 机械事件追踪 |

### 8.2 测试质量亮点

#### Klaus Dormann 功能测试
```rust
// tests/klaus_dormann.rs
const SUCCESS_TRAP: u16 = 0x3469;
const MAX_CYCLES: u64 = 100_000_000;

#[test]
fn klaus_dormann_functional_test() {
    // ... 运行完整 6502 测试套件
    assert_eq!(cpu.pc(), SUCCESS_TRAP);
}
```
这是 6502 模拟器的黄金标准测试。

### 8.3 改进建议

#### 增加边界测试
- 磁盘写入后的持久化验证
- 不同 ROM 大小（12K vs 20K）的加载
- 视频模式切换的边界条件

#### 性能回归测试
建议添加基准测试：
```rust
#[bench]
fn bench_cpu_step(b: &mut test::Bencher) {
    b.iter(|| cpu.step(&mut mem));
}
```

---

## 9. 错误处理评价

### 9.1 优点

#### 自定义错误类型
```rust
// error.rs
pub enum Error {
    Io(io::Error),
    UnsupportedRomSize { actual: usize },
    InvalidDiskSize { expected: usize, actual: usize },
    InvalidDiskLocation { drive: usize, track: u8, sector: u8 },
    DiskNotLoaded,
    DiskWriteProtected,
}
```

#### 友好的错误消息
```rust
Error::UnsupportedRomSize { actual } => write!(
    f,
    "Unsupported ROM size: {actual} ({actual:#X}). Only Apple II / Apple II+ ROMs are supported (12K or 20K)."
),
```

### 9.2 潜在问题

#### 前端错误处理
```rust
// a2vm-gui/src/main.rs:98-104
let rom_data = cli.shared.rom_data().unwrap_or_else(|e| {
    eprintln!("Error loading ROM: {e}");
    std::process::exit(1);
});
```
使用 `unwrap_or_else` + `exit(1)` 是可接受的，但建议统一错误处理策略。

---

## 10. 性能考量

### 10.1 热路径优化

#### CPU 步进
```rust
pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
    // 单态化 Bus trait，无虚调用开销
}
```

#### 视频脏标记
```rust
// machine.rs:36
pub video_dirty: bool,  // 写入视频 RAM ($0400-$5FFF) 时设置
```
前端只在 `video_dirty` 时重渲染。

### 10.2 内存分配

#### 预分配音频缓冲
```rust
// a2vm-gui/src/main.rs:153
audio_buffer: Vec::with_capacity(4096),
```

#### 磁盘 Box 使用
```rust
// disk.rs:54
nibble_data: Box<[[u8; NIBBLE_TRACK_SIZE]; 35]>,  // ~230KB
raw_data: Option<Box<[u8; DSK_SIZE]>>,            // ~140KB
```
避免栈溢出，合理使用堆分配。

---

## 11. 代码风格评价

### 11.1 优点

#### 一致的命名约定
- 结构体: `PascalCase` (`AppleII`, `DiskII`, `BusState`)
- 函数: `snake_case` (`read_operand`, `handle_display_switch`)
- 常量: `SCREAMING_SNAKE_CASE` (`CPU_HZ`, `NIBBLE_TRACK_SIZE`)

#### 适当的注释
```rust
// NMOS 6502 bug: JMP ($xxFF) wraps within page
let addr = bus.read_word_page_wrap(ptr);
```

### 11.2 改进建议

#### 增加公共 API 文档
```rust
/// 执行一条 CPU 指令
/// 
/// # 参数
/// - `bus`: 可变借用总线
/// 
/// # 返回
/// 消耗的周期数
pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32
```

---

## 12. 安全性评价

### 12.1 Unsafe 使用
仅有 1 处 unsafe：
```rust
// video.rs:615
let (prefix, aligned, suffix) = unsafe { rgba.align_to_mut::<u32>() };
```
这是标准库函数的安全使用，用于内存对齐访问。

### 12.2 无 panic 风险
- 无 `unwrap()` 在热路径
- 错误情况正确传播
- 边界检查由 Rust 自动处理

---

## 13. 总体评分

| 维度 | 评分 | 说明 |
|------|------|------|
| **架构设计** | 9/10 | 清晰的模块分离，优雅的 Bus trait |
| **代码质量** | 8/10 | 一致的风格，适当的注释 |
| **正确性** | 9/10 | 通过 Klaus Dormann 测试，正确的 BCD 和中断 |
| **性能** | 8/10 | 合理的优化，脏标记，内联 |
| **测试覆盖** | 7/10 | 核心功能有测试，缺乏边界和性能测试 |
| **文档** | 6/10 | AGENTS.md 优秀，公共 API 文档不足 |
| **错误处理** | 8/10 | 自定义错误类型，友好的消息 |

**总体评分**: **8.1/10**

---

## 14. 优先改进建议

### 高优先级
1. **增加 API 文档**: 为所有公共函数添加 `///` 文档注释
2. **边界测试**: 添加磁盘 I/O、ROM 加载的边界条件测试
3. **替换 unreachable!**: 使用更安全的错误处理

### 中优先级
4. **性能基准**: 添加 `#[bench]` 测试防止性能回归
5. **错误处理统一**: 前端使用 `anyhow` 或统一的错误策略
6. **常量提取**: 将魔法数字提取为命名常量

### 低优先级
7. **考虑 `no_std`**: 核心库理论上可以 `no_std`
8. **日志系统**: 添加 `log` facade 支持调试
9. **配置持久化**: 保存用户设置到文件

---

## 15. 结论

A2VM 是一个高质量的 Apple II 模拟器实现，展现了以下优点：

- **架构清晰**: Bus trait 和 CPU-Bus 分离设计优雅
- **实现准确**: 正确处理 BCD 算术、JMP indirect bug、NTSC 伪影颜色
- **功能完整**: 支持双驱、fast-disk、机械噪音
- **代码健康**: 无 unsafe 滥用，一致的风格

该项目适合作为学习 6502 模拟和 Rust 系统编程的参考。建议按照上述优先级进行改进，可进一步提升代码质量。
