# A2VM Code Review

## 1. Bug: SAX 非法指令操作码表缺失

**文件:** `a2vm-core/src/cpu/opcodes.rs`

SAX 指令只实现了 `0x97` (ZeroPageY) 和 `0x9F` (AbsoluteY)，但遗漏了三个常用变体：

| 操作码 | 寻址模式 | 当前状态 |
|--------|----------|---------|
| `0x83` | (Indirect,X) | `ill()` — 应为 `op(SAX, IndirectX, 6, false)` |
| `0x87` | ZeroPage | `ill()` — 应为 `op(SAX, ZeroPage, 3, false)` |
| `0x8F` | Absolute | `ill()` — 应为 `op(SAX, Absolute, 4, false)` |

**相关测试问题:** `a2vm-core/src/cpu/tests.rs` 中的 `sax_stores_a_and_x` 测试使用了操作码 `0x87`，但因 `0x87` 在表中为 `ill()`，该指令实际以 NOP 方式执行。测试断言 `mem.data[0x0010] == 0x00` 能通过，仅因为 `0xF0 & 0x0F == 0x00` 恰好等于内存的初始值 0。这是一个**假阳性测试**——将 `cpu.a` 改为 `0xFF` 即可暴露此 bug。

---

## 2. TUI / GUI 大量重复代码

**文件:** `a2vm-tui/src/main.rs`、`a2vm-gui/src/main.rs`

以下逻辑在两个前端中几乎完全相同：

- **模拟循环** (`run_emulation`)：delta-time 计算、cycle_accum 累加、turbo 倍率、run_cycles 调用
- **音频输出**：Speaker PCM 采集 → SamplesBuffer → Sink::append
- **机械噪声**：DiskMechTracker::check → match event → Decoder/repeat_infinite
- **性能统计**：perf_last_time / perf_last_cycles → MHz 计算
- **初始化流程**：ROM 加载 → disk 挂载 → fast_disk → reset → audio stream 创建

建议将共享逻辑提取到 `a2vm-oxide` 中，例如：

```rust
// a2vm-oxide/src/emulator.rs
pub struct EmulatorRunner {
    pub apple: AppleII,
    cycle_accum: u128,
    last_tick: Instant,
    turbo: bool,
    // ...
}

impl EmulatorRunner {
    pub fn tick(&mut self) -> TickResult { ... }
}
```

---

## 3. video_dirty 检测范围过宽

**文件:** `a2vm-core/src/machine.rs:124`

```rust
if (0x0400..0x6000).contains(&addr) {
    self.video_dirty = true;
}
```

Apple II 的实际视频 RAM 区域：
- TEXT/GR Page 1: `$0400-$07FF`
- TEXT/GR Page 2: `$0800-$0BFF`
- HGR Page 1: `$2000-$3FFF`
- HGR Page 2: `$4000-$5FFF`

当前范围 `$0400..$6000` 包含了 `$0C00-$1FFF` 这段非视频 RAM（约 5KB）。对该区域的写操作会触发不必要的重绘。建议改为精确匹配：

```rust
let is_video = (0x0400..0x0C00).contains(&addr)
    || (0x2000..0x6000).contains(&addr);
```

---

## 4. 磁盘 nibble 写入无自动持久化

**文件:** `a2vm-core/src/disk.rs`

通过 Q6+Q7 写模式（`io_write` → `write_nibble`）写入的数据仅修改内存中的 `nibble_data` 并设置 `dirty = true`，但没有任何代码在适当时机（如电机关闭时）自动调用 `sync_nibble_to_raw()`。

`write_sector_raw()` 能立即持久化，但它是 RWTS trap 专用路径。如果程序通过硬件级 nibble 接口写盘（如拷贝保护程序），数据修改会在退出时丢失。

建议在电机关闭时触发同步：

```rust
// disk.rs handle_switch
0x08 => {
    let prev_motor = self.motor_on;
    self.motor_on = false;
    self.data_ready = false;
    if prev_motor {
        let _ = self.sync_nibble_to_raw(self.selected_drive);
    }
}
```

---

## 5. `run_cycles()` 普通路径未驱动 `disk.tick()`，与 `step()` 语义不一致

**文件:** `a2vm-core/src/machine.rs:220-260`

`step()` 在每条指令后调用 `self.bus.disk.tick(cycles)`（第 228 行），但 `run_cycles()` 的非 fast-disk 分支直接调用 `self.cpu.run()`（第 259 行），完全跳过了 `disk.tick()`。

当前 `tick()` 为空实现所以不暴露问题，但一旦实现 cycle-accurate 磁盘时序，将出现"单步正常、批量运行异常"的行为分叉。

