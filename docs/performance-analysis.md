# Performance Analysis Report

**Date**: 2025-02-11  
**Scope**: A2VM Apple II Emulator - Full Codebase  
**Status**: Analysis Complete, Implementation Pending

---

## Executive Summary

This document identifies 18 performance optimization opportunities across the A2VM emulator codebase, categorized by severity and component. The primary bottlenecks are in the CPU execution loop (struct movement overhead), video rendering (redundant computations), and audio processing (allocation pressure).

**Estimated Overall Impact**: 20-40% performance improvement possible with targeted optimizations.

---

## 🔴 High Priority Issues

### 1. CPU Struct Movement Overhead (Critical)

**Location**: `a2vm-core/src/machine.rs`
- Lines 82-84, 96-98, 114-136, 127-130

**Problem**: 
The `std::mem::take` pattern is used to temporarily extract the CPU for mutable access:

```rust
let mut cpu = mem::take(&mut self.cpu);
cpu.reset(self);
self.cpu = cpu;  // Moves 56 bytes back
```

This pattern occurs:
- Every instruction execution (`step()`)
- Every `run_cycles()` call
- Every reset operation

**Impact**:
- 56-byte memory copy per instruction (CPU struct size)
- At 1.023MHz, that's ~57MB/sec of memory traffic just for struct movement
- Cache pressure and memcpy overhead

**Benchmark Estimate**: 10-20% of total CPU time

**Suggested Fix**:
```rust
// Option 1: Use Option<Cpu> with take()
self.cpu.take().map(|mut cpu| {
    cpu.step(self);
    self.cpu = Some(cpu);
});

// Option 2: Pin<Box<Cpu>> for stable address
// Option 3: RefCell/UnsafeCell for interior mutability
```

---

### 2. Dynamic Dispatch on Bus Trait (Critical)

**Location**: 
- `a2vm-core/src/cpu/mod.rs`: Lines 155-181, 208-241
- `a2vm-core/src/bus.rs`: Entire trait

**Problem**:
All memory operations go through `&mut dyn Bus` virtual dispatch:

```rust
pub fn step(&mut self, bus: &mut dyn Bus) -> u32 {
    let opcode = self.fetch(bus);  // vtable lookup
    // ...
}

fn fetch(&mut self, bus: &mut dyn Bus) -> u8 {
    bus.read(self.pc)  // Indirect call
}
```

**Impact**:
- ~2-3 memory accesses per instruction minimum
- Millions of vtable lookups per second
- Branch prediction misses on indirect jumps

**Benchmark Estimate**: 5-15% overhead on memory-intensive code

**Suggested Fix**:
```rust
// Generic approach
pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
    // Monomorphized - direct calls
}

// Or function pointer table for known bus implementations
```

---

### 3. Audio Buffer Cloning in TUI (High)

**Location**: `a2vm-tui/src/main.rs:323-327`

**Problem**:
```rust
sink.append(SamplesBuffer::new(
    1,
    AUDIO_SAMPLE_RATE,
    audio_buffer.clone(),  // Full copy every frame!
));
```

The audio buffer is cloned every frame (~60fps) to pass to rodio. This creates significant allocation pressure.

**Impact**:
- ~44KB clone per frame at 44.1kHz (assuming ~735 samples/frame)
- 2.6MB/sec allocation pressure
- GC/allocator overhead

**Suggested Fix**:
```rust
// Use drain or swap
let samples = std::mem::take(&mut audio_buffer);
sink.append(SamplesBuffer::new(1, AUDIO_SAMPLE_RATE, samples));
// audio_buffer is now empty and will be reallocated
```

---

### 4. Audio Buffer Cloning in GUI (High)

**Location**: `a2vm-gui/src/main.rs:274-278`

Same issue as TUI - identical pattern.

---

## 🟡 Medium Priority Issues

### 5. Braille Conversion Division Operations

**Location**: `a2vm-tui/src/main.rs:54-100`

**Problem**:
```rust
let byte_idx = y * BITMAP_STRIDE + x / 8;  // Division
let bit_idx = 7 - (x % 8);                  // Modulo
```

These operations execute for every pixel (280×192 = 53,760 pixels):
- 53,760 divisions (`/ 8`)
- 53,760 modulo operations (`% 8`)

**Impact**:
- Integer division is ~20-40 cycles on most CPUs
- ~2-4 million cycles wasted per frame

