# A2VM 开发路线图

文档版本: 2026-02-11  
本文档整合了 code-review.md、gui-plan.md、improvement-plan.md 和 milestones.md 的内容。

---

## 一、已完成的里程碑

### M1 - 6502 CPU 功能验证
- 通过 Klaus Dormann 功能测试
- 正确建模 NMOS 陷阱（BCD 行为、JMP 间接页环绕、BRK/RTI/RTS 语义）

### M2 - Apple II 机器集成
- `AppleII` 总线/内存集成
- ROM 加载和复位向量启动
- 键盘锁存/选通和按键注入路径

### M3 - Applesoft/Monitor 执行路径
- ROM 启动流程到达 Monitor/BASIC 路径
- 前端键盘映射支持交互式输入

### M4 - 视频管线（TEXT/GR/HGR）
- 文本/图形模式的核心位图渲染器
- 支持彩色/单色变体的 GUI RGBA 管线
- TUI 盲文转换显示

### M5 - Disk II 读取路径 + Fast-Disk 陷阱
- `.dsk` 加载和 nibblized 磁道访问
- slot ROM 映射和 Disk II I/O 开关
- fast-disk RWTS 读取陷阱

### M6 - 扬声器音频输出
- `$C030` 边沿时间线捕获
- 从周期时间戳生成 PCM
- 两个前端都支持可选的 rodio 播放

---

## 二、代码审查项目状态

### 已完成项目（15/20）

| # | 项目 | 说明 |
|---|------|------|
| 1 | `run_cycles` fast-disk 路径调用 `disk.tick()` | fast-disk 分支现在正确调用 disk.tick() |
| 2 | `read_operand` / `addr_of` 静默返回 0 | 改为 `unreachable!()` 暴露异常路径 |
| 4 | 缺少自定义错误类型 | 已添加 `a2vm_core::Error` 枚举 |
| 5 | I/O 地址匹配依赖顺序 | 拆分为不重叠区间，降低顺序依赖风险 |
| 8 | 缺少反汇编器 | 新增 `cpu/disasm.rs`，使用 `peek` 无副作用读取 |
| 9 | 缺少 CPU 指令级单元测试 | 新增 `cpu/tests.rs`，覆盖 ADC/SBC BCD、移位、分支跨页等 |
| 10 | 集成测试依赖外部 ROM 文件 | 添加 `require_paths()` 检查，文件不存在时跳过 |
| 11 | HGR 颜色渲染重复计算行地址 | 复用外层循环已计算的 `addr` |
| 13 | `pixels.render().ok()` 静默吞噬错误 | 改为显式错误日志输出 |
| 14 | `Status` 标志位类型安全性 | 新增 `Flag` 枚举，`set(flag: Flag, val: bool)` |
| 16 | `disk_controller_enabled` 默认值 | 默认改为 `false`，加载磁盘时启用 |
| 17 | `CHAR_ROM` 硬编码与来源说明 | 已补充数据来源说明 |
| 18 | `architecture.md` 与代码不同步 | 已更新为当前结构 |
| 19 | `milestones.md` 未更新 | 已更新里程碑状态 |
| 20 | `encode_6and2` 魔法数字较多 | 添加 `AUX_BYTES`、`MAIN_BYTES`、`TOTAL_NIBBLES` 等命名常量 |

### 部分完成项目（3/20）

| # | 项目 | 状态 | 说明 |
|---|------|------|------|
| 6 | ILL（非法操作码）静默忽略 | 短期完成 | 已添加 `#[cfg(debug_assertions)]` 日志输出非法操作码和 PC 地址，常用非法操作码（LAX/SAX/DCP 等）尚未实现 |
| 7 | Disk II 不支持写入 | 部分完成 | 已支持 RWTS 写入路径（`write_sector_raw`），完整的 nibble/raw 双向同步可作为后续里程碑 |
| 12 | `render_until` 每次调用分配新 Vec | API 完成 | 已提供 `render_until_into()` 复用缓冲区 API，前端调用侧可继续迁移 |

