# Plan: a2vm-gui — pixels + winit 图形界面

## Context

现有 TUI 前端使用 Braille 字符渲染 280×192 单色画面，无法显示 Apple II 的真实色彩。
目标：用 `pixels` + `winit` 创建 GUI 前端，支持全彩色渲染（Lo-Res 16色、Hi-Res NTSC伪彩、文本绿色磷光），功能与 TUI 对等。

## 状态

- ✅ 本计划已完成落地（`a2vm-gui` 已实现并并入 workspace）。
- ✅ 文中 Step 1/2/3 均已实现。
- ℹ️ 本文现作为实施记录保留。

## 修改文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `a2vm-core/src/video.rs` | 修改 | 添加 `render_rgba()` 及色彩常量 |
| `Cargo.toml` (workspace) | 修改 | members 加入 `"a2vm-gui"` |
| `a2vm-gui/Cargo.toml` | 新建 | 依赖 pixels, winit, rodio(optional), a2vm-core |
| `a2vm-gui/src/main.rs` | 新建 | 窗口、事件循环、渲染、输入、音频 |

## Step 1: 在 video.rs 添加 RGBA 彩色渲染（✅ 已完成）

在 `a2vm-core/src/video.rs` 中添加：

**常量：**
- `RGBA_WIDTH = 280`, `RGBA_HEIGHT = 192`, `RGBA_SIZE = 280*192*4`
- `LORES_PALETTE: [[u8;4]; 16]` — Apple II 标准 16 色 RGBA
- Hi-Res 伪彩色常量：PURPLE, GREEN, BLUE, ORANGE, WHITE, BLACK
- 文本磷光色：`TEXT_FG = [0x33, 0xFF, 0x33, 0xFF]`, `TEXT_BG = [0,0,0,0xFF]`

**函数：**
```rust
pub fn render_rgba(
    ram: &[u8],
    mode: &DisplayMode,
    flash_on: bool,
    color_mode: DisplayColorMode,
    frame_phase: u64,
    rgba: &mut [u8],
)
```
内部分派到三个模式渲染器：

1. **`render_text_rows_rgba`** — 复用 `CHAR_ROM` 查表，像素写入 `TEXT_FG`/`TEXT_BG`
2. **`render_lores_rows_rgba`** — 读取 nibble 颜色索引，查 `LORES_PALETTE[color]`，填充 7×4 块
3. **`render_hires_scanlines_rgba`** — NTSC 伪彩色：
   - bit7=0: even col→Purple, odd col→Green
   - bit7=1: even col→Blue, odd col→Orange
   - 相邻两个 ON 像素→White; OFF→Black

**状态输出：**
- GUI 不再在窗口内绘制状态栏，状态信息输出到控制台（stderr）并在同一行刷新。

## Step 2: 创建 a2vm-gui crate（✅ 已完成）

**`a2vm-gui/Cargo.toml`：**
```toml
[package]
name = "a2vm-gui"
version = "0.1.0"
edition = "2021"

[features]
default = ["audio"]
audio = ["dep:rodio"]

[dependencies]
a2vm-core = { path = "../a2vm-core" }
pixels = "0.15"
winit = "0.30"
rodio = { version = "0.21", optional = true }
```

workspace `Cargo.toml` 加入 `"a2vm-gui"`。

## Step 3: GUI main.rs 实现（✅ 已完成）

结构参照 TUI (`a2vm-tui/src/main.rs`, 427行)，单文件。

### 窗口设置
- 逻辑分辨率：280×192（仅显示区域，无状态栏）
- 默认窗口：3× 缩放 = 840×576
- `pixels` 自动 GPU 缩放

### App 结构体 + ApplicationHandler
```rust
struct App {
    apple: AppleII,
    pixels: Option<Pixels>,
    window: Option<Arc<Window>>,
    rgba_buf: Vec<u8>,        // 280*192*4
    // 时序状态（同 TUI）
    last_tick: Instant,
    cycle_accum: u128,
    turbo: bool,
    emu_mhz: f64,
    perf_last_time: Instant,
    perf_last_cycles: u64,
    flash_on: bool,
    flash_last_toggle: Instant,
    // 音频
    #[cfg(feature = "audio")]
    audio_sink: Option<Sink>,
    fast_disk: bool,
    modifiers: ModifiersState,
}
```

### 事件循环映射
| winit 事件 | 处理 |
|------------|------|
| `resumed()` | 创建 Window + Pixels |
| `about_to_wait()` | CPU 执行 + 音频采样 + request_redraw + ControlFlow::WaitUntil |
| `RedrawRequested` | render_rgba → copy to pixels frame → pixels.render() |
| `KeyboardInput` | 热键(Ctrl+Q/R/T) + map_key → apple.key_press() |
| `ModifiersChanged` | 更新 modifiers 状态 |
| `Resized` | pixels.resize_surface() |
| `CloseRequested` | exit |

### 键盘映射
复用 TUI 的映射逻辑，改用 winit 类型：
- `Key::Character` → ASCII（自动大写）
- Ctrl+A-Z → 0x01-0x1A
- Enter=0x0D, Backspace=0x08, 方向键, Esc, Tab
- 忽略 Alt 修饰键

### 时序控制
- `about_to_wait`: 计算 `dt * CPU_HZ` 累加器驱动 CPU 周期
- Turbo: 4× 乘数
- 帧率：`ControlFlow::WaitUntil(now + 16.667ms)` (60fps)
- Flash: 每 267ms 翻转

### 音频
与 TUI 相同：rodio OutputStreamBuilder + Sink，在 about_to_wait 中取样。

### CLI 参数
- 共享参数：`--rom`, `--disk`, `--fast-disk`, `--help`，支持 `A2VM_ROM` 环境变量。
- GUI 额外参数：`--color-mode <mode>`，可选 `color`（默认）、`mono`、`mono-scanlines`。

## 实现顺序（✅ 已完成）

1. **video.rs** — 添加 render_rgba + 调色板 + 状态栏渲染
2. **创建 crate 骨架** — Cargo.toml + 最小 main.rs（打开黑色窗口）
3. **集成 pixels** — 测试 RGBA 帧缓冲区管线
4. **连接 AppleII** — ROM 加载、CPU 执行、render_rgba 渲染
5. **键盘输入** — 完成交互
6. **音频** — rodio 集成
7. **状态输出** — 控制台同一行输出运行状态信息

## 验证

```bash
# 编译
cargo build -p a2vm-gui

# 运行（需要 ROM）
cargo run -p a2vm-gui -- --rom roms/apple2p.rom

# 带磁盘
cargo run -p a2vm-gui -- --rom roms/apple2p.rom --disk "disks/DOS3.3.dsk"

# 确认 TUI 未受影响
cargo build -p a2vm-tui
cargo test -p a2vm-core
```
