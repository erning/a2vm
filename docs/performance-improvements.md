# Performance Improvement Plan

**Date**: 2025-02-11
**Scope**: A2VM Apple II Emulator - Code Review & Architecture Analysis

---

## Summary

After thorough code review, this document identifies the **actual** high-impact optimizations
and corrects several inaccuracies in the prior analysis (`performance-analysis.md`).

The two biggest opportunities are architectural:
1. Eliminating dynamic dispatch on the Bus trait (monomorphization)
2. Splitting CPU from Bus ownership to remove `mem::take`

Together these can yield **30-50%** improvement in CPU emulation throughput.

---

## Corrections to Prior Analysis

Before listing improvements, several claims in `performance-analysis.md` are inaccurate:

### C1. `mem::take` frequency is overstated

The prior report claims `mem::take` happens "every instruction execution." This is **wrong**.

In the hot path (`run_cycles`, non-fast-disk), the CPU is extracted **once per frame**:
```rust
// machine.rs:134-137 — ONE take per run_cycles call
let mut cpu = mem::take(&mut self.cpu);
let cycles = cpu.run(self, target);   // runs thousands of instructions
self.cpu = cpu;
```

`Cpu::run()` then loops internally calling `self.step(bus)` without any take/put-back.
The per-instruction `AppleII::step()` method (which does take/put-back) is only used
in the fast-disk trap fallthrough path, not the normal execution path.

### C2. CPU struct size is 24 bytes, not 56

```rust
pub struct Cpu {
    a: u8, x: u8, y: u8, sp: u8,  // 4 bytes
    pc: u16,                        // 2 bytes (+2 padding)
    p: Status,                      // 1 byte (u8 wrapper)
    cycles: u64,                    // 8 bytes (requires 8-byte alignment)
    irq_pending: bool,              // 1 byte
    nmi_pending: bool,              // 1 byte
}
// Total with alignment: ~24 bytes, not 56
```

### C3. Division by 8 is not slow (Issue #5)

The prior report claims `x / 8` and `x % 8` in Braille conversion are expensive divisions.
**Any compiler optimizes `/8` to `>>3` and `%8` to `&7`.** These are single-cycle bit
operations. This is a non-issue.

### C4. Opcode match already compiles to a jump table (Issue #12)

LLVM compiles a `match` with 78+ arms on a contiguous enum into a computed jump table.
Replacing it with an explicit function pointer table would **prevent inlining** and likely
make performance worse, since most instruction bodies are tiny (2-5 operations).

### C5. Fast-disk PC polling is already optimized (Issue #13)

The current code uses `cpu.run_until(self, remaining, 0xB7B5)` which checks PC inside
the CPU loop (`cpu/mod.rs:195-204`). The analysis describes a pattern that no longer
exists in the codebase.

### C6. Bitmap clear is negligible (Issue #8)

Clearing 6720 bytes (~7KB) per frame takes <100ns on modern hardware. This is
immeasurable relative to the frame budget (16.7ms). Not worth optimizing.

---

## Improvements, Ranked by Impact

### P0. Monomorphize Bus Trait (Critical — est. 20-30%)

**Location**: `a2vm-core/src/cpu/mod.rs` — all methods taking `&mut dyn Bus`

**Problem**:
Every bus access goes through virtual dispatch:
```rust
pub fn step(&mut self, bus: &mut dyn Bus) -> u32 {
    bus.set_cycle(self.cycles);          // vtable call #1
    let opcode = self.fetch(bus);         // → bus.read() — vtable call #2
    let resolved = self.resolve(..., bus); // → bus.read() ×1-2 — vtable calls #3-4
    self.execute(..., bus);               // → bus.read/write() ×0-2 — vtable calls #5-6
}
```

That's **3-6 vtable-indirect calls per instruction**, or 3-6M/sec at 1.023 MHz.
Each indirect call:
- Loads the vtable pointer from the fat pointer
- Loads the function pointer from the vtable
- Performs an indirect branch (branch predictor miss risk)
- **Prevents inlining** — the most critical loss

The common case for `Bus::read` (RAM access) is a single array index:
`self.ram[addr as usize]`. Without inlining, function call overhead dominates
the actual work.

