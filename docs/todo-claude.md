# A2VM TODO（基于代码审查）

> 来源：`docs/code-review-claude.md`、`docs/code-review-codex.md`

## P0 — 必须修复

- [ ] **#1 SAX 操作码缺失 + 假阳性测试**
  - `a2vm-core/src/cpu/opcodes.rs`: 补充 0x83 (IndirectX)、0x87 (ZeroPage)、0x8F (Absolute) 的 SAX 条目
  - `a2vm-core/src/cpu/tests.rs`: 修复 `sax_stores_a_and_x` 测试（改用非零期望值，如 `cpu.a = 0xFF`）
  - 为 0x83、0x8F 新增测试用例

- [ ] **#4 磁盘 nibble 写入无持久化**
  - `a2vm-core/src/disk.rs`: 电机关闭路径（`0xC0E8`）触发 `sync_nibble_to_raw(selected_drive)`
  - `a2vm-core/src/disk.rs`: 新增 `flush_all_drives()` 公开方法
  - 前端退出时调用 `flush_all_drives()`
  - 新增测试：Q6/Q7 写模式写入 → 关电机 → 验证数据已同步到 raw image

## P1 — 重要改进

- [ ] **#5 `run_cycles()` 普通路径未驱动 `disk.tick()`**
  - `a2vm-core/src/machine.rs`: 统一 `run_cycles()` 非 fast-disk 分支的执行路径，每条指令后调用 `disk.tick()`
  - 或引入 `cpu.run_with_hook()` 统一处理外设 tick

- [ ] **#3 video_dirty 范围过宽**
  - `a2vm-core/src/machine.rs:124`: 将 `(0x0400..0x6000)` 收窄为 `(0x0400..0x0C00) || (0x2000..0x6000)`

- [ ] **#15 TUI 错误路径遗留 raw mode**
  - `a2vm-tui/src/main.rs`: 引入 `TerminalGuard` RAII 结构体，在 `Drop` 中恢复终端状态
  - 移除 main 末尾手动清理代码

## P2 — 架构优化

- [ ] **#2 TUI/GUI 代码重复**
  - `a2vm-oxide`: 提取共享的 `EmulatorRunner`（模拟循环、turbo、cycle_accum）
  - `a2vm-oxide`: 提取共享的音频输出逻辑（PCM 采集 + Sink 提交）
  - `a2vm-oxide`: 提取共享的机械噪声处理逻辑
  - `a2vm-oxide`: 提取共享的性能统计（MHz 计算）
  - `a2vm-oxide`: 提取共享的初始化流程（ROM/disk/reset）

- [ ] **#16 12K ROM 加载未清理 slot-6 ROM**
  - `a2vm-core/src/disk.rs`: 新增 `DiskII::clear_slot_rom()` 方法
  - `a2vm-core/src/machine.rs`: 12K ROM 分支调用 `clear_slot_rom()`
  - 新增测试：先加载 20K 再加载 12K，验证 `$C600` 返回 0

- [ ] **#7 CPU 中断 cycle 计数不一致**
  - `a2vm-core/src/cpu/mod.rs`: `handle_nmi()`/`handle_irq()` 改为只返回 cycle 数，不内部累加
  - `step()` 统一负责 `self.cycles += ...`

- [ ] **#10 Workspace 依赖管理**
  - `Cargo.toml`（workspace root）: 添加 `[workspace.dependencies]` 统一 clap、rodio、a2vm-core、a2vm-oxide 版本
  - 各 crate 的 Cargo.toml 改为引用 `{ workspace = true }`

## P3 — 小改进

- [ ] **#17 `noise` 字段 audio feature 告警**
  - `a2vm-tui/src/main.rs`: `noise` 字段加 `#[cfg(feature = "audio")]`，对应初始化和使用处同步修改

- [ ] **#6 `rom_data()` 不必要拷贝**
  - `a2vm-oxide/src/cli.rs`: 返回类型改为 `Cow<'static, [u8]>`，嵌入 ROM 走 `Cow::Borrowed`

- [ ] **#9 状态栏不支持小写**
  - `a2vm-core/src/video.rs` `render_status_bar`: 添加 `0x60..0x80` 范围映射到与大写相同字形

- [ ] **#11 Error source 链缺失**
  - `a2vm-core/src/error.rs`: 为 `Error` 实现 `source()` 方法，`Io` 变体返回内部 `io::Error`

- [ ] **#12 GUI `process::exit`**
  - `a2vm-gui/src/main.rs`: `App::new()` 改为返回 `Result`，错误在 `main()` 中处理

- [ ] **#8 测试临时文件清理**
  - `a2vm-core/src/machine.rs`、`a2vm-core/src/disk.rs`（tests）: 引入 `TempFile` RAII 守卫或 `tempfile` crate

- [ ] **#13 `fill_rect` 效率**
  - `a2vm-core/src/video.rs`: Lo-Res 7×4 块改用批量字节操作代替逐像素 `set_pixel`

- [ ] **#14 `AppleKey::Space` 冗余**
  - `a2vm-core/src/keyboard.rs`: 移除 `Space` 变体，统一走 `Printable(' ')`；更新前端映射
