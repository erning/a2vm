# A2VM 代码审查报告

> 审查范围：全部源码（a2vm-core、a2vm-tui、a2vm-gui）
> 日期：2026-02-11

---

## 总体评价

代码质量高，架构清晰。6502 CPU 实现经过 Klaus Dormann 功能测试验证，BCD 细节处理正确。Bus trait 设计合理（`read(&mut self)` 正确建模副作用读取）。已实现 TEXT/GR/HGR 渲染、Disk II 控制器、Speaker 音频、TUI 和 GUI 双前端。

以下按优先级分类列出改进建议。

---

## P0：潜在 Bug

### 1. `run_cycles` fast-disk 路径未调用 `disk.tick()`

`machine.rs:111-141` — 正常 `step()` 路径在每条指令后调用 `self.disk.tick(cycles)`，但 `run_cycles` 的 fast-disk 分支使用 `cpu.run_until()` 批量执行，跳过了 `disk.tick()`。当前 `tick()` 是空实现无影响，但如果未来添加周期精确的磁盘时序（如 WOZ 格式支持），此处会导致磁盘状态不同步。

**建议**：在 fast-disk 路径的内循环中，累计 `run_until` 执行的周期数并调用 `disk.tick()`。或在 `tick()` 实现时同步修复此路径。

### 2. `read_operand` / `addr_of` 对无效操作数静默返回 0

`cpu/mod.rs:249-261` — 当 `Operand` 不匹配预期时返回 0，可能掩盖 opcode 表配置错误。

```rust
fn read_operand(&self, resolved: &Resolved, bus: &mut dyn Bus) -> u8 {
    match resolved.operand {
        Operand::Address(addr) => bus.read(addr),
        _ => 0,  // ← 静默忽略
    }
}
```

**建议**：在 `_ =>` 分支添加 `debug_assert!(false, "unexpected operand for {:?}", mnemonic)` 或使用 `unreachable!()` 在调试模式下暴露错误。

---

## P1：架构改进

### 3. TUI 与 GUI 大量重复代码

以下逻辑在两个前端中几乎完全相同：

| 重复内容 | TUI 位置 | GUI 位置 |
|---------|----------|----------|
| CLI 参数解析 | `tui/main.rs:142-206` | `gui/main.rs:56-145` |
| 键盘映射 | `tui/main.rs:104-139` | `gui/main.rs:391-431` |
| 墙钟→CPU 周期换算 | `tui/main.rs:298-323` | `gui/main.rs:239-267` |
| 性能测量 | `tui/main.rs:326-336` | `gui/main.rs:269-313` |
| 音频采样 | `tui/main.rs:316-322` | `gui/main.rs:260-266` |
| Flash 计算 | `tui/main.rs:345` | `gui/main.rs:316` |
| 常量定义 | CPU_HZ, TURBO_MULTIPLIER, FLASH_HALF_PERIOD_MS, AUDIO_SAMPLE_RATE | 同上 |

**建议**：

- 将 CLI 解析提取到 `a2vm-core` 的共享模块（或使用 `clap` crate 统一处理）。
- 将模拟时序逻辑（周期累加器、turbo、性能测量、flash 状态）封装为 `EmulationTimer` 结构体，放入 `a2vm-core`。
- 常量如 `CPU_HZ` 已在 `audio.rs` 中定义一次（`pub const CPU_HZ: f64`），但 TUI/GUI 又各自定义了 `const CPU_HZ: u64`，应统一使用 `a2vm_core::audio::CPU_HZ`。

### 4. 缺少自定义错误类型

整个项目直接使用 `io::Result` / `io::Error`。ROM 加载、磁盘加载等操作的错误信息通过 `io::Error::new(InvalidData, ...)` 构造，不够类型化。

**建议**：定义 `a2vm_core::Error` 枚举（可使用 `thiserror`），区分 `RomError`、`DiskError` 等。对核心库来说，这使 API 更清晰，也方便前端做差异化错误处理。