**Suggested Fix**:
```rust
// Precompute lookup tables
const X_TO_BYTE: [usize; 280] = {
    let mut table = [0usize; 280];
    let mut i = 0;
    while i < 280 {
        table[i] = i / 8;
        i += 1;
    }
    table
};

const X_TO_BIT: [u8; 280] = {
    let mut table = [0u8; 280];
    let mut i = 0;
    while i < 280 {
        table[i] = 7 - (i % 8) as u8;
        i += 1;
    }
    table
};

// Usage
let byte_idx = y * BITMAP_STRIDE + X_TO_BYTE[x];
let bit_idx = X_TO_BIT[x];
```

---

### 6. Hi-Res RGBA Neighbor Pixel Recalculation

**Location**: `a2vm-core/src/video.rs:468-531`

**Problem**:
For every pixel, the code recalculates neighbor positions:

```rust
for bit in 0..7usize {
    let on = byte & (1 << bit) != 0;
    // ... color calculation
    
    let prev_on = if x > 0 {
        let prev_col = (x - 1) / 7;      // Division!
        let prev_bit = (x - 1) % 7;      // Modulo!
        ram[addr + prev_col] & (1 << prev_bit) != 0
    } else { false };
    
    let next_on = if x < 279 {
        let next_col = (x + 1) / 7;      // Division!
        let next_bit = (x + 1) % 7;      // Modulo!
        ram[addr + next_col] & (1 << next_bit) != 0
    } else { false };
}
```

**Impact**:
- 280×192×2 = 107,520 division operations per frame
- ~4-8 million cycles

**Suggested Fix**:
```rust
// Process pixels sequentially, maintain sliding window
let mut prev_byte = 0u8;
let mut curr_byte = ram[addr];
let mut next_byte = ram[addr + 1];

for col in 0..40 {
    for bit in 0..7 {
        let x = col * 7 + bit;
        let prev_on = if bit == 0 { 
            prev_byte & 0x40 != 0  // Bit 6 of previous
        } else {
            curr_byte & (1 << (bit - 1)) != 0
        };
        // ... etc
    }
    prev_byte = curr_byte;
    curr_byte = next_byte;
    next_byte = ram[addr + col + 2];
}
```

---

### 7. Scanline Effect Floating-Point Math

**Location**: `a2vm-core/src/video.rs:533-561`

**Problem**:
```rust
fn apply_scanlines(rgba: &mut [u8], frame_phase: u64) {
    let global_flicker = 0.985 + 0.015 * ((frame_phase as f32) * 0.11).sin();
    
    for y in 0..RGBA_HEIGHT {
        let row_wobble = 0.01 * ((y as f32) * 0.35 + (frame_phase as f32) * 0.07).sin();
        
        for x in 0..RGBA_WIDTH {
            // ... per-pixel float math
        }
    }
}
```

**Impact**:
- 53,760 pixels × multiple sin() calls
- ~270K FLOPs per frame
- Float-to-int conversions

**Suggested Fix**:
```rust
// Precompute sin tables
const FLICKER_TABLE_SIZE: usize = 256;
const ROW_WOBBLE_TABLE_SIZE: usize = 192;

static FLICKER_TABLE: [f32; FLICKER_TABLE_SIZE] = {
    let mut table = [0.0f32; FLICKER_TABLE_SIZE];
    let mut i = 0;
    while i < FLICKER_TABLE_SIZE {
        let phase = (i as f32) * 0.11;
        table[i] = 0.985 + 0.015 * phase.sin();
        i += 1;
    }
    table
};

// Usage: FLICKER_TABLE[(frame_phase as usize) % FLICKER_TABLE_SIZE]
```

---

### 8. Bitmap Force-Clear Overhead

**Location**: `a2vm-core/src/video.rs:99`

**Problem**:
```rust
pub fn render(ram: &[u8], mode: &DisplayMode, flash_on: bool, bitmap: &mut [u8; BITMAP_SIZE]) {
    bitmap.fill(0);  // Always clears 6720 bytes
    // ... rendering may write all pixels anyway
}
```

When rendering text, lo-res, or hi-res modes, the renderers write to all pixels anyway. The `fill(0)` is redundant.

**Impact**:
- 6720 bytes written unnecessarily per frame
- Cache line pollution

**Suggested Fix**:
```rust
pub fn render(ram: &[u8], mode: &DisplayMode, flash_on: bool, bitmap: &mut [u8; BITMAP_SIZE]) {
    // Only clear if the mode might leave gaps
    // Or ensure all renderers completely fill the bitmap
}
```

