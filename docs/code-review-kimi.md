# A2VM 代码审查报告

**审查日期**: 2026-02-12  
**审查者**: Kimi (AI Assistant)  
**项目**: A2VM — Apple II/II+ Emulator  
**语言**: Rust

---

## 1. 项目概述

A2VM 是一个用 Rust 编写的 Apple II/II+ 模拟器，具有以下特点：
- 完整的 6502 CPU 实现（56 条官方指令 + 8 条非法指令）
- 支持 TEXT/GR/HGR 显示模式
- Disk II 软盘控制器仿真
- 扬声器音频输出
- 机械磁盘噪音模拟
- TUI（Braille 字符）和 GUI（GPU 加速）双前端

**项目结构**:
```
a2vm/
├── a2vm-core/     # 核心仿真库
├── a2vm-oxide/    # 共享前端资源
├── a2vm-tui/      # 终端 UI 前端
└── a2vm-gui/      # 图形 UI 前端
```

---

## 2. 架构与设计

### 2.1 整体架构

项目采用分层架构：
- **核心层** (`a2vm-core`): 纯仿真逻辑，无外部依赖
- **共享层** (`a2vm-oxide`): 跨前端共享的 CLI 参数和资源
- **前端层** (`a2vm-tui`/`a2vm-gui`): 平台特定的 UI 实现

**优点**:
- 清晰的模块边界
- 核心库可独立测试和复用
- 前端实现互不干扰

### 2.2 CPU-Bus 模式

```rust
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, val: u8);
    fn peek(&self, addr: u16) -> u8;
    // ...
}
```

**设计亮点**:
- Bus trait 抽象了内存访问，允许 CPU 独立于具体硬件实现
- `read` 使用 `&mut self` 以支持副作用（如 $C030 扬声器切换）
- `peek` 提供无副作用的读取（用于调试）

**潜在问题**:
- `BusState` 结构体较大（包含 RAM、ROM、磁盘等），可能导致缓存不友好

### 2.3 显示渲染管道

统一渲染管道设计：
1. 所有模式（TEXT/GR/HGR）渲染到 280×192 单色位图
2. TUI 将位图转换为 Braille 字符
3. GUI 可选择彩色/单色/扫描线模式渲染 RGBA

**优点**:
- 单一渲染路径减少代码重复
- 易于测试（位图比较）

---

## 3. 代码质量分析

### 3.1 优点

#### 3.1.1 正确的 6502 实现

- **BCD 运算**: 正确处理 NMOS 6502 的 BCD 标志行为（N/Z 来自二进制结果）
  ```rust
  // cpu/mod.rs:807-840
  fn adc_bcd(&mut self, val: u8) {
      // NMOS 6502: N and Z are based on binary result, not BCD result
      self.p.set(N, bin_sum as u8 & 0x80 != 0);
      self.p.set(Z, (bin_sum as u8) == 0);
  }
  ```

- **JMP indirect 页边界 bug**: 正确实现了 $xxFF 读取高字节从 $xx00 的 bug
  ```rust
  // bus.rs:25-30
  fn read_word_page_wrap(&mut self, addr: u16) -> u16 {
      let hi_addr = (addr & 0xFF00) | ((addr.wrapping_add(1)) & 0x00FF);
      // ...
  }
  ```

- **非法指令支持**: 实现了 8 条常见非法指令（LAX, SAX, DCP, ISC, SLO, RLA, RRA, SRE）

#### 3.1.2 良好的测试覆盖

- **单元测试**: 各模块都有对应的测试
  - `machine.rs`: 13 个测试（RAM/ROM 访问、键盘、扬声器、磁盘等）
  - `disk.rs`: 18 个测试（编解码、步进电机、I/O 等）
  - `video.rs`: 10 个测试（渲染、地址计算等）
  - `keyboard.rs`: 5 个测试（键位映射）
  - `noise.rs`: 7 个测试（机械事件追踪）

- **功能测试**: 集成 Klaus Dormann 的 6502 功能测试

#### 3.1.3 类型安全

- 使用 `Flag` enum 代替裸整数表示状态标志位
- 地址模式使用 `AddrMode` enum
- 指令助记符使用 `Mnemonic` enum

#### 3.1.4 错误处理

自定义错误类型 `Error` 实现了 `std::error::Error` 和 `std::fmt::Display`：
```rust
pub enum Error {
    Io(io::Error),
    UnsupportedRomSize { actual: usize },
    InvalidDiskSize { expected: usize, actual: usize },
    // ...
}
```

### 3.2 需要改进的地方

#### 3.2.1 文档缺失

