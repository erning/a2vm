# A2VM 综合改进计划

> **来源**: 综合 Claude、Codex、Kimi、Zhipu 四份代码审查报告  
> **更新日期**: 2026-02-12

---

## 审查共识汇总

| 审查者 | 发现问题数 | 重点领域 | 评分 |
|--------|-----------|----------|------|
| Claude | 17 | 正确性、数据安全、代码重复 | 未评分 |
| Codex | 12 | 与 Claude 高度重合，按优先级整理 | 未评分 |
| Kimi | 12 | 架构、文档、魔法数字 | 4.3/5.0 |
| Zhipu | 15 | 架构设计、API 文档、测试覆盖 | 8.1/10 |

**四份审查的一致结论**:
1. ✅ 架构设计优秀（Bus trait、CPU-Bus 分离）
2. ✅ 6502 实现正确（BCD、JMP indirect bug、非法指令）
3. ⚠️ SAX 操作码缺失是共识性 P0 问题
4. ⚠️ 磁盘 nibble 写入不持久化是共识性 P0 问题
5. ⚠️ TUI/GUI 代码重复是共识性改进点
6. ⚠️ 文档不足（API 文档、数据来源注释）

---

## P0 — 必须修复（数据安全 / 功能正确性）

### 1. SAX 非法指令操作码缺失 + 假阳性测试

**发现者**: Claude, Codex, Zhipu  
**风险**: 依赖这些指令的软件行为错误；现有测试不能有效拦截回归

- [ ] `a2vm-core/src/cpu/opcodes.rs`: 补充缺失的 SAX 操作码
  - `0x83` → `op(SAX, IndirectX, 6, false)`
  - `0x87` → `op(SAX, ZeroPage, 3, false)`
  - `0x8F` → `op(SAX, Absolute, 4, false)`

- [ ] `a2vm-core/src/cpu/tests.rs`: 修复 `sax_stores_a_and_x` 假阳性测试
  - 改用非零期望值（如 `cpu.a = 0xFF, cpu.x = 0x0F`，期望 `0x0F`）
  
- [ ] 为 `0x83`、`0x8F` 新增测试用例

**验证命令**:
```bash
cargo test -p a2vm-core sax
cargo test klaus_dormann
```

---

### 2. Disk II nibble 写入不持久化

**发现者**: Claude, Codex  
**风险**: 通过 Q6+Q7 写模式的磁盘修改在退出时丢失

- [ ] `a2vm-core/src/disk.rs`: 电机关闭路径触发同步
  ```rust
  // handle_switch 0x08 (motor off)
  if self.motor_on {
      let _ = self.sync_nibble_to_raw(self.selected_drive);
  }
  ```

- [ ] `a2vm-core/src/disk.rs`: 新增公开方法
  ```rust
  pub fn flush_drive(&mut self, drive: usize) -> Result<()>;
  pub fn flush_all_drives(&mut self) -> Result<()>;
  ```

- [ ] `a2vm-tui/src/main.rs`: 退出时调用 `flush_all_drives()`

- [ ] `a2vm-gui/src/main.rs`: 退出时调用 `flush_all_drives()`

- [ ] 新增测试: Q6+Q7 写入 → motor off → 验证 `.dsk` 文件已更新

---

## P1 — 重要改进（稳定性 / 一致性）

### 3. `run_cycles()` 普通路径未驱动 `disk.tick()`

**发现者**: Claude, Codex  
**风险**: 后续实现周期级磁盘时序时，`step()` 与 `run_cycles()` 行为分叉

- [ ] `a2vm-core/src/machine.rs`: 统一执行路径
  - 方案 A: 非 fast-disk 分支改为按指令循环，每条指令后 `disk.tick()`
  - 方案 B: 引入 `cpu.run_with_hook()` 统一处理外设 tick

---

### 4. video_dirty 检测范围过宽

**发现者**: Claude, Codex  
**风险**: 对非视频 RAM 的写操作触发不必要重绘

- [ ] `a2vm-core/src/machine.rs:124`: 收窄检测范围
  ```rust
  // 当前: (0x0400..0x6000).contains(&addr)
  // 改为:
  let is_video = (0x0400..0x0C00).contains(&addr)  // TEXT/GR Page 1&2
      || (0x2000..0x6000).contains(&addr);         // HGR Page 1&2
  ```

---

### 5. TUI 错误路径遗留 raw mode

**发现者**: Claude, Codex  
**风险**: 终端状态污染，影响后续 shell 使用

