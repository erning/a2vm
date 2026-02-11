# A2VM 改进计划

## Context

基于 `docs/code-review.md` 的 20 项改进建议，组织为 4 个独立可测试的阶段。
每个阶段完成后运行 `cargo test && cargo build -p a2vm-tui -p a2vm-gui` 验证。

---

## Phase A: 快速修复（6 项，每项 1-5 行改动）

### A1. `read_operand`/`addr_of` 添加 debug_assert（审查 #2）
**文件**: `a2vm-core/src/cpu/mod.rs:249-261`
- `_ => 0` 分支添加 `debug_assert!(false, "unexpected operand")`

### A2. `disk_controller_enabled` 默认改为 false（审查 #16）
**文件**: `a2vm-core/src/machine.rs:45`
- `disk_controller_enabled: true` → `false`

### A3. I/O 地址匹配添加注释（审查 #5）
**文件**: `a2vm-core/src/machine.rs:270-308`
- 在 `0xC011..=0xC0FF => 0x00` 上方添加注释说明匹配顺序重要性

### A4. fast-disk 路径添加 TODO（审查 #1）
**文件**: `a2vm-core/src/machine.rs:111-142`
- 在 fast-disk 分支添加 `// TODO: call disk.tick() when cycle-accurate timing is implemented`

### A5. HGR 渲染复用行地址（审查 #11）
**文件**: `a2vm-core/src/video.rs` — `render_hires_scanlines_rgba`
- 将 `ram[hgr_line_addr(base, y) + prev_col]` 改为 `ram[addr + prev_col]`（addr 已在外层计算）

### A6. `pixels.render()` 错误日志（审查 #13）
**文件**: `a2vm-gui/src/main.rs:337`
- `pixels.render().ok()` → `if let Err(e) = pixels.render() { eprintln!("render: {e}"); }`

---

## Phase B: 代码去重（审查 #3，最大改动）

将 TUI/GUI 间重复的常量、时序逻辑、CLI 解析提取到 `a2vm-core/src/frontend/`。

### B1. 新建 `frontend` 模块目录
```
a2vm-core/src/frontend/
├── mod.rs          — pub mod + re-export
├── constants.rs    — 共享常量
├── timing.rs       — EmulationTimer
└── cli.rs          — CommonArgs + build_apple()
```
**文件**: `a2vm-core/src/lib.rs` — 添加 `pub mod frontend;`

### B2. 共享常量 (`constants.rs`)
```rust
pub const CPU_HZ: u64 = 1_023_000;
pub const TURBO_MULTIPLIER: u64 = 4;
pub const FLASH_HALF_PERIOD_MS: u128 = 267;
pub const PERF_SAMPLE_INTERVAL_MS: u64 = 250;
pub const FRAME_INTERVAL_MICROS: u64 = 16_667;
pub const AUDIO_SAMPLE_RATE: u32 = 44_100;
```

### B3. EmulationTimer (`timing.rs`)
```rust
pub struct EmulationTimer {
    boot_time: Instant,
    last_tick: Instant,
    cycle_accum: u128,
    turbo: bool,
    emu_mhz: f64,
    perf_last_time: Instant,
    perf_last_cycles: u64,
}
```
关键方法：
- `tick() -> (cycles_to_run: u64, real_cycles: u64)` — 墙钟→周期换算 + turbo
- `update_perf(cpu_cycles) -> bool` — 性能测量，返回是否更新
- `flash_on() -> bool` — Flash 状态
- `toggle_turbo()` / `is_turbo()` / `emu_mhz()`

### B4. CLI 解析 + 工厂函数 (`cli.rs`)
```rust
pub struct CommonArgs {
    pub rom_path: String,
    pub disk_file: Option<String>,
    pub fast_disk: bool,
    pub color_mode: Option<DisplayColorMode>,  // GUI 用，TUI 忽略
}

impl CommonArgs {
    pub fn parse() -> Result<Self, i32> { ... }
}

pub fn build_apple(args: &CommonArgs) -> io::Result<AppleII> {
    // load_rom, set_disk_controller, load_disk, set_fast_disk, reset
}
```
`--color-mode` 统一解析，TUI 忽略该字段。

### B5. 重构 TUI (`a2vm-tui/src/main.rs`)
- 删除本地常量定义（~20 行）
- 删除 CLI 解析代码（~65 行）
- 替换时序变量为 `let mut timer = EmulationTimer::new()`
- 主循环中用 `timer.tick()`, `timer.update_perf()`, `timer.flash_on()`

### B6. 重构 GUI (`a2vm-gui/src/main.rs`)
- 删除 `CliArgs` 结构体和 `parse_args()` 函数（~90 行）
- 删除本地常量定义（~18 行）
- App 结构体中时序字段替换为 `timer: EmulationTimer`
- `run_emulation()` 中用 timer 方法

**预计净减少**: TUI ~85 行，GUI ~90 行

---

## Phase C: 新功能

### C1. ILL 操作码日志（审查 #6）
**文件**: `a2vm-core/src/cpu/mod.rs`
- `execute()` 签名添加 `opcode: u8` 参数
- `step()` 调用处传入 opcode
- ILL 分支添加 `#[cfg(debug_assertions)] eprintln!("ILL ${:02X} at PC=${:04X}", ...)`

### C2. 反汇编器（审查 #8）
**新文件**: `a2vm-core/src/cpu/disasm.rs`
- `pub fn disasm(bus: &dyn Bus, pc: u16) -> (String, u8)` — 用 `peek` 读取，避免副作用
- 按寻址模式格式化操作数（`#$FF`, `$1234,X`, `($00),Y` 等）
- 包含单元测试
- `cpu/mod.rs` 添加 `pub mod disasm;`

注意：`disasm` 应使用 `bus.peek()` 而非 `bus.read()`，签名用 `&dyn Bus`。

---

## Phase D: 文档更新

### D1. 更新 `docs/architecture.md`（审查 #18）
- 架构图改为 TUI/GUI + a2vm-core（删除 Swift/FFI）
- 项目结构反映实际文件（删除 keyboard.rs、softswitch.rs、ffi.rs 等）
- 添加 frontend 模块说明

### D2. 更新 `docs/milestones.md`（审查 #19）
- M1-M6 标记为 ✓ COMPLETE
- 添加 M7（代码质量）及后续目标

### D3. `encode_6and2` 命名常量（审查 #20）
**文件**: `a2vm-core/src/disk.rs`
- 添加 `AUX_BYTES=86`, `MAIN_BYTES=256`, `TOTAL_NIBBLES=342`, `ENCODING_BUF_SIZE=344`
- `encode_6and2` 中用命名常量替换 `0x158`, `0x56`, `0x101`, `0x156`

---

## 延期项

| 项 | 原因 |
|----|------|
| #4 自定义错误类型 | 需引入 thiserror，当前规模收益有限 |
| #7 磁盘写入 | 独立里程碑 |
| #9 CPU 单元测试 | 持续性工作 |
| #10 ROM 依赖测试 | CI 相关，非紧急 |
| #14 Status 枚举 | 改动大（全部 flag 引用），收益有限 |
| #15 Cpu pub 字段 | 当前规模可接受 |
| #17 CHAR_ROM 注释 | 极低优先级 |

---

## 验证

每个 Phase 完成后：
```bash
cargo test                                    # 所有测试通过
cargo build -p a2vm-tui -p a2vm-gui           # 编译成功
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --fast-disk "disks/Apple DOS 3.3 January 1983.dsk"  # 功能正常
```