---

### 9. Fill Rect Pixel-by-Pixel

**Location**: `a2vm-core/src/video.rs:254-260`

**Problem**:
```rust
fn fill_rect(bitmap: &mut [u8; BITMAP_SIZE], x: usize, y: usize, w: usize, h: usize) {
    for dy in 0..h {
        for dx in 0..w {
            set_pixel(bitmap, x + dx, y + dy);  // Per-pixel function call
        }
    }
}
```

Lo-res mode uses this for color blocks (40×48 blocks, each 7×4 or 7×4 pixels).

**Impact**:
- Function call overhead per pixel
- No SIMD utilization

**Suggested Fix**:
```rust
fn fill_rect_fast(bitmap: &mut [u8; BITMAP_SIZE], x: usize, y: usize, w: usize, h: usize) {
    for dy in 0..h {
        let row_start = (y + dy) * BITMAP_STRIDE;
        let start_byte = row_start + x / 8;
        let end_byte = row_start + (x + w - 1) / 8;
        
        // Handle partial bytes at edges
        // Fill full bytes in middle with 0xFF
        bitmap[start_byte..=end_byte].fill(0xFF);
    }
}
```

---

## 🟢 Lower Priority Issues

### 10. Disk Sector Decoding - Reverse Table Recreation

**Location**: `a2vm-core/src/disk.rs:494-497`

**Problem**:
```rust
fn decode_6and2_sector(encoded: &[u8; 343]) -> Option<[u8; 256]> {
    let mut reverse_table = [0u8; 256];  // Stack allocation every call!
    for (i, &val) in WRITE_TABLE.iter().enumerate() {
        reverse_table[val as usize] = i as u8;
    }
    // ...
}
```

A 256-byte array is created and filled on every sector decode (used during nibblization sync).

**Impact**:
- Stack allocation and initialization overhead
- Only affects disk I/O operations (infrequent)

**Suggested Fix**:
```rust
static REVERSE_TABLE: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 64 {
        table[WRITE_TABLE[i] as usize] = i as u8;
        i += 1;
    }
    table
};
```

---

### 11. Braille String Allocation Per Frame

**Location**: `a2vm-tui/src/main.rs:46, 56-97`

**Problem**:
```rust
fn bitmap_to_braille(bitmap: &[u8; BITMAP_SIZE]) -> Vec<String> {
    let mut lines = Vec::with_capacity(rows);
    for brow in 0..rows {
        let mut line = String::with_capacity(cols * 3);
        // ...
        lines.push(line);  // New String every row
    }
    lines
}
```

140×48 Braille grid = 6,720 characters ≈ 20KB of string data allocated per frame.

**Impact**:
- Allocation pressure (~1.2MB/sec at 60fps)
- Fragmentation

**Suggested Fix**:
Reuse a single buffer:
```rust
struct BrailleBuffer {
    lines: Vec<String>,
}

impl BrailleBuffer {
    fn clear(&mut self) {
        for line in &mut self.lines {
            line.clear();  // Keep capacity
        }
    }
    
    fn update(&mut self, bitmap: &[u8; BITMAP_SIZE]) {
        self.clear();
        // Append to existing strings
    }
}
```

---

### 12. Opcode Match Branch Prediction

**Location**: `a2vm-core/src/cpu/mod.rs:387-779`

**Problem**:
A 78-branch match statement on `Mnemonic` enum:

```rust
match mnemonic {
    Mnemonic::LDA => { ... }
    Mnemonic::LDX => { ... }
    // ... 76 more branches
}
```

Modern CPUs struggle to predict which branch will be taken when the opcode sequence is irregular.

**Impact**:
- Branch prediction misses
- Pipeline flushes

**Suggested Fix**:
```rust
// Function pointer table (jump table)
type OpcodeFn = fn(&mut Cpu, &Resolved, &mut dyn Bus) -> u32;

static OPCODE_TABLE: [OpcodeFn; 78] = [
    op_lda, op_ldx, // ...
];

// In execute:
let cycles = OPCODE_TABLE[mnemonic as usize](self, resolved, bus);
```

---

### 13. Fast-Disk Mode PC Polling

**Location**: `a2vm-core/src/machine.rs:103-139`