**Fix**: Make CPU methods generic over `Bus`:
```rust
impl Cpu {
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 { ... }
    pub fn run<B: Bus>(&mut self, bus: &mut B, target: u64) -> u64 { ... }
    pub fn run_until<B: Bus>(&mut self, bus: &mut B, target: u64, trap: u16) -> u64 { ... }
    pub fn reset<B: Bus>(&mut self, bus: &mut B) { ... }
    fn fetch<B: Bus>(&mut self, bus: &mut B) -> u8 { ... }
    fn resolve<B: Bus>(&mut self, mode: AddrMode, bus: &mut B) -> Resolved { ... }
    fn execute<B: Bus>(&mut self, ..., bus: &mut B) -> u32 { ... }
    // ... all internal methods
}
```

With monomorphization, the compiler can inline `Bus::read` directly into the CPU loop,
turning `bus.read(addr)` into a direct memory access with no function call overhead.

**Compatibility**: The `Bus` trait itself stays unchanged. Only `Cpu` methods change
from `&mut dyn Bus` to generic `<B: Bus>`. AppleII code is the only caller and
requires no changes beyond the method signatures.

**Risk**: Low. The change is mechanical — replace `&mut dyn Bus` with `&mut B` and
add `<B: Bus>` to all methods. Unit tests using `FlatMemory` still work via
monomorphization.

---

### P1. Split CPU from Bus Ownership (Critical — est. 5-15%)

**Location**: `a2vm-core/src/machine.rs`

**Problem**:
`AppleII` owns both `Cpu` and the bus state (RAM/ROM/IO). When the CPU needs to
execute, it needs `&mut self` (Cpu) + `&mut dyn Bus` (AppleII). Since `Cpu` lives
inside `AppleII`, Rust's borrow checker prevents borrowing both simultaneously.
The current workaround is `mem::take`:
```rust
let mut cpu = mem::take(&mut self.cpu);  // Extract CPU, leave Default in place
cpu.run(self, target);                    // Now cpu and self are separate
self.cpu = cpu;                           // Put CPU back
```

While the overhead per frame is small (one 24-byte swap+zero per `run_cycles` call),
this pattern is architecturally fragile and prevents future optimizations like
keeping the CPU hot across frames.

**Fix**: Separate CPU and bus into sibling fields:
```rust
pub struct BusState {
    pub display: DisplayMode,
    pub disk: DiskII,
    speaker: Speaker,
    bus_cycle: u64,
    disk_controller_enabled: bool,
    fast_disk: bool,
    ram: [u8; 0xC000],
    rom: [u8; 0x3000],
    rom_loaded: bool,
    kbd_latch: u8,
}

impl Bus for BusState { ... }  // Move Bus impl here

pub struct AppleII {
    pub cpu: Cpu,
    pub bus: BusState,
}

impl AppleII {
    pub fn step(&mut self) -> u32 {
        self.cpu.step(&mut self.bus)  // No mem::take needed!
    }
    pub fn run_cycles(&mut self, target: u64) -> u64 {
        self.cpu.run(&mut self.bus, target)
    }
}
```

The borrow checker allows `&mut self.cpu` and `&mut self.bus` simultaneously because
they are disjoint fields. This eliminates `mem::take` entirely.

**Combined with P0**: When `Cpu::step` is generic, the call becomes:
```rust
self.cpu.step(&mut self.bus)  // Monomorphized, no vtable, no mem::take
```

**Risk**: Medium. Requires updating all code that accesses `apple.ram`, `apple.display`,
etc. to go through `apple.bus.ram`, `apple.bus.display`. Mechanical but touches many
call sites. The `try_rwts_trap` method needs access to both CPU and bus, which is
straightforward with the new layout.

---

### P2. Eliminate Per-Instruction `set_cycle` Call (High — est. 3-5%)

**Location**: `a2vm-core/src/cpu/mod.rs:156`, `a2vm-core/src/bus.rs:15`

**Problem**:
Every instruction begins with a vtable call to set the cycle counter:
```rust
pub fn step(&mut self, bus: &mut dyn Bus) -> u32 {
    bus.set_cycle(self.cycles);  // Called ~1M times/sec
    ...
}
```

This exists solely so the speaker toggle ($C030) records the correct cycle. But the
speaker is toggled maybe a few hundred times per second. The other ~999,800
instructions pay the overhead for nothing.

