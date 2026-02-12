# A2VM 代码审查综合清单（Codex，更新于 2026-02-12）

## 更新说明

- 本版已对照 `docs/code-review-claude.md` 合并新增发现。
- 已剔除“本机编译环境异常”类条目（当前 `cargo test` 全 workspace 已通过）。

## 综合优先级建议

### P0（优先修复）

#### 1. [功能正确性] SAX 非法指令操作码表缺失，且现有测试为假阳性

- 证据
  - `a2vm-core/src/cpu/opcodes.rs:188`（`0x83` 仍为 `ill()`）
  - `a2vm-core/src/cpu/opcodes.rs:192`（`0x87` 仍为 `ill()`）
  - `a2vm-core/src/cpu/opcodes.rs:200`（`0x8F` 仍为 `ill()`）
  - `a2vm-core/src/cpu/tests.rs:138` 当前 `sax_stores_a_and_x` 用例会被内存初值掩盖。
- 风险
  - 依赖这些非法指令的软件行为错误，且现有测试不能有效拦截回归。
- 建议
  - 补齐 `0x83/0x87/0x8F` 到 `SAX` 对应寻址模式与周期。
  - 重写 `sax_stores_a_and_x` 为非零初值场景，确保失败可观测。

#### 2. [数据安全] Disk II nibble 写入路径存在“脏数据不落盘”风险

- 证据
  - `a2vm-core/src/disk.rs:314` 的 `write_nibble()` 仅置 `dirty = true`。
  - `a2vm-core/src/disk.rs:255` 电机关闭路径未触发 `sync_nibble_to_raw()`。
  - `a2vm-core/src/disk.rs:330` 同步函数仅在测试中使用，运行路径未接入。
- 风险
  - nibble 层写盘可能只留在内存，退出后丢失。
- 建议
  - 在 `0xC0E8`（motor off）路径触发 `sync_nibble_to_raw(selected_drive)`。
  - 提供显式 `flush_drive/flush_all_drives`，前端退出时调用并报告错误。
- 建议测试
  - Q6+Q7 写入 -> motor off -> 重读 `.dsk` 文件，断言持久化。

### P1（高价值稳定性）

#### 3. [行为一致性] `run_cycles()` 普通路径未驱动 `disk.tick()`

- 证据
  - `a2vm-core/src/machine.rs:227` 的 `step()` 每条指令后会 `disk.tick(cycles)`。
  - `a2vm-core/src/machine.rs:258` 的普通 `run_cycles()` 直接 `cpu.run(...)`，未 tick。
- 风险
  - 后续实现周期级磁盘时序时，`step()` 与 `run_cycles()` 行为分叉。
- 建议
  - 普通 `run_cycles()` 也按指令驱动 `disk.tick()`，保持语义一致。
  - 不建议删除 `tick()` 调用点；应保留统一扩展接口。

#### 4. [鲁棒性] TUI 异常路径可能遗留 raw mode / alternate screen

- 证据
  - 启用终端状态：`a2vm-tui/src/main.rs:401`。
  - 清理逻辑仅在正常退出：`a2vm-tui/src/main.rs:428`、`a2vm-tui/src/main.rs:429`。
  - 中途 `?` 返回会跳过清理。
- 风险
  - 终端状态污染，影响后续 shell 使用。
- 建议
  - 引入 RAII 终端守卫，在 `Drop` 中保证恢复。

#### 5. [状态一致性] 12K ROM 加载未清理 slot-6 ROM 状态

- 证据
  - `a2vm-core/src/machine.rs:195`（12K 分支仅拷贝主 ROM）
  - `a2vm-core/src/machine.rs:202`（20K 分支会加载 slot ROM）
- 风险
  - 同实例先载入 20K 再载入 12K 时，slot ROM 可能残留旧映像。
- 建议
  - 增加 `DiskII::clear_slot_rom()`，在 12K 分支显式清空。

### P2（性能与架构可维护性）

#### 6. [性能] `video_dirty` 范围过宽导致不必要重绘

- 证据
  - `a2vm-core/src/machine.rs:124` 使用 `(0x0400..0x6000)`。
- 风险
  - 非视频 RAM（如 `$0C00-$1FFF`）写入也触发重绘。
- 建议
  - 收窄为真实视频区：`0x0400..0x0C00` 与 `0x2000..0x6000`。

#### 7. [可维护性] TUI / GUI 运行循环与音频逻辑高度重复

- 证据
  - `a2vm-tui/src/main.rs` 与 `a2vm-gui/src/main.rs` 中 emulation tick、audio、noise、perf 统计逻辑高度相似。
- 建议
  - 抽取共享运行器到 `a2vm-oxide`，前端仅保留 UI/事件层。

#### 8. [错误传播] GUI 初始化直接 `process::exit(1)`，不可组合

- 证据
  - `a2vm-gui/src/main.rs:100`、`a2vm-gui/src/main.rs:104`、`a2vm-gui/src/main.rs:112`。
- 风险
  - 绕过上层错误处理与清理流程。
- 建议
  - `App::new` 改为 `Result<Self, E>`，统一到调用方处理。

### P3（工程质量改进）

#### 9. `Error::Io` 未提供 `source()` 错误链

- 证据
  - `a2vm-core/src/error.rs:41` 仅空实现 `std::error::Error`。
- 建议
  - 为 `Error::Io` 返回 `Some(io_err)`，提升诊断质量。

#### 10. 测试临时文件清理依赖手动 `remove_file`

- 证据
  - `a2vm-core/src/disk.rs`、`a2vm-core/src/machine.rs` 多处测试末尾显式删除。
- 风险
  - panic 时残留临时文件。
- 建议
  - 使用 RAII 封装或 `tempfile`。

#### 11. `render_status_bar` 小写字母映射缺失（函数当前未被调用）

- 证据
  - `a2vm-core/src/video.rs:578`；工程内无调用点。
- 建议
  - 若后续启用状态栏渲染，补齐小写映射或在入口统一 `to_ascii_uppercase()`。

#### 12. `--no-default-features` 下 TUI 存在未使用字段告警

- 证据
  - `a2vm-tui/src/main.rs:125`（`noise` 字段）。
- 建议
  - 对字段做 `#[cfg(feature = "audio")]` 条件编译，或拆分为音频状态结构。

## 建议实施顺序

1. P0-1 SAX 操作码与测试修复。
2. P0-2 nibble 写盘持久化与 flush 机制。
3. P1-3 `run_cycles` / `step` 的 disk.tick 语义统一。
4. P1-4 TUI 终端清理 RAII。
5. P1-5 12K ROM 清理 slot ROM。
6. P2 与 P3 按维护窗口分批处理。

## 验证建议

```bash
cargo test
cargo test -p a2vm-core
cargo test klaus_dormann
```