| 文件 | 问题 |
|------|------|
| `opcodes.rs` | 255 行 opcode 表缺少注释说明数据来源 |
| `video.rs` | CHAR_ROM 数组来源未注明 |
| `disk.rs` | DOS 3.3 sector interleave 是标准但需要引用 |

**建议**: 添加数据来源注释和实现参考。

#### 3.2.2 魔法数字

多处使用未经解释的十六进制常数：

```rust
// machine.rs:186-205
match data.len() {
    0x3000 => { /* 12K ROM */ }
    0x5000 => { /* 20K ROM */ }
    _ => { /* error */ }
}

// disk.rs:351
let iob_addr = self.cpu.a() as u16 | ((self.cpu.y() as u16) << 8);
// IOB 字段偏移量
let command = self.bus.peek(iob_addr.wrapping_add(0x0C));
let track = self.bus.peek(iob_addr.wrapping_add(0x04));
```

**建议**: 使用命名常量：
```rust
const ROM_SIZE_12K: usize = 0x3000;
const ROM_SIZE_20K: usize = 0x5000;
const IOB_OFFSET_COMMAND: u16 = 0x0C;
const IOB_OFFSET_TRACK: u16 = 0x04;
```

#### 3.2.3 重复代码

TUI 和 GUI 前端有大量重复的仿真循环代码：

- 两者都实现了相似的 `run_emulation()` 逻辑
- 两者都处理音频和机械噪音
- 两者都有性能监控

**建议**: 将共享的运行时逻辑提取到 `a2vm-oxide`：
```rust
// a2vm-oxide/src/runtime.rs
pub struct EmulatorRuntime {
    apple: AppleII,
    turbo: bool,
    cycle_accum: u128,
    // ...
}

impl EmulatorRuntime {
    pub fn tick(&mut self, dt: Duration) -> u64 {
        // 共享的仿真逻辑
    }
}
```

#### 3.2.4 潜在的 Panic 路径

```rust
// video.rs:71
let ch = char::from_u32(0x2800 + bits as u32).expect("valid braille codepoint");
```

虽然这里的 expect 是安全的（bits 范围 0-255），但最好使用 `unwrap_or` 或 `unsafe` 转换。

#### 3.2.5 不必要的 `#[rustfmt::skip]`

```rust
// opcodes.rs:4
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[rustfmt::skip]
pub enum Mnemonic {
    ADC, AND, ASL, BCC, BCS, BEQ, BIT, BMI,
    // ...
}
```

Rustfmt 的默认格式化对枚举是合理的，skip 反而降低了可读性。

---

## 4. 性能考虑

### 4.1 视频渲染

```rust
// video.rs:246-251
fn set_pixel(bitmap: &mut [u8; BITMAP_SIZE], x: usize, y: usize) {
    let byte_idx = y * BITMAP_STRIDE + x / 8;
    let bit_idx = 7 - (x % 8);
    bitmap[byte_idx] |= 1 << bit_idx;
}
```

**分析**: 位图渲染使用逐像素设置，没有 SIMD 优化。不过对于 280×192 分辨率来说性能足够。

### 4.2 音频合成

```rust
// audio.rs:65-99
pub fn render_until_into(&mut self, target_cycle: u64, sample_rate: u32, out: &mut Vec<f32>) {
    // 使用 VecDeque 存储切换时间戳
    // 逐个样本合成
}
```

**分析**: 
- 使用 `VecDeque` 高效处理切换事件
- 预分配输出缓冲区
- 高通滤波器去除 DC 偏移

### 4.3 Fast Disk 模式

RWTS trap 在 `$B7B5` 拦截 DOS 3.3 磁盘 I/O，直接读写原始扇区数据：

```rust
// machine.rs:349-419
fn try_rwts_trap(&mut self) -> Option<u32> {
    // 解析 IOB，直接读写扇区
}
```

**优点**: 显著提升磁盘操作速度

---

## 5. 安全与可靠性

### 5.1 内存安全

- 无 `unsafe` 代码块（除了 RGBA 填充的对齐优化）
- 数组访问使用常量索引或边界检查

### 5.2 错误处理

| 组件 | 处理策略 |
|------|----------|
| ROM 加载 | 验证大小（12K/20K），返回 `Result` |
| 磁盘加载 | 验证大小（143360 字节），检查写保护 |
| I/O 操作 | 使用 `?` 传播 I/O 错误 |

### 5.3 潜在问题

```rust
// disk.rs:147-149
let write_protected = std::fs::metadata(path)
    .map(|meta| meta.permissions().readonly())
    .unwrap_or(true);
```

**问题**: 无法获取元数据时默认只读，这可能导致数据丢失预期（用户以为可写但实际上不可写）。

**建议**: 添加警告日志。