**Fix**: Remove `set_cycle` from the per-instruction path. Instead, pass the cycle
count directly when toggling the speaker:

Option A — Pass cycle in Bus::read signature:
```rust
trait Bus {
    fn read(&mut self, addr: u16, cycle: u64) -> u8;
}
```
This is invasive but clean.

Option B — Store cycle in CPU, let bus access it:
Since the bus already has `&mut self` access during `read()`, after P1 the bus
can just record the cycle from `Cpu::cycles()` at the point of speaker toggle.
Actually, with P1, `BusState` doesn't have access to Cpu. So:

Option C — Only update cycle on I/O access:
```rust
fn read(&mut self, addr: u16) -> u8 {
    match addr {
        0x0000..=0xBFFF => self.ram[addr as usize],  // Hot path: no cycle update
        _ => self.read_io(addr),  // Cold path: I/O region
    }
}
```

The I/O handler can accept the current cycle as a parameter, or `set_cycle` can be
called only before the bus read/write that might touch I/O.

**Best approach with P0+P1**: In `Cpu::step`, only call `bus.set_cycle()` for I/O
addresses. The CPU already knows the opcode's target address after `resolve()`.
For zero-page and RAM-only instructions, skip the call entirely.

---

### P3. Optimize HGR Color Neighbor Lookup (High — est. 5-10% in HGR mode)

**Location**: `a2vm-core/src/video.rs:491-504`

**Problem**:
In color HGR rendering, neighbor pixel checks use **division by 7** (not a power of 2):
```rust
let prev_col = (x - 1) / 7;   // Real integer division!
let prev_bit = (x - 1) % 7;   // Real modulo!
let next_col = (x + 1) / 7;   // Real integer division!
let next_bit = (x + 1) % 7;   // Real modulo!
```

Division/modulo by 7 compiles to a multiply-shift sequence (~5 cycles), not a simple
bit shift like `/8`. This happens for up to 280×192 = 53,760 pixels per frame,
with 4 divisions each = 215,040 divisions.

**Fix**: Use a sliding window, processing sequentially within each byte:
```rust
fn render_hires_scanlines_rgba(ram: &[u8], rgba: &mut [u8], base: usize, num_lines: usize, color_mode: DisplayColorMode) {
    for y in 0..num_lines {
        let addr = hgr_line_addr(base, y);
        let mut prev_pixel_on = false;

        for col in 0..40usize {
            let byte = ram[addr + col];
            let high_bit = byte & 0x80 != 0;
            let next_byte = if col < 39 { ram[addr + col + 1] } else { 0 };
            let pixel_x = col * 7;

            for bit in 0..7usize {
                let on = byte & (1 << bit) != 0;
                let x = pixel_x + bit;

                // Next pixel: either next bit in same byte, or bit 0 of next byte
                let next_on = if bit < 6 {
                    byte & (1 << (bit + 1)) != 0
                } else {
                    next_byte & 1 != 0
                };

                let color = if color_mode == DisplayColorMode::Color {
                    if !on {
                        HIRES_BLACK
                    } else if prev_pixel_on || next_on {
                        HIRES_WHITE
                    } else if high_bit {
                        if x % 2 == 0 { HIRES_BLUE } else { HIRES_ORANGE }
                    } else {
                        if x % 2 == 0 { HIRES_PURPLE } else { HIRES_GREEN }
                    }
                } else if on { MONO_FG } else { MONO_BG };

                let idx = (y * RGBA_WIDTH + x) * 4;
                rgba[idx..idx + 4].copy_from_slice(&color);
                prev_pixel_on = on;
            }
        }
    }
}
```

Zero divisions. Previous pixel state carried forward. Next pixel checked via bit
shift within current/next byte.

---

### P4. Audio Buffer: `mem::take` Instead of Clone (Medium — allocation pressure)

**Location**: `a2vm-tui/src/main.rs:326`, `a2vm-gui/src/main.rs:277`

**Problem**:
```rust
sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, audio_buffer.clone()));
```

Clones ~735 f32 samples (~3KB) per frame. The original buffer is then overwritten
next frame via `clear()` + `render_until_into()`.

**Fix**:
```rust
// TUI — take ownership, buffer will be re-filled next frame
let samples: Vec<f32> = std::mem::take(&mut audio_buffer);
sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, samples));
// audio_buffer is now empty Vec with zero capacity — will reallocate next frame
```