建议统一执行路径：
- 方案 A：`run_cycles()` 普通模式改为按指令循环，每条指令后调用 `disk.tick()`
- 方案 B：引入 `cpu.run_with_hook()`，在 hook 中统一处理外设 tick

---

## 6. `SharedArgs::rom_data()` 不必要的内存拷贝

**文件:** `a2vm-oxide/src/cli.rs:38-43`

```rust
pub fn rom_data(&self) -> Result<Vec<u8>, std::io::Error> {
    match &self.rom {
        Some(path) => std::fs::read(path),
        None => Ok(DEFAULT_ROM.to_vec()), // 20KB 拷贝
    }
}
```

嵌入的 `DEFAULT_ROM` 是 `&'static [u8]`，但 `to_vec()` 每次都会分配并拷贝 20KB。可以改为返回 `Cow`：

```rust
pub fn rom_data(&self) -> Result<Cow<'static, [u8]>, std::io::Error> {
    match &self.rom {
        Some(path) => Ok(Cow::Owned(std::fs::read(path)?)),
        None => Ok(Cow::Borrowed(DEFAULT_ROM)),
    }
}
```

对应 `load_rom_data` 签名已接受 `&[u8]`，无需修改。

---

## 7. CPU 中断处理器 cycle 计数模式不一致

**文件:** `a2vm-core/src/cpu/mod.rs`

普通指令的 cycle 计数在 `step()` 中统一累加（第 179 行）：

```rust
self.cycles += cycles as u64;
```

但 `handle_nmi` / `handle_irq` 在各自内部直接累加（第 924、933 行）：

```rust
self.cycles += 7;
7
```

两种路径混用了不同的计数方式。建议统一为一种模式，例如让中断处理器只返回 cycle 数，由 `step()` 统一累加。

---

## 8. 测试中临时文件未保证清理

**文件:** `a2vm-core/src/disk.rs`（tests）、`a2vm-core/src/machine.rs`（tests）

多个测试使用 `write_temp_dsk()` / `write_temp_file()` 创建临时文件，在测试末尾手动 `fs::remove_file()`。如果测试 panic，文件不会被清理。

建议引入 RAII 清理模式或使用 `tempfile` crate：

```rust
struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}
```

---

## 9. `render_status_bar` 不支持小写字母

**文件:** `a2vm-core/src/video.rs:578-607`

`render_status_bar` 的字符映射只处理 ASCII `0x20-0x5F` 范围：

```rust
let char_index = if (0x20..0x40).contains(&ascii) {
    (ascii - 0x20 + 32) as usize
} else if (0x40..0x60).contains(&ascii) {
    (ascii - 0x40) as usize
} else {
    0 // 小写字母 (0x61-0x7A) 全部映射到 '@'
};
```

传入小写字母（如状态栏中的 "MHz"）会渲染为 `@`。建议添加小写映射：

```rust
} else if (0x60..0x80).contains(&ascii) {
    (ascii - 0x60) as usize  // 映射到与大写相同的字形
}
```

---

## 10. Workspace 依赖版本未统一管理

**文件:** `Cargo.toml`（workspace root）

`clap` 和 `rodio` 在 `a2vm-oxide`、`a2vm-tui`、`a2vm-gui` 中各自独立声明版本。建议使用 workspace 级依赖统一管理：

```toml
# Cargo.toml (workspace root)
[workspace.dependencies]
clap = { version = "4.5", features = ["derive"] }
rodio = { version = "0.21" }
a2vm-core = { path = "a2vm-core" }
a2vm-oxide = { path = "a2vm-oxide" }
```

```toml
# a2vm-tui/Cargo.toml
[dependencies]
clap = { workspace = true, features = ["env"] }
rodio = { workspace = true, optional = true }
```

---

## 11. `Error` 类型未实现 `source()` 链

**文件:** `a2vm-core/src/error.rs:41`

```rust
impl std::error::Error for Error {}
```

`Error::Io` 包装了 `io::Error`，但未实现 `source()` 方法返回内部错误。这会导致错误链信息丢失（如 `anyhow` 或 `eyre` 无法展示完整的错误链）：

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

## 12. GUI 初始化使用 `process::exit` 而非错误传播

**文件:** `a2vm-gui/src/main.rs:98-105`

```rust
fn new(cli: &CliArgs) -> Self {  // 返回 Self，非 Result
    let rom_data = cli.shared.rom_data().unwrap_or_else(|e| {
        eprintln!("Error loading ROM: {e}");
        std::process::exit(1);
    });
```

`App::new()` 在 ROM/磁盘加载失败时直接 `process::exit(1)`，跳过所有 Drop 清理。TUI 端正确使用了 `io::Result` 传播。建议 GUI 也返回 `Result`。

---

