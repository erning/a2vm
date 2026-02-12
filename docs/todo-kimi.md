# A2VM 统一 TODO 清单

> 综合来源: code-review-claude.md, code-review-codex.md, code-review-zhipu.md, code-review-kimi.md  
> 整理时间: 2026-02-12  
> 整理者: Kimi

---

## P0 — 必须立即修复（功能缺陷/数据安全）

### P0.1 修复 SAX 非法指令操作码缺失
- **问题**: SAX 指令遗漏三个变体：0x83 (IndirectX)、0x87 (ZeroPage)、0x8F (Absolute)
- **文件**: `a2vm-core/src/cpu/opcodes.rs`
- **动作**:
  - [ ] 将 `0x83` 改为 `op(SAX, IndirectX, 6, false)`
  - [ ] 将 `0x87` 改为 `op(SAX, ZeroPage, 3, false)`
  - [ ] 将 `0x8F` 改为 `op(SAX, Absolute, 4, false)`
- **验收**: 新增测试用例验证三个操作码正确执行

### P0.2 修复 SAX 测试假阳性
- **问题**: `sax_stores_a_and_x` 测试使用内存初始值 0x00 作为期望值，无法检测实现错误
- **文件**: `a2vm-core/src/cpu/tests.rs`
- **动作**:
  - [ ] 修改测试使用非零期望值（如 `cpu.a = 0xFF`, `cpu.x = 0x0F`）
  - [ ] 确保测试在错误实现下失败、修复后通过

### P0.3 修复磁盘 nibble 写入不持久化
- **问题**: Q6+Q7 写模式写入的数据只修改内存中的 `nibble_data`，电机关闭时不会自动同步到文件
- **文件**: `a2vm-core/src/disk.rs`, `a2vm-core/src/machine.rs`
- **动作**:
  - [ ] 在电机关闭路径（`0xC0E8`）触发 `sync_nibble_to_raw(selected_drive)`
  - [ ] 新增 `flush_drive(drive)` / `flush_all_drives()` 公开方法
  - [ ] 前端退出时调用 `flush_all_drives()`
  - [ ] 新增测试：Q6+Q7 写入 → 关电机 → 验证 `.dsk` 文件已更新

---

## P1 — 重要改进（行为一致性/稳定性）

### P1.1 统一 `run_cycles()` 与 `step()` 的 disk.tick() 语义
- **问题**: `step()` 每条指令后调用 `disk.tick()`，但 `run_cycles()` 普通分支直接调用 `cpu.run()` 跳过了 `disk.tick()`
- **文件**: `a2vm-core/src/machine.rs`
- **动作**:
  - [ ] 方案 A: 普通 `run_cycles()` 改为按指令循环，每条指令后调用 `disk.tick()`
  - [ ] 方案 B: 引入 `cpu.run_with_hook()`，在 hook 中统一处理外设 tick
- **验收**: `step` 与 `run_cycles` 在同等预算下行为一致

### P1.2 收窄 video_dirty 检测范围
- **问题**: 当前范围 `(0x0400..0x6000)` 包含了非视频 RAM（`$0C00-$1FFF`），导致不必要重绘
- **文件**: `a2vm-core/src/machine.rs:124`
- **动作**:
  - [ ] 将范围收窄为真实视频区：`(0x0400..0x0C00) || (0x2000..0x6000)`

### P1.3 TUI 错误路径遗留 raw mode / alternate screen
- **问题**: 终端状态恢复逻辑仅在正常退出路径执行，错误返回时终端状态污染
- **文件**: `a2vm-tui/src/main.rs`
- **动作**:
  - [ ] 引入 `TerminalGuard` RAII 结构体，在 `Drop` 中恢复终端状态
  - [ ] 移除 main 末尾手动清理代码

### P1.4 修复 12K ROM 加载时 slot-6 ROM 残留状态
- **问题**: 先加载 20K ROM 再加载 12K ROM 时，slot-6 ROM 内容不会清空
- **文件**: `a2vm-core/src/machine.rs`, `a2vm-core/src/disk.rs`
- **动作**:
  - [ ] 在 `disk.rs` 新增 `DiskII::clear_slot_rom()` 方法
  - [ ] 在 `machine.rs` 12K ROM 分支调用 `clear_slot_rom()`
  - [ ] 新增测试验证残留问题已修复

### P1.5 CPU 中断 cycle 计数不一致
- **问题**: 普通指令 cycle 计数在 `step()` 中统一累加，但 `handle_nmi()`/`handle_irq()` 在内部直接累加
- **文件**: `a2vm-core/src/cpu/mod.rs`
- **动作**:
  - [ ] 修改中断处理器只返回 cycle 数，不内部累加
  - [ ] 由 `step()` 统一负责 `self.cycles += ...`

### P1.6 GUI 初始化错误使用 `process::exit` 而非错误传播
- **问题**: `App::new()` 在加载失败时直接 `process::exit(1)`，绕过 Drop 清理
- **文件**: `a2vm-gui/src/main.rs`
- **动作**:
  - [ ] 将 `App::new()` 改为返回 `Result<Self, E>`
  - [ ] 错误统一到 `main()` 中处理

---

## P2 — 架构优化（可维护性/性能）

### P2.1 抽取 TUI/GUI 共享运行时逻辑到 `a2vm-oxide`
- **问题**: 两个前端有大量重复的仿真循环、音频、性能统计代码
- **文件**: `a2vm-tui/src/main.rs`, `a2vm-gui/src/main.rs`, `a2vm-oxide/src/*`
- **动作**:
  - [ ] 在 `a2vm-oxide` 创建 `EmulatorRunner` 结构体
  - [ ] 抽取共享的 emulation tick、turbo、cycle_accum 逻辑
  - [ ] 抽取共享的音频输出逻辑（PCM 采集 + Sink 提交）
  - [ ] 抽取共享的机械噪声处理逻辑
  - [ ] 抽取共享的性能统计（MHz 计算）
  - [ ] 抽取共享的初始化流程（ROM/disk/reset）