Better: use `std::mem::replace` to keep the capacity:
```rust
let samples = std::mem::replace(&mut audio_buffer, Vec::new());
sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, samples));
```

Or best: double-buffer approach (never reallocate):
```rust
let mut audio_buf_a = Vec::with_capacity(4096);
let mut audio_buf_b = Vec::with_capacity(4096);
// Each frame:
apple.take_audio_samples_into(AUDIO_SAMPLE_RATE, real_cycles, &mut audio_buf_a);
if !audio_buf_a.is_empty() {
    std::mem::swap(&mut audio_buf_a, &mut audio_buf_b);
    sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, audio_buf_b.clone()));
    // audio_buf_b keeps its capacity for next swap
}
```

Actually the simplest correct fix: just `std::mem::take`:
```rust
sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, std::mem::take(&mut audio_buffer)));
```

Since `render_until_into` calls `out.reserve(expected)` anyway, the Vec will
reallocate to the right size on the next call. The allocation overhead is minimal
(once per frame, ~3KB).

---

### P5. Static Reverse Table for Disk Decode (Low — disk I/O only)

**Location**: `a2vm-core/src/disk.rs:493-497`

**Problem**:
```rust
fn decode_6and2_sector(encoded: &[u8; 343]) -> Option<[u8; 256]> {
    let mut reverse_table = [0u8; 256];
    for (i, &val) in WRITE_TABLE.iter().enumerate() {
        reverse_table[val as usize] = i as u8;
    }
    ...
}
```

Rebuilds the 256-byte reverse lookup table on every sector decode call.

**Fix**:
```rust
const REVERSE_TABLE: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 64 {
        table[WRITE_TABLE[i] as usize] = i as u8;
        i += 1;
    }
    table
};
```

Computed at compile time. Zero runtime cost.

---

### P6. Optimize `fill_rgba` with `u32` Writes (Low — rendering)

**Location**: `a2vm-core/src/video.rs:602-606`

**Problem**:
```rust
fn fill_rgba(rgba: &mut [u8], color: [u8; 4]) {
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&color);
    }
}
```

Writes 4 bytes at a time. The compiler may or may not vectorize this.

**Fix**: Write as `u32` to guarantee word-sized writes:
```rust
fn fill_rgba(rgba: &mut [u8], color: [u8; 4]) {
    let word = u32::from_ne_bytes(color);
    // Safety: RGBA buffer is always a multiple of 4 bytes
    for chunk in rgba.chunks_exact_mut(4) {
        let ptr = chunk.as_mut_ptr() as *mut u32;
        unsafe { ptr.write_unaligned(word) };
    }
}
```

Or, safer alternative relying on the optimizer:
```rust
fn fill_rgba(rgba: &mut [u8], color: [u8; 4]) {
    let color_word = u32::from_ne_bytes(color);
    let (prefix, aligned, suffix) = unsafe { rgba.align_to_mut::<u32>() };
    for b in prefix.chunks_exact_mut(4) { b.copy_from_slice(&color); }
    aligned.fill(color_word);
    for b in suffix.chunks_exact_mut(4) { b.copy_from_slice(&color); }
}
```

---

### P7. Pre-allocate Speaker Toggle VecDeque (Low — audio latency)

**Location**: `a2vm-core/src/audio.rs:22`

**Problem**:
`VecDeque::new()` starts with zero capacity. First toggle causes allocation.

**Fix**:
```rust
pub fn new() -> Self {
    Self {
        state: false,
        toggles: VecDeque::with_capacity(2048),
        ...
    }
}
```

---

### P8. GUI: Skip Rendering When Display Unchanged (Low — GPU bandwidth)

**Location**: `a2vm-gui/src/main.rs:333-354`

**Problem**:
The GUI calls `render_rgba()` every frame unconditionally. The TUI already has
dirty-checking (`bitmap != last_bitmap`), but the GUI does not.

**Fix**: Track dirty state based on RAM writes to display regions:
```rust
// In BusState (after P1):
fn write(&mut self, addr: u16, val: u8) {
    match addr {
        0x0000..=0xBFFF => {
            self.ram[addr as usize] = val;
            // Mark display dirty if write hits display memory
            if (0x0400..0x0800).contains(&addr) || (0x2000..0x4000).contains(&addr) {
                self.display_dirty = true;
            }
        }
        ...
    }
}
```