### 5. I/O 地址匹配依赖顺序，易碎

`machine.rs:259-296` — Bus::read 的 match 臂有重叠范围，依赖 Rust 的先匹配优先：

```rust
0xC030 => { ... },           // speaker
0xC050..=0xC057 => { ... },  // display
0xC0E0..=0xC0EF => { ... },  // disk
0xC011..=0xC0FF => 0x00,     // ← 覆盖了上面三个范围
```

新增 I/O 设备时，若将新 handler 放在 `0xC011..=0xC0FF` 之后则不会匹配到。

**建议**：考虑将 `0xC011..=0xC0FF` 拆分为不重叠的子范围，或添加注释标注匹配顺序的重要性。未来设备增多时，可改用查表 / 函数指针数组来路由 I/O。

---

## P2：功能完善

### 6. ILL（非法操作码）静默忽略

`cpu/mod.rs:590-593` — 所有非法操作码被当作 NOP 处理，无任何日志。一些 Apple II 软件依赖非法操作码（如 LAX、SAX、DCP 等）。

**建议**：
- 短期：添加 `#[cfg(debug_assertions)]` 日志，输出遇到的非法操作码及 PC 地址。
- 长期：实现常用非法操作码（至少 LAX、SAX、DCP、ISC、SLO、RLA、RRA、SRE），以提高软件兼容性。

### 7. Disk II 不支持写入

`disk.rs:127-128,154-161` — Q7（写模式）标志和 `write_latch` 存在，但写操作未实现。这意味着无法保存文件到磁盘镜像。

**建议**：实现写入路径：将 nibble 数据写回 Drive 的 nibble_data，退出时（或定期）反向解码写回 raw_data 和 .dsk 文件。需要注意写保护检查和脏标记。

### 8. 缺少反汇编器

调试时只能看到 PC 地址和寄存器值，无法看到当前指令的助记符和操作数。

**建议**：在 `cpu/` 中添加 `disasm(bus, pc) -> (String, u8)` 函数，返回反汇编文本和指令字节数。可用于 TUI/GUI 的调试叠加层，也方便单步调试。

### 9. 缺少 CPU 指令级单元测试

当前仅有 Klaus Dormann 集成测试。虽然覆盖全面，但定位单条指令的 bug 效率低。

**建议**：为关键指令组添加独立单元测试（ADC/SBC 的各种 BCD 边界、移位指令、分支跨页等）。可以用 `FlatMemory` 构造极简测试用例。

### 10. 集成测试依赖外部 ROM 文件

`machine.rs` 中多个测试引用 `roms/apple2p.rom` 和 `disks/Apple DOS 3.3...dsk`。CI 环境或新 clone 的仓库中这些文件可能不存在。

**建议**：使用 `#[ignore]` 标记依赖外部文件的测试，或在测试函数开头检查文件是否存在，不存在时跳过（`return` 而非 `panic`）。

---

## P3：性能优化

### 11. HGR 颜色渲染重复计算行地址

`video.rs:491-503` — `render_hires_scanlines_rgba` 在检查相邻像素是否亮起时，重新调用 `hgr_line_addr(base, y)`：

```rust
let prev_col = (x - 1) / 7;
let prev_bit = (x - 1) % 7;
ram[hgr_line_addr(base, y) + prev_col] & (1 << prev_bit) != 0
```

外层循环已经计算了 `addr = hgr_line_addr(base, y)`，可直接复用。

**建议**：将 `hgr_line_addr` 的结果复用为 `ram[addr + prev_col]`。

### 12. `render_until` 每次调用分配新 Vec

`audio.rs:43-77` — 每帧（~60fps）调用 `render_until`，每次分配一个新 `Vec<f32>`（约 735 个样本）。

**建议**：使用可复用的缓冲区（传入 `&mut Vec<f32>` 或在 `Speaker` 内部维护缓冲区），减少分配。对于当前规模影响极小，可作为后续优化。

### 13. `pixels.render().ok()` 静默吞噬错误