- [ ] `a2vm-tui/src/main.rs`: 引入 RAII 终端守卫
  ```rust
  struct TerminalGuard;
  impl Drop for TerminalGuard {
      fn drop(&mut self) {
          let _ = terminal::disable_raw_mode();
          let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
      }
  }
  ```

---

### 6. 12K ROM 加载未清理 slot-6 ROM 残留

**发现者**: Claude, Codex  
**风险**: 同实例先加载 20K 再加载 12K 时，slot ROM 残留旧数据

- [ ] `a2vm-core/src/disk.rs`: 新增方法
  ```rust
  pub fn clear_slot_rom(&mut self) {
      self.slot_rom.fill(0);
      self.slot_rom_loaded = false;
  }
  ```

- [ ] `a2vm-core/src/machine.rs`: 12K 分支调用 `clear_slot_rom()`

- [ ] 新增测试: 先加载 20K 再加载 12K，验证 `$C600` 返回 0

---

## P2 — 架构优化（可维护性）

### 7. TUI/GUI 代码重复

**发现者**: Claude, Codex, Kimi  
**风险**: 维护成本高，两端行为可能分叉

- [ ] `a2vm-oxide/src/emulator.rs`: 提取共享运行器
  ```rust
  pub struct EmulatorRunner {
      pub apple: AppleII,
      cycle_accum: u128,
      last_tick: Instant,
      turbo: bool,
      // ...
  }
  
  impl EmulatorRunner {
      pub fn tick(&mut self, dt: Duration) -> u64;
      pub fn take_audio(&mut self, sample_rate: u32, real_cycles: u64) -> Vec<f32>;
      pub fn check_mechanical_event(&mut self) -> Option<MechanicalEvent>;
      pub fn perf_stats(&self) -> (f64, u64);  // (MHz, cycles)
  }
  ```

- [ ] `a2vm-tui/src/main.rs`: 使用 `EmulatorRunner`

- [ ] `a2vm-gui/src/main.rs`: 使用 `EmulatorRunner`

---

### 8. CPU 中断 cycle 计数不一致

**发现者**: Claude  
**风险**: 代码风格不统一，潜在维护陷阱

- [ ] `a2vm-core/src/cpu/mod.rs`: 统一计数方式
  - `handle_nmi()` / `handle_irq()` 只返回 cycle 数
  - `step()` 统一累加 `self.cycles`

---

### 9. Workspace 依赖版本统一管理

**发现者**: Claude, Kimi  
**风险**: 版本不一致可能导致兼容性问题

- [ ] `Cargo.toml` (workspace root): 添加 workspace dependencies
  ```toml
  [workspace.dependencies]
  clap = { version = "4.5", features = ["derive"] }
  rodio = { version = "0.21" }
  ratatui = "0.29"
  crossterm = "0.28"
  pixels = "0.15"
  winit = "0.30"
  a2vm-core = { path = "a2vm-core" }
  a2vm-oxide = { path = "a2vm-oxide" }
  ```

- [ ] 各 crate 的 `Cargo.toml` 改为 `{ workspace = true }`

---

## P3 — 工程质量改进

### 10. 增加 API 文档

**发现者**: Zhipu, Kimi  
**优先级**: 高

- [ ] `a2vm-core/src/cpu/mod.rs`: 为公共方法添加 `///` 文档
- [ ] `a2vm-core/src/machine.rs`: 为公共方法添加文档
- [ ] `a2vm-core/src/disk.rs`: 为公共方法添加文档
- [ ] `a2vm-core/src/video.rs`: 为公共函数添加文档
- [ ] `a2vm-core/src/bus.rs`: 为 trait 方法添加文档

---

### 11. 消除魔法数字

**发现者**: Kimi  
**优先级**: 中

- [ ] `a2vm-core/src/machine.rs`: ROM 大小常量
  ```rust
  const ROM_SIZE_12K: usize = 0x3000;
  const ROM_SIZE_20K: usize = 0x5000;
  ```

- [ ] `a2vm-core/src/machine.rs`: IOB 字段偏移常量
  ```rust
  const IOB_OFFSET_COMMAND: u16 = 0x0C;
  const IOB_OFFSET_TRACK: u16 = 0x04;
  const IOB_OFFSET_SECTOR: u16 = 0x05;
  const IOB_OFFSET_BUFFER: u16 = 0x08;
  ```

- [ ] `a2vm-core/src/disk.rs`: 磁盘参数常量
  ```rust
  const MAX_HALF_TRACK: u8 = 69;
  const MAX_TRACKS: usize = 35;
  const SECTORS_PER_TRACK: usize = 16;
  ```

---

### 12. `Error` 类型增强