---

## 6. 可维护性

### 6.1 模块组织

```
a2vm-core/src/
├── lib.rs          # 模块导出
├── bus.rs          # Bus trait
├── machine.rs      # AppleII + BusState
├── cpu/
│   ├── mod.rs      # CPU 实现
│   ├── opcodes.rs  # 指令表
│   ├── addressing.rs
│   ├── status.rs
│   ├── disasm.rs
│   └── tests.rs
├── video.rs        # 渲染
├── audio.rs        # 扬声器
├── disk.rs         # Disk II
├── keyboard.rs     # 键位映射
├── memory.rs       # （空？）
├── timing.rs       # 常量
└── error.rs        # 错误类型
```

**问题**: `memory.rs` 似乎为空或无用，应检查是否可删除。

### 6.2 命名规范

- ✅ 使用 `snake_case` 函数名
- ✅ 使用 `CamelCase` 类型名
- ✅ 常量使用 `SCREAMING_SNAKE_CASE`
- ⚠️ 某些缩写不一致：`rgb` vs `RGBA`，`kb` vs `keyboard`

### 6.3 代码行数统计

```bash
find . -name "*.rs" -not -path "./target/*" | xargs wc -l
```

估算：
- `a2vm-core`: ~4000 行
- `a2vm-oxide`: ~300 行
- `a2vm-tui`: ~450 行
- `a2vm-gui`: ~500 行
- 总计: ~5250 行

---

## 7. 建议与改进

### 7.1 高优先级

1. **添加架构文档**: 在 `docs/` 目录添加详细的架构说明
2. **修复魔法数字**: 为所有硬件地址和偏移量添加命名常量
3. **提取共享运行时**: 将 TUI/GUI 重复的仿真循环代码提取到共享库

### 7.2 中优先级

4. **增强文档**: 为 opcode 表、CHAR_ROM、sector interleave 等添加来源注释
5. **添加集成测试**: 测试完整的启动流程和磁盘 I/O
6. **配置系统**: 支持配置文件而非仅命令行参数

### 7.3 低优先级

7. **性能优化**: 使用 SIMD 优化视频渲染（如需要）
8. **调试功能**: 添加断点、单步执行、内存查看器等调试功能
9. **保存状态**: 支持保存/加载模拟器状态

---

## 8. 总结

### 8.1 评分

| 类别 | 评分 | 说明 |
|------|------|------|
| 功能完整性 | ★★★★★ | 完整的 Apple II+ 仿真 |
| 代码正确性 | ★★★★★ | 准确的 6502 实现，通过功能测试 |
| 代码可读性 | ★★★★☆ | 结构清晰，但文档可加强 |
| 测试覆盖 | ★★★★☆ | 单元测试良好，缺少集成测试 |
| 架构设计 | ★★★★★ | 分层清晰，前端分离 |
| 文档完整性 | ★★★☆☆ | README 良好，代码内文档不足 |

**总体评分**: 4.3/5.0

### 8.2 最终评价

A2VM 是一个**高质量**的 Apple II 模拟器项目：

- ✅ **架构优秀**: 清晰的分层和模块边界
- ✅ **实现准确**: 正确处理 6502 的各种怪癖
- ✅ **测试充分**: 各模块都有相应的单元测试
- ✅ **功能完整**: 支持显示、音频、磁盘等完整功能

需要改进的主要是**文档**和**代码可读性**方面。魔法数字和重复代码虽然不影响功能，但增加了维护难度。

**推荐继续开发**，项目具有良好的基础和扩展潜力。

---

## 9. 附录

### 9.1 关键文件清单

| 文件 | 功能 | 代码行数 |
|------|------|----------|
| `a2vm-core/src/cpu/mod.rs` | 6502 CPU 实现 | ~940 |
| `a2vm-core/src/machine.rs` | AppleII 机器状态 | ~690 |
| `a2vm-core/src/disk.rs` | Disk II 控制器 | ~940 |
| `a2vm-core/src/video.rs` | 视频渲染 | ~750 |
| `a2vm-core/src/cpu/opcodes.rs` | 指令表 | ~330 |
| `a2vm-tui/src/main.rs` | TUI 前端 | ~430 |
| `a2vm-gui/src/main.rs` | GUI 前端 | ~490 |

### 9.2 参考资源

- [Klaus Dormann's 6502 Functional Tests](https://github.com/Klaus2m5/6502_65C02_functional_tests)
- [Apple IIe Technical Reference Manual](https://archive.org/details/AppleIIeTechnicalReferenceManual)
- [DOS 3.3 Sector Format](http://www.apple2.org.za/gswv/a2zine/Docs/DiskImage_1Mg.TXT)
