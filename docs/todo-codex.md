# A2VM TODO (Codex)

> 来源：`docs/code-review-codex.md`
> 目标：先修正确性和数据安全，再做稳定性与维护性改进。

## P0（立即处理）

- [ ] 修复 SAX 非法指令操作码缺失
  - 文件：`a2vm-core/src/cpu/opcodes.rs`
  - 动作：补齐 `0x83`、`0x87`、`0x8F` 的 SAX 映射（寻址与周期）。
  - 验收：新增/更新测试后，相关用例能在错误实现下失败、修复后通过。

- [ ] 修复 SAX 测试假阳性
  - 文件：`a2vm-core/src/cpu/tests.rs`
  - 动作：重写 `sax_stores_a_and_x`，避免被内存初值 `0x00` 掩盖。
  - 验收：测试可真实验证内存被 `A & X` 写入。

- [ ] 修复 Disk nibble 写入不落盘风险
  - 文件：`a2vm-core/src/disk.rs`
  - 动作：在 motor off（`0xC0E8`）时触发 `sync_nibble_to_raw(selected_drive)`。
  - 动作：增加 `flush_drive` / `flush_all_drives`（或等效接口）。
  - 验收：Q6+Q7 写入后关电机，`.dsk` 文件内容可验证已持久化。

## P1（本轮跟进）

- [ ] 统一 `run_cycles()` 与 `step()` 的磁盘 tick 语义
  - 文件：`a2vm-core/src/machine.rs`
  - 动作：普通 `run_cycles()` 路径也驱动 `disk.tick()`，不要只在 `step()` 生效。
  - 验收：`step` 与 `run_cycles` 在同等预算下行为一致。

- [ ] 增加 TUI 终端状态 RAII 清理
  - 文件：`a2vm-tui/src/main.rs`
  - 动作：引入 guard，保证异常返回也恢复 raw mode / alternate screen。
  - 验收：初始化失败或运行中错误后，终端状态正常。

- [ ] 修复 12K ROM 加载时 slot ROM 残留状态
  - 文件：`a2vm-core/src/machine.rs`、`a2vm-core/src/disk.rs`
  - 动作：新增 `clear_slot_rom()` 并在 12K ROM 分支调用。
  - 验收：先加载 20K 再加载 12K，不应保留旧 slot-6 ROM 内容。

## P2（性能与结构优化）

- [ ] 缩小 `video_dirty` 判定范围
  - 文件：`a2vm-core/src/machine.rs`
  - 动作：从 `0x0400..0x6000` 改为真实视频区（`0x0400..0x0C00`、`0x2000..0x6000`）。
  - 验收：非视频 RAM 写入不触发不必要重绘。

- [ ] 抽取 TUI/GUI 共享运行逻辑到 `a2vm-oxide`
  - 文件：`a2vm-tui/src/main.rs`、`a2vm-gui/src/main.rs`、`a2vm-oxide/src/*`
  - 动作：收敛 emulation tick、audio、noise、perf 统计重复代码。
  - 验收：两端重复逻辑显著减少，功能行为不变。

- [ ] GUI 初始化错误改为 `Result` 传播
  - 文件：`a2vm-gui/src/main.rs`
  - 动作：移除 `process::exit(1)` 直退，改为返回错误给调用方处理。
  - 验收：错误路径可组合且可测试。

## P3（工程质量）

- [ ] 为 `Error::Io` 增加 `source()` 错误链
  - 文件：`a2vm-core/src/error.rs`

- [ ] 测试临时文件改 RAII 清理（或 `tempfile`）
  - 文件：`a2vm-core/src/disk.rs`、`a2vm-core/src/machine.rs`

- [ ] 处理 `render_status_bar` 小写映射（若后续启用）
  - 文件：`a2vm-core/src/video.rs`

- [ ] 清理 `--no-default-features` 下 TUI 未使用字段告警
  - 文件：`a2vm-tui/src/main.rs`

## 统一回归命令

```bash
cargo test
cargo test -p a2vm-core
cargo test klaus_dormann
```
