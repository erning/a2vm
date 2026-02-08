# A2VM — Milestones

## M1: 6502 CPU Functional Test ← 当前

**目标**: 6502 CPU 通过 Klaus Dormann functional test suite。

**范围**: 只需 CPU + FlatMemory，不需要任何 Apple II 硬件。

| 步骤 | 文件 | 内容 |
|------|------|------|
| 1 | `Cargo.toml` x2 | workspace + crate 配置 |
| 2 | `lib.rs` | 模块声明 |
| 3 | `bus.rs` | Bus trait |
| 4 | `memory.rs` | FlatMemory (64K RAM) |
| 5 | `cpu/status.rs` | Status register 封装 |
| 6 | `cpu/addressing.rs` | 13 种寻址模式 |
| 7 | `cpu/opcodes.rs` | 256 项 opcode 表 |
| 8 | `cpu/mod.rs` | Cpu struct + 全部指令实现 |
| 9 | `tests/klaus_dormann.rs` | 集成测试 |

**验证方案**:
- 加载 `6502_functional_test.bin` 到 FlatMemory
- PC 从 $0400 开始执行
- 成功: PC 停在 ~$3469（成功 trap）
- 失败: PC 停在其他地址（具体地址可定位失败的测试项）

---

## M2: Apple II Monitor

**目标**: 加载 Apple II ROM，进入 Monitor（`*` 提示符），能执行基本命令。

**新增模块**:
- `machine.rs` — AppleII struct, Bus 地址解码
- `memory.rs` — AppleIIMemory (48K RAM + Language Card)
- `keyboard.rs` — 键盘锁存/选通
- `softswitch.rs` — 软开关状态
- `video.rs` — 40 列文本模式渲染
- `ffi.rs` — C FFI 接口
- Swift app 基础框架

**验证**: 能看到 `*` 提示符，输入地址查看/修改内存。

---

## M3: Applesoft BASIC

**目标**: 运行 Applesoft BASIC，能执行简单 BASIC 程序。

**新增**:
- 完整键盘映射
- 光标闪烁
- BASIC ROM 加载

**验证**: `10 PRINT "HELLO" / 20 GOTO 10 / RUN` 正常执行。

---

## M4: HiRes Graphics

**目标**: 支持 Lo-Res 和 Hi-Res 图形模式。

**新增**:
- `video.rs` — Lo-Res (40x48) + Hi-Res (280x192) 渲染
- 混合模式（文本 + 图形）
- 颜色处理

**验证**: 能运行 HiRes 演示程序。

---

## M5: Disk II

**目标**: 支持 Disk II 磁盘驱动器，能从 .dsk/.woz 镜像启动。

**新增**:
- `disk.rs` — Disk II 控制器模拟
- .dsk / .woz 镜像格式解析
- Slot 6 ROM

**验证**: 从 DOS 3.3 磁盘镜像启动，能 `CATALOG` 和 `RUN` 程序。

---

## M6: Sound

**目标**: 扬声器音频输出。

**新增**:
- `audio.rs` — toggle timestamps → PCM 采样
- Swift AudioUnit 播放

**验证**: 能听到 `PEEK(-16336)` 产生的声音，程序音效正常。

---

## 后续可能

- 80 列卡 (//e)
- 双 Hi-Res
- MockingBoard 音频卡
- 打印机模拟
- 存档/快照
- 调试器 UI