- **验收**: 两端重复逻辑显著减少，功能行为不变

### P2.2 Workspace 依赖统一管理
- **问题**: `clap` 和 `rodio` 在各 crate 中独立声明版本
- **文件**: `Cargo.toml`（workspace root）
- **动作**:
  - [ ] 在 workspace root 添加 `[workspace.dependencies]`
  - [ ] 各 crate 改为引用 `{ workspace = true }`

### P2.3 优化 `SharedArgs::rom_data()` 内存拷贝
- **问题**: 每次调用 `to_vec()` 分配并拷贝 20KB 嵌入 ROM
- **文件**: `a2vm-oxide/src/cli.rs`
- **动作**:
  - [ ] 返回类型改为 `Cow<'static, [u8]>`
  - [ ] 嵌入 ROM 走 `Cow::Borrowed`

### P2.4 替换魔法数字为命名常量
- **问题**: ROM 大小、IOB 偏移量等使用裸十六进制
- **文件**: `a2vm-core/src/machine.rs`, `a2vm-core/src/disk.rs`
- **动作**:
  - [ ] `ROM_SIZE_12K = 0x3000`, `ROM_SIZE_20K = 0x5000`
  - [ ] `IOB_OFFSET_COMMAND = 0x0C`, `IOB_OFFSET_TRACK = 0x04` 等

### P2.5 优化 `fill_rect` 效率
- **问题**: Lo-Res 7×4 块逐像素调用 `set_pixel`（含除法/取模）
- **文件**: `a2vm-core/src/video.rs`
- **动作**:
  - [ ] 对齐情况改用批量字节操作

### P2.6 为 `Error` 实现 `source()` 错误链
- **问题**: `Error::Io` 包装了 `io::Error` 但未提供 `source()` 方法
- **文件**: `a2vm-core/src/error.rs`
- **动作**:
  - [ ] 实现 `fn source(&self) -> Option<&(dyn std::error::Error + 'static)>`

---

## P3 — 小改进（代码质量/工程规范）

### P3.1 测试临时文件 RAII 清理
- **问题**: 测试使用手动 `fs::remove_file()`，panic 时文件残留
- **文件**: `a2vm-core/src/machine.rs`, `a2vm-core/src/disk.rs`（tests）
- **动作**:
  - [ ] 引入 `TempFile` RAII 守卫或 `tempfile` crate

### P3.2 处理 `--no-default-features` 下 TUI 未使用字段告警
- **问题**: `noise` 字段在非 audio 编译时产生 dead_code 告警
- **文件**: `a2vm-tui/src/main.rs:125`
- **动作**:
  - [ ] 添加 `#[cfg(feature = "audio")]` 条件编译

### P3.3 修复 `render_status_bar` 小写字母映射
- **问题**: 小写字母（0x61-0x7A）全部映射到 '@'
- **文件**: `a2vm-core/src/video.rs:578-607`
- **动作**:
  - [ ] 添加 `0x60..0x80` 范围映射到与大写相同字形

### P3.4 移除 `AppleKey::Space` 冗余变体
- **问题**: `Space` 与 `Printable(' ')` 语义重叠
- **文件**: `a2vm-core/src/keyboard.rs`
- **动作**:
  - [ ] 移除 `Space` 变体，统一走 `Printable(' ')`
  - [ ] 更新前端映射

### P3.5 移除不必要的 `#[rustfmt::skip]`
- **问题**: `opcodes.rs` 中 `Mnemonic` 枚举的 `#[rustfmt::skip]` 降低可读性
- **文件**: `a2vm-core/src/cpu/opcodes.rs:4`
- **动作**:
  - [ ] 移除 `#[rustfmt::skip]`

### P3.6 添加文档注释
- **问题**: opcode 表、CHAR_ROM、sector interleave 缺少数据来源
- **文件**: `a2vm-core/src/cpu/opcodes.rs`, `a2vm-core/src/video.rs`, `a2vm-core/src/disk.rs`
- **动作**:
  - [ ] 添加数据来源和实现参考注释

### P3.7 检查空模块 `memory.rs`
- **问题**: `a2vm-core/src/memory.rs` 可能为空或无用
- **文件**: `a2vm-core/src/memory.rs`
- **动作**:
  - [ ] 确认内容，如无用则删除

### P3.8 替换 `unreachable!` 使用
- **问题**: `read_operand` 等函数使用 `unreachable!` 而非安全错误处理
- **文件**: `a2vm-core/src/cpu/mod.rs:359`
- **动作**:
  - [ ] 使用 `debug_assert!` 配合默认返回值，或改用 `Option` 类型

---

## 验证命令

```bash
# 运行全部测试
cargo test

# 运行核心库测试
cargo test -p a2vm-core

# 运行 Klaus Dormann 功能测试
cargo test klaus_dormann

# 无音频特性构建测试
cargo build --no-default-features -p a2vm-tui
cargo build --no-default-features -p a2vm-gui
```

---

## 实施建议顺序

1. **立即处理 P0**: 修复 SAX 操作码和磁盘持久化问题
2. **本轮迭代 P1**: 统一 disk.tick 语义、TUI 终端清理、ROM 状态管理
3. **维护窗口 P2**: 抽取共享运行时、优化性能、统一管理依赖
4. **随时处理 P3**: 小改进可分散在日常提交中