Or simpler: just hash/compare the relevant RAM regions per frame (cheaper than
full RGBA render).

---

### P9. Braille Conversion: Lookup Table for Bit Mapping (Low — TUI only)

**Location**: `a2vm-tui/src/main.rs:77-87`

**Problem**:
```rust
let braille_bit = match (dx, dy) {
    (0, 0) => 0,
    (0, 1) => 1,
    (0, 2) => 2,
    (0, 3) => 6,
    (1, 0) => 3,
    (1, 1) => 4,
    (1, 2) => 5,
    (1, 3) => 7,
    _ => unreachable!(),
};
```

This match executes 140 × 48 × 8 = 53,760 times per frame.

**Fix**: Use a 2D const array:
```rust
const BRAILLE_BIT: [[u8; 4]; 2] = [
    [0, 1, 2, 6], // dx=0: dy 0,1,2,3
    [3, 4, 5, 7], // dx=1: dy 0,1,2,3
];
// Usage: bits |= 1 << BRAILLE_BIT[dx as usize][dy as usize];
```

---

## Recommended Implementation Order

### Phase 1: Architecture (P0 + P1)
Monomorphize Bus + split CPU/Bus ownership. These two changes are synergistic
and should be done together. **Expected: 25-40% CPU throughput improvement.**

Steps:
1. Create `BusState` struct, move RAM/ROM/IO fields from `AppleII`
2. Move `Bus` impl from `AppleII` to `BusState`
3. Update `AppleII` to hold `cpu: Cpu` + `bus: BusState`
4. Make all `Cpu` methods generic: `fn step<B: Bus>(&mut self, bus: &mut B)`
5. Remove all `mem::take` patterns in `machine.rs`
6. Update frontends to access `apple.bus.display`, `apple.bus.disk`, etc.
7. Run Klaus Dormann test to verify correctness
8. Benchmark before/after

### Phase 2: Quick Wins (P4, P5, P7)
Audio clone fix + static reverse table + VecDeque prealloc.
Trivial changes, clear benefits. **Expected: measurable allocation reduction.**

### Phase 3: Rendering (P2, P3, P6)
Eliminate `set_cycle` per instruction + HGR neighbor optimization + fill optimization.
**Expected: 5-15% improvement in active rendering scenarios.**

### Phase 4: Polish (P8, P9)
GUI dirty tracking + Braille lookup table. Minor improvements.

---

## Benchmarking Plan

Before starting, establish baselines:

```bash
# CPU throughput baseline (Klaus Dormann test)
cargo test -p a2vm-core klaus_dormann --release -- --nocapture

# Profile CPU hot loop
cargo build --release -p a2vm-core
cargo instruments -t time -p a2vm-core --test klaus_dormann --release

# Frame rendering time (add timing to render functions)
# Audio latency measurement
```

Key metrics to track:
- **cycles/sec** from Klaus Dormann test (currently ~117M cycles/sec = 0.82s for 96M)
- **frame render time** (ms) for each video mode
- **allocation rate** (bytes/sec) via DHAT or Instruments

---

## Appendix: Quick Reference

| ID | Improvement | Location | Impact | Effort |
|----|-------------|----------|--------|--------|
| P0 | Monomorphize Bus | `cpu/mod.rs` | 20-30% | Medium |
| P1 | Split CPU/Bus | `machine.rs` | 5-15% | Medium |
| P2 | Remove `set_cycle` | `cpu/mod.rs:156` | 3-5% | Low |
| P3 | HGR neighbor sliding window | `video.rs:491-504` | 5-10% (HGR) | Low |
| P4 | Audio buffer no-clone | TUI/GUI main.rs | Alloc | Trivial |
| P5 | Static reverse table | `disk.rs:493` | Disk I/O | Trivial |
| P6 | fill_rgba u32 writes | `video.rs:602` | Minor | Low |
| P7 | VecDeque prealloc | `audio.rs:22` | Minor | Trivial |
| P8 | GUI dirty tracking | GUI main.rs | Minor | Low |
| P9 | Braille bit lookup | TUI main.rs | Minor | Trivial |
