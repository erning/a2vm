# A2VM 改进计划

## Context

基于 `docs/code-review.md` 的 20 项改进建议，组织为 4 个独立可测试的阶段。
每个阶段完成后运行 `cargo test && cargo build -p a2vm-tui -p a2vm-gui` 验证。

## 执行状态（2026-02-11）

- ✅ Phase A 已完成并通过验证（`cargo test`、`cargo build -p a2vm-tui -p a2vm-gui`）。
- ⏸️ Phase B 暂停（按当前决策，TUI/GUI 去重暂不实施）。
- ✅ Phase C 已完成（C1/C2 已落地并有测试覆盖）。
- ✅ Phase D 已完成（文档与命名常量更新完成）。
- 🔁 Phase B 方案保留：前端共享逻辑如后续恢复，仍建议拆到独立 workspace package（暂定名 `a2vm-frontend-common`），不放入 `a2vm-core`。

---

## Phase A: 快速修复（6 项）✅ 已完成

### A1. `read_operand`/`addr_of` 防御性分支（审查 #2）✅ 已完成
**文件**: `a2vm-core/src/cpu/mod.rs:249-261`
- `_ => 0` 的静默返回已移除，改为 `unreachable!(...)` 暴露异常路径。

### A2. `disk_controller_enabled` 默认改为 false（审查 #16）✅ 已完成
**文件**: `a2vm-core/src/machine.rs:45`
- `disk_controller_enabled: true` → `false`

### A3. I/O 地址匹配去重叠（审查 #5）✅ 已完成
**文件**: `a2vm-core/src/machine.rs`
- 将 `0xC011..=0xC0FF` 拆分为不重叠区间，降低新增设备时的顺序依赖风险。

### A4. fast-disk 路径补齐 `disk.tick()`（审查 #1）✅ 已完成
**文件**: `a2vm-core/src/machine.rs:111-142`
- fast-disk 分支在 `run_until` 与 fallback `step` 后都调用 `disk.tick()`。

### A5. HGR 渲染复用行地址（审查 #11）✅ 已完成
**文件**: `a2vm-core/src/video.rs` — `render_hires_scanlines_rgba`
- 将 `ram[hgr_line_addr(base, y) + prev_col]` 改为 `ram[addr + prev_col]`（addr 已在外层计算）

### A6. `pixels.render()` 错误日志（审查 #13）✅ 已完成
**文件**: `a2vm-gui/src/main.rs:337`
- `pixels.render().ok()` → `if let Err(e) = pixels.render() { eprintln!("render: {e}"); }`

---

## Phase B: 代码去重（审查 #3，最大改动，当前暂停）

将 TUI/GUI 间重复的常量、时序逻辑、CLI 解析提取到独立 package `a2vm-frontend-common/`，避免污染 `a2vm-core` 边界。

### B1. 新建 `a2vm-frontend-common` package
```
a2vm-frontend-common/
├── Cargo.toml
└── src/
    ├── lib.rs      — pub mod + re-export
    ├── constants.rs
    ├── timing.rs
    └── cli.rs
```
**文件**:
- workspace `Cargo.toml` — 添加 member `a2vm-frontend-common`
- `a2vm-tui/Cargo.toml`、`a2vm-gui/Cargo.toml` — 添加对 `a2vm-frontend-common` 的依赖

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

## Phase C: 新功能 ✅ 已完成

### C1. ILL 操作码日志（审查 #6）✅ 已完成（短期目标）
**文件**: `a2vm-core/src/cpu/mod.rs`
- `execute()` 签名添加 `opcode: u8` 参数
- `step()` 调用处传入 opcode
- ILL 分支添加 `#[cfg(debug_assertions)] eprintln!("ILL ${:02X} at PC=${:04X}", ...)`

### C2. 反汇编器（审查 #8）✅ 已完成
**新文件**: `a2vm-core/src/cpu/disasm.rs`
- `pub fn disasm(bus: &dyn Bus, pc: u16) -> (String, u8)` — 用 `peek` 读取，避免副作用
- 按寻址模式格式化操作数（`#$FF`, `$1234,X`, `($00),Y` 等）
- 包含单元测试
- `cpu/mod.rs` 添加 `pub mod disasm;`

注意：`disasm` 应使用 `bus.peek()` 而非 `bus.read()`，签名用 `&dyn Bus`。

---

## Phase D: 文档更新 ✅ 已完成

### D1. 更新 `docs/architecture.md`（审查 #18）✅ 已完成
- 架构图改为 TUI/GUI + a2vm-core（删除 Swift/FFI）
- 项目结构反映实际文件（删除 keyboard.rs、softswitch.rs、ffi.rs 等）
- 添加 frontend 模块说明

### D2. 更新 `docs/milestones.md`（审查 #19）✅ 已完成
- M1-M6 标记为 ✓ COMPLETE
- 添加 M7（代码质量）及后续目标

### D3. `encode_6and2` 命名常量（审查 #20）✅ 已完成
**文件**: `a2vm-core/src/disk.rs`
- 添加 `AUX_BYTES=86`, `MAIN_BYTES=256`, `TOTAL_NIBBLES=342` 等命名常量
- `encode_6and2` 中用命名常量替换 `0x158`, `0x56`, `0x101`, `0x156`

---

## 后续项（当前未完成 / 部分完成）

| 项 | 原因 |
|----|------|
| #3 TUI/GUI 去重 | 按当前决策暂停 |
| #6 非法操作码完整支持 | 已有 debug 日志，常用非法操作码尚未实现 |
| #7 磁盘写入完整链路 | 已支持 RWTS 写入路径，进一步完善 nibble/raw 双向同步可作为后续里程碑 |
| #12 音频缓冲复用落地 | 已提供复用 API，前端调用侧可继续迁移 |
| #15 Cpu pub 字段 | 当前可用但封装性仍可提升 |

---

## 验证

每个 Phase 完成后：
```bash
cargo test                                    # 所有测试通过
cargo build -p a2vm-tui -p a2vm-gui           # 编译成功
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --fast-disk "disks/Apple DOS 3.3 January 1983.dsk"  # 功能正常
```