`gui/main.rs:337` — GPU 渲染失败被忽略。在 Metal/Vulkan 设备丢失等场景下，用户看不到任何反馈。

**建议**：至少用 `if let Err(e) = pixels.render() { eprintln!("render error: {e}"); }` 记录错误。

---

## P4：代码质量

### 14. `Status` 的标志位常量类型安全性

`cpu/status.rs:6-13` — 标志位用 `pub const C: u8 = 0` 等裸常量表示，`set(flag: u8, val: bool)` 接受任意 u8 值。传入错误的值（如 8）不会编译报错。

**建议**：考虑使用枚举：
```rust
#[repr(u8)]
pub enum Flag { C = 0, Z = 1, I = 2, D = 3, B = 4, U = 5, V = 6, N = 7 }
```
这样 `set(Flag::C, true)` 比 `set(C, true)` 更安全，且不影响性能。

### 15. `Cpu` 所有字段 pub

`cpu/mod.rs:10-20` — 所有寄存器字段都是 `pub`，外部代码可以直接修改 CPU 状态。当前项目规模下可接受，但随着代码增长，考虑提供受控的访问方法。

### 16. `AppleII::new()` 中 `disk_controller_enabled` 默认为 true

`machine.rs:45` — 即使没有加载磁盘，Disk II 控制器默认也是启用的。前端代码会在没有磁盘时显式关闭它（`set_disk_controller_enabled(disk_file.is_some())`），但如果直接使用 `AppleII::new()` 而忘记关闭，访问 `$C600-$C6FF` 会返回未初始化的 slot ROM 数据。

**建议**：默认设为 `false`，仅在加载磁盘时启用。

### 17. `CHAR_ROM` 硬编码在源码中

`video.rs:44-77` — 512 字节的字符 ROM 数据直接嵌入 Rust 源码。数据来源和正确性难以验证。

**建议**：添加注释说明数据来源（如 "Data from Apple II Video Character ROM, verified against ..."），或从二进制文件 `include_bytes!` 加载以便与原始 ROM dump 对比。

---

## P5：文档与可维护性

### 18. `architecture.md` 与实际代码不同步

- 仍然提及 `keyboard.rs`、`softswitch.rs`、`ffi.rs`、`a2vm-app/` 等未实现的文件
- 项目结构图中没有 `a2vm-tui/` 和 `a2vm-gui/`
- FFI 接口和 Swift 前端描述与当前 TUI/GUI Rust 前端不一致

**建议**：更新 architecture.md 以反映实际实现状态。

### 19. `milestones.md` 未更新

M1 标记为"当前"，但 M2-M5 实际上已经（部分或全部）完成。Monitor、BASIC、HGR、Disk II、Sound 都已实现。

**建议**：更新里程碑状态，标记已完成项，添加新的后续目标。

### 20. `encode_6and2` 中魔法数字较多

`disk.rs:323-357` — `0x158`、`0x56`、`0x101`、`0x156` 等常量缺少解释。

**建议**：添加命名常量：
```rust
const AUX_BYTES: usize = 86;    // 0x56
const MAIN_BYTES: usize = 256;  // 0x100
const TOTAL_NIBBLES: usize = AUX_BYTES + MAIN_BYTES; // 342 = 0x156
```

---

## 总结

| 优先级 | 数量 | 主题 |
|--------|------|------|
| P0 | 2 | 潜在 bug（fast-disk tick、静默 0 返回） |
| P1 | 3 | 架构（代码重复、错误类型、I/O 路由） |
| P2 | 5 | 功能（非法操作码、磁盘写入、反汇编器、单元测试、ROM 依赖） |
| P3 | 3 | 性能（HGR 地址复用、音频分配、渲染错误处理） |
| P4 | 4 | 代码质量（类型安全、封装、默认值、硬编码数据） |
| P5 | 3 | 文档（architecture.md、milestones.md、魔法数字） |

建议优先处理 P0 和 P1 项，特别是代码重复问题（#3），因为它直接影响日常开发效率。