### 暂停/未处理项目（2/20）

| # | 项目 | 状态 | 说明 |
|---|------|------|------|
| 3 | TUI 与 GUI 大量重复代码 | 暂停 | 按当前决策暂停，不放入 a2vm-core，如恢复建议拆到独立 workspace package `a2vm-frontend-common` |
| 15 | `Cpu` 所有字段 `pub` | 未处理 | 当前规模下可用，封装性可后续提升 |

---

## 三、改进计划执行状态

### Phase A: 快速修复（6 项）- 已完成

- **A1** `read_operand`/`addr_of` 防御性分支 - 改为 `unreachable!()`
- **A2** `disk_controller_enabled` 默认改为 false
- **A3** I/O 地址匹配去重叠
- **A4** fast-disk 路径补齐 `disk.tick()`
- **A5** HGR 渲染复用行地址
- **A6** `pixels.render()` 错误日志

### Phase B: 代码去重 - 暂停

将 TUI/GUI 间重复的常量、时序逻辑、CLI 解析提取到独立 package `a2vm-frontend-common/`，避免污染 `a2vm-core` 边界。

**当前状态**: 按决策暂停，不放入 a2vm-core。如后续恢复，建议拆到独立 workspace package。

### Phase C: 新功能 - 已完成

- **C1** ILL 操作码日志（短期目标）- 已完成
- **C2** 反汇编器 - 已完成，`cpu/disasm.rs` 已落地并有测试覆盖

### Phase D: 文档更新 - 已完成

- **D1** 更新 `architecture.md` - 已更新为 TUI/GUI + a2vm-core 结构
- **D2** 更新 `milestones.md` - M1-M6 标记为 COMPLETE，添加 M7
- **D3** `encode_6and2` 命名常量 - 已添加命名常量

---

## 四、GUI 实施计划

**状态**: 已完成落地

`a2vm-gui` 已实现并并入 workspace：

1. **video.rs RGBA 彩色渲染** - 已完成
   - `render_rgba()` 函数
   - `LORES_PALETTE` 16 色调色板
   - Hi-Res NTSC 伪彩色支持
   - 文本磷光色

2. **创建 a2vm-gui crate** - 已完成
   - 依赖 pixels, winit, rodio(optional)
   - 已并入 workspace

3. **GUI main.rs 实现** - 已完成
   - 窗口、事件循环、渲染、输入、音频
   - CLI 参数支持 `--color-mode`
   - 热键支持（Ctrl+Q/R/T）

---

## 五、后续工作项

### 当前未完成/部分完成

| 项 | 优先级 | 说明 |
|----|--------|------|
| TUI/GUI 去重 | 低 | 按当前决策暂停 |
| 非法操作码完整支持 | ✅ 完成 | 已实现 8 个常用非法操作码：LAX/SAX/DCP/ISC/SLO/RLA/RRA/SRE |
| 磁盘写入完整链路 | 中 | 已支持 RWTS 写入路径，完善 nibble/raw 双向同步可作为后续里程碑 |
| 音频缓冲复用落地 | 低 | 已提供复用 API，前端调用侧可继续迁移 |
| Cpu pub 字段封装 | 低 | 当前可用但封装性仍可提升 |

### 未来候选功能

- 更广泛的非官方操作码兼容性（超出 debug 日志）
- 周期精确的磁盘时序模型
- 80 列/扩展视频硬件支持
- TUI/GUI 调试器覆盖层
- 快照/保存状态支持

---

## 六、验证命令

```bash
# 运行所有测试
cargo test

# 编译前端
cargo build -p a2vm-tui -p a2vm-gui

# 运行 GUI（需要 ROM）
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --fast-disk "disks/Apple DOS 3.3 January 1983.dsk"
```