**发现者**: Claude, Zhipu  
**优先级**: 中

- [ ] `a2vm-core/src/error.rs`: 实现 `source()` 错误链
  ```rust
  impl std::error::Error for Error {
      fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
          match self {
              Error::Io(err) => Some(err),
              _ => None,
          }
      }
  }
  ```

---

### 13. GUI 初始化错误处理

**发现者**: Claude, Codex  
**优先级**: 中

- [ ] `a2vm-gui/src/main.rs`: `App::new()` 改为返回 `Result<Self, AppError>`
- [ ] 移除 `process::exit(1)` 调用
- [ ] 在 `main()` 中统一错误处理

---

### 14. 测试临时文件清理

**发现者**: Claude, Codex  
**优先级**: 低

- [ ] `a2vm-core/src/disk.rs` (tests): 引入 RAII 守卫或 `tempfile` crate
- [ ] `a2vm-core/src/machine.rs` (tests): 同上

---

### 15. 替换 `unreachable!` 为安全处理

**发现者**: Zhipu  
**优先级**: 低

- [ ] `a2vm-core/src/cpu/mod.rs:359`: `read_operand` 中的 `unreachable!`
  ```rust
  // 改为 debug_assert! + 默认返回值
  fn read_operand<B: Bus>(&self, resolved: &Resolved, bus: &mut B) -> u8 {
      match resolved.operand {
          Operand::Address(addr) => bus.read(addr),
          Operand::Byte(b) => b,  // Immediate 模式
      }
  }
  ```

---

### 16. 其他小改进

- [ ] `a2vm-oxide/src/cli.rs`: `rom_data()` 改为返回 `Cow<'static, [u8]>` 避免不必要拷贝
- [ ] `a2vm-core/src/video.rs`: `render_status_bar` 添加小写字母映射
- [ ] `a2vm-core/src/video.rs`: `fill_rect` 优化为批量字节操作
- [ ] `a2vm-tui/src/main.rs`: `noise` 字段加 `#[cfg(feature = "audio")]`
- [ ] `a2vm-core/src/keyboard.rs`: 移除 `AppleKey::Space` 冗余变体

---

## P4 — 未来增强（可选）

### 17. 增加边界测试

**发现者**: Zhipu  
**优先级**: 未来

- [ ] 磁盘写入后的持久化验证
- [ ] 不同 ROM 大小（12K vs 20K）的加载测试
- [ ] 视频模式切换的边界条件测试

---

### 18. 性能基准测试

**发现者**: Zhipu  
**优先级**: 未来

- [ ] 添加 `#[bench]` 测试防止性能回归
  ```rust
  #[bench]
  fn bench_cpu_step(b: &mut test::Bencher);
  
  #[bench]
  fn bench_video_render(b: &mut test::Bencher);
  ```

---

### 19. 调试功能

**发现者**: Kimi  
**优先级**: 未来

- [ ] 添加断点支持
- [ ] 添加单步执行模式
- [ ] 添加内存查看器
- [ ] 添加保存/加载状态功能

---

### 20. `no_std` 支持

**发现者**: Zhipu  
**优先级**: 未来

- [ ] 评估核心库 `no_std` 可行性
- [ ] 分离 `std` 依赖特性

---

## 执行计划

### 第一阶段（1-2 天）: P0 修复
1. 修复 SAX 操作码缺失（#1）
2. 修复磁盘 nibble 写入持久化（#2）

### 第二阶段（2-3 天）: P1 改进
3. 统一 disk.tick() 语义（#3）
4. 收窄 video_dirty 范围（#4）
5. TUI 终端状态 RAII（#5）
6. 修复 12K ROM 加载残留（#6）

### 第三阶段（1 周）: P2 架构优化
7. 提取共享运行器（#7）
8. 统一 cycle 计数（#8）
9. 统一依赖版本（#9）

### 第四阶段（持续）: P3/P4 工程质量
10. API 文档（#10）
11. 消除魔法数字（#11）
12. 错误处理增强（#12-14）
13. 小改进累积（#15-16）

---

## 验证命令

每次修改后运行:
```bash
# 全量测试
cargo test

# 核心库测试
cargo test -p a2vm-core

# CPU 功能测试
cargo test klaus_dormann

# 编译检查
cargo build --release
cargo build -p a2vm-tui --no-default-features
cargo build -p a2vm-gui --no-default-features

# Clippy 静态分析
cargo clippy --all-targets -- -D warnings
```

---

## 变更日志

| 日期 | 变更 |
|------|------|
| 2026-02-12 | 初始版本，综合四份审查报告 |