## 13. `fill_rect` 逐像素设置效率低

**文件:** `a2vm-core/src/video.rs:254-260`

Lo-Res 渲染中，每个颜色块为固定 7×4 像素，但 `fill_rect` 内部逐像素调用 `set_pixel`（含除法和取模运算）。对于高频调用路径，可以改为直接操作字节：

```rust
fn fill_rect(bitmap: &mut [u8; BITMAP_SIZE], x: usize, y: usize, w: usize, h: usize) {
    for dy in 0..h {
        for dx in 0..w {
            set_pixel(bitmap, x + dx, y + dy);  // 每次调用含 / 和 %
        }
    }
}
```

当 `w=7` 且 `x` 对齐到 7 的倍数（Lo-Res 总是如此）时，可以用批量位操作一次设置多个像素。

---

## 14. `AppleKey::Space` 与 `Printable(' ')` 语义重叠

**文件:** `a2vm-core/src/keyboard.rs`

`AppleKey` 枚举同时定义了 `Space` 和 `Printable(' ')`，两者都映射到 `0x20`。前端代码中 winit 的 `NamedKey::Space` 映射到 `AppleKey::Space`，而终端的空格键走 `Printable(' ')` 路径。

功能上无 bug，但语义上 `Space` 变体是多余的——它的存在让调用方需要决定用哪个。

---

## 15. TUI 错误路径遗留 raw mode / alternate screen

**文件:** `a2vm-tui/src/main.rs:398-431`

`main()` 在第 401 行启用 raw mode、第 403 行进入 alternate screen，但之后 `TuiApp::new(&cli)?`（第 407 行）以及主循环内的多个 `?`（第 414、420 行）都可能提前返回。终端恢复逻辑仅在正常退出路径（第 428-429 行），异常时终端状态不会恢复。

建议引入 RAII 终端守卫：

```rust
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
    }
}

fn main() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    let _guard = TerminalGuard;
    // ... 后续代码无需手动清理
}
```

---

## 16. 12K ROM 加载未清理 slot-6 ROM 残留状态

**文件:** `a2vm-core/src/machine.rs:194-208`

`load_rom_data()` 的 12K 分支仅复制主 ROM 到 `$D000-$FFFF`，不涉及 slot ROM。但 20K 分支会调用 `self.bus.disk.load_slot_rom()` 加载 slot-6 ROM。

如果同一 `AppleII` 实例先加载 20K ROM 再加载 12K ROM，slot-6 ROM 中的旧内容会残留，导致 `$C600-$C6FF` 返回过期数据。

建议在 12K 分支显式清空 slot ROM：

```rust
0x3000 => {
    self.bus.rom.copy_from_slice(data);
    self.bus.disk.clear_slot_rom(); // 新增方法：清零 slot_rom + 置 slot_rom_loaded = false
}
```

---

## 17. `noise` 字段在 audio feature 关闭时产生告警

**文件:** `a2vm-tui/src/main.rs:125`

`TuiApp` 的 `noise: bool` 字段在 `--no-default-features`（禁用 audio）编译时未被使用，产生 dead_code 告警。

建议加条件编译或 `#[allow(dead_code)]`（GUI 端已用 `#[allow(dead_code)]` 处理了类似的 `fast_disk` 字段）：

```rust
#[cfg(feature = "audio")]
noise: bool,
```

---

## 优先级建议

| 优先级 | 项目 | 类型 |
|--------|------|------|
| P0 | #1 SAX 操作码缺失 + 假阳性测试 | Bug |
| P0 | #4 磁盘 nibble 写入无持久化 | 数据丢失风险 |
| P1 | #5 run_cycles() 普通路径未驱动 disk.tick() | 一致性 |
| P1 | #3 video_dirty 范围过宽 | 性能 |
| P1 | #15 TUI 错误路径遗留 raw mode | 健壮性 |
| P2 | #2 TUI/GUI 代码重复 | 可维护性 |
| P2 | #16 12K ROM 加载未清理 slot-6 ROM | 正确性 |
| P2 | #7 中断 cycle 计数不一致 | 代码质量 |
| P2 | #10 Workspace 依赖管理 | 工程规范 |
| P3 | #17 noise 字段 audio feature 告警 | 编译告警 |
| P3 | #6 rom_data 不必要拷贝 | 性能微优化 |
| P3 | #9 状态栏不支持小写 | 显示缺陷 |
| P3 | #11 Error source 链缺失 | 错误处理 |
| P3 | #12 GUI process::exit | 错误处理 |
| P3 | #8 测试临时文件清理 | 测试质量 |
| P3 | #13 fill_rect 效率 | 性能微优化 |
| P3 | #14 Space 枚举重复 | 代码整洁 |