**Problem**:
```rust
pub fn run_cycles(&mut self, target: u64) -> u64 {
    if self.fast_disk {
        while self.cpu.cycles() - start < effective {
            // ...
            if self.cpu.pc() == 0xB7B5 {  // Check every iteration
                // ...
            }
        }
    }
}
```

The PC is checked every instruction iteration to trap RWTS calls.

**Impact**:
- Branch prediction pressure
- Extra comparison per instruction

**Suggested Fix**:
Use the existing `run_until` mechanism more efficiently, or add a dedicated fast-path with PC check in the CPU's main loop.

---

### 14. HGR Memory Non-Sequential Access

**Location**: `a2vm-core/src/video.rs:217-241`

**Problem**:
Hi-res graphics memory is interleaved in a complex pattern:
```rust
fn hgr_line_addr(base: usize, y: usize) -> usize {
    base + ((y & 7) << 10) + (((y >> 3) & 7) << 7) + (y >> 6) * 40
}
```

Rendering scanlines sequentially jumps through memory non-linearly, causing cache misses.

**Impact**:
- Cache thrashing
- Memory bandwidth inefficiency

**Suggested Fix**:
Precompute all line addresses:
```rust
const HGR_LINE_ADDRS: [usize; 192] = {
    let mut addrs = [0usize; 192];
    let mut y = 0;
    while y < 192 {
        addrs[y] = 0x2000 + ((y & 7) << 10) + (((y >> 3) & 7) << 7) + (y >> 6) * 40;
        y += 1;
    }
    addrs
};
```

---

### 15. VecDeque Growth in Audio

**Location**: `a2vm-core/src/audio.rs:12, 39-40`

**Problem**:
Speaker toggle timestamps are stored in a `VecDeque<u64>`. Under heavy speaker activity, this may reallocate.

**Impact**:
- Allocation during audio generation
- Potential latency spikes

**Suggested Fix**:
Pre-allocate capacity based on expected toggle rate, or use a fixed-size ring buffer:
```rust
pub struct Speaker {
    toggles: VecDeque<u64>,
}

impl Speaker {
    pub fn new() -> Self {
        Self {
            toggles: VecDeque::with_capacity(1024),  // Pre-allocate
            // ...
        }
    }
}
```

---

### 16. RGBA Fill Byte-by-Byte

**Location**: `a2vm-core/src/video.rs:602-619`

**Problem**:
```rust
fn fill_rgba_region(rgba: &mut [u8], color: [u8; 4], x: usize, y: usize, w: usize, h: usize) {
    for dy in 0..h {
        for dx in 0..w {
            let idx = row_start + dx * 4;
            rgba[idx..idx + 4].copy_from_slice(&color);  // 4-byte copy per pixel
        }
    }
}
```

**Impact**:
- No SIMD utilization
- Multiple small copies instead of bulk operations

**Suggested Fix**:
Use platform-specific fill operations or `slice::fill` with pattern:
```rust
// Fill with pattern [R,G,B,A,R,G,B,A,...]
let pattern = [color[0], color[1], color[2], color[3]].repeat(w);
for dy in 0..h {
    let row_start = ((y + dy) * RGBA_WIDTH + x) * 4;
    rgba[row_start..row_start + w * 4].copy_from_slice(&pattern);
}
```

---

### 17. Missing Frame Skip Logic

**Location**: `a2vm-tui/src/main.rs:263-432`, `a2vm-gui/src/main.rs:243-354`

**Problem**:
Neither frontend implements frame skipping when emulation falls behind real-time. If the CPU can't keep up, the emulator enters a "death spiral" of accumulated delay.

**Impact**:
- Audio/visual desync
- Unrecoverable lag

**Suggested Fix**:
Implement frame skip counter:
```rust
const MAX_FRAMES_BEHIND: u32 = 3;

if frames_behind > MAX_FRAMES_BEHIND {
    skip_render = true;
    frames_to_catch_up = frames_behind - MAX_FRAMES_BEHIND;
}
```

---

### 18. No SIMD Usage Anywhere

**Location**: Entire codebase

**Problem**:
The codebase makes no use of SIMD instructions (SSE, AVX, NEON) for:
- Video rendering
- Audio synthesis
- Memory fills/copies

**Impact**:
- Missing 4-16x performance improvements on bulk operations

**Suggested Fix**:
- Use `std::simd` (nightly) or `packed_simd`
- Platform-specific intrinsics for hot paths
- Consider `rayon` for parallel frame rendering

---

## Recommended Implementation Order

### Phase 1: Quick Wins (1-2 days)
1. ✅ Fix audio buffer cloning (Issues #3, #4)
2. ✅ Precompute Braille lookup tables (Issue #5)
3. ✅ Make reverse table static (Issue #10)
4. ✅ Preallocate VecDeque capacity (Issue #15)

**Expected Gain**: 10-15% improvement

### Phase 2: CPU Optimizations (3-5 days)
1. ✅ Replace `mem::take` pattern (Issue #1)
2. ✅ Consider generic Bus trait (Issue #2)
3. ✅ Implement opcode jump table (Issue #12)

**Expected Gain**: 15-25% improvement

### Phase 3: Rendering Optimizations (5-7 days)
1. ✅ Optimize Hi-Res neighbor access (Issue #6)
2. ✅ Scanline lookup tables (Issue #7)
3. ✅ Remove redundant bitmap clear (Issue #8)
4. ✅ Optimize fill_rect (Issue #9)
5. ✅ Braille buffer reuse (Issue #11)

**Expected Gain**: 20-30% rendering throughput

### Phase 4: Advanced Optimizations (1-2 weeks)
1. ✅ HGR address precomputation (Issue #14)
2. ✅ SIMD integration (Issue #18)
3. ✅ Frame skip logic (Issue #17)
4. ✅ Profile-guided optimization

**Expected Gain**: Additional 10-20%

---

## Benchmarking Recommendations

Before implementing changes, establish baseline metrics:

1. **CPU Benchmark**: Run Klaus Dormann test, measure cycles/second
2. **Rendering Benchmark**: Measure FPS in different video modes
3. **Audio Benchmark**: Measure latency and buffer underruns
4. **Memory Benchmark**: Track allocation rates with `dhat` or `valgrind`

### Tools
```bash
# CPU profiling
cargo build --release -p a2vm-core
cargo test klaus_dormann --release -- --nocapture

# Memory profiling
cargo install dhat
cargo build --release -p a2vm-gui
valgrind --tool=dhat ./target/release/a2vm-gui --rom roms/apple2p.rom

# Flamegraph
cargo install flamegraph
cargo flamegraph -p a2vm-gui -- --rom roms/apple2p.rom
```

---

## Appendix: Issue Quick Reference

| ID | Issue | Location | Severity | Est. Impact |
|----|-------|----------|----------|-------------|
| 1 | CPU struct movement | `machine.rs:82-136` | 🔴 Critical | 10-20% |
| 2 | Dynamic dispatch | `cpu/mod.rs, bus.rs` | 🔴 Critical | 5-15% |
| 3 | TUI audio clone | `a2vm-tui/src/main.rs:323` | 🔴 High | Allocation |
| 4 | GUI audio clone | `a2vm-gui/src/main.rs:274` | 🔴 High | Allocation |
| 5 | Braille division | `a2vm-tui/src/main.rs:71-72` | 🟡 Medium | ~4M cycles |
| 6 | Hi-Res neighbors | `video.rs:492-504` | 🟡 Medium | ~8M cycles |
| 7 | Scanline float math | `video.rs:534-540` | 🟡 Medium | ~270K FLOPs |
| 8 | Bitmap clear | `video.rs:99` | 🟡 Medium | 6.7KB/frame |
| 9 | Fill rect per-pixel | `video.rs:254-260` | 🟡 Medium | Function calls |
| 10 | Disk reverse table | `disk.rs:494-497` | 🟢 Low | Disk I/O only |
| 11 | Braille allocation | `a2vm-tui/src/main.rs:56-97` | 🟢 Low | ~1.2MB/sec |
| 12 | Opcode match | `cpu/mod.rs:387-779` | 🟢 Low | Branch mispredict |
| 13 | Fast-disk polling | `machine.rs:124` | 🟢 Low | Per-instruction |
| 14 | HGR non-sequential | `video.rs:217-241` | 🟢 Low | Cache misses |
| 15 | VecDeque growth | `audio.rs:12,39` | 🟢 Low | Audio latency |
| 16 | RGBA byte fill | `video.rs:609-617` | 🟢 Low | No SIMD |
| 17 | No frame skip | TUI/GUI main loops | 🟢 Low | UX issue |
| 18 | No SIMD | Entire codebase | 🟢 Low | 4-16x potential |

---

*Document generated by performance analysis. Implementation tracking: See GitHub issues or project board.*
