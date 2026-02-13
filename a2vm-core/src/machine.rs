use std::path::Path;

use crate::audio::Speaker;
use crate::bus::Bus;
use crate::cpu::status::C;
use crate::cpu::Cpu;
use crate::disk::DiskII;
use crate::error::{Error, Result};
use crate::video::DisplayMode;

/// Bus state: RAM, ROM, keyboard, display, disk, speaker.
///
/// Separated from CPU to allow simultaneous mutable borrows,
/// eliminating the `mem::take` pattern.
///
/// Memory map:
///   $0000-$BFFF  48K RAM
///   $C000        Keyboard latch (read: last key | bit7=strobe)
///   $C010        Keyboard strobe clear (read/write: clears bit 7 of latch)
///   $C011-$C04F  I/O stubs (read: $00)
///   $C050-$C057  Display mode soft switches
///   $C058-$C0FF  I/O stubs (read: $00)
///   $C100-$CFFF  Slot ROM stubs (read: $00)
///   $D000-$FFFF  12K ROM
pub struct BusState {
    pub display: DisplayMode,
    pub disk: DiskII,
    pub(crate) speaker: Speaker,
    bus_cycle: u64,
    pub(crate) disk_controller_enabled: bool,
    fast_disk: bool,
    ram: [u8; 0xC000], // 48K RAM
    rom: [u8; 0x3000], // 12K ROM ($D000-$FFFF)
    rom_loaded: bool,
    kbd_latch: u8,                // $C000: keyboard latch (bit 7 = strobe)
    pub(crate) video_dirty: bool, // Set on writes to video RAM ($0400-$5FFF)
    display_mode_gen: u8,         // Incremented on display mode switch changes
}

impl BusState {
    fn new() -> Self {
        Self {
            display: DisplayMode::default(),
            disk: DiskII::new(),
            speaker: Speaker::new(),
            bus_cycle: 0,
            disk_controller_enabled: false,
            fast_disk: false,
            ram: [0; 0xC000],
            rom: [0; 0x3000],
            rom_loaded: false,
            kbd_latch: 0,
            video_dirty: true,
            display_mode_gen: 0,
        }
    }

    /// Handle display mode soft switches $C050-$C057 (shared by read and write).
    fn handle_display_switch(&mut self, addr: u16) {
        match addr {
            0xC050 => self.display.text = false,
            0xC051 => self.display.text = true,
            0xC052 => self.display.mixed = false,
            0xC053 => self.display.mixed = true,
            0xC054 => self.display.page2 = false,
            0xC055 => self.display.page2 = true,
            0xC056 => self.display.hires = false,
            0xC057 => self.display.hires = true,
            _ => {}
        }
        self.display_mode_gen = self.display_mode_gen.wrapping_add(1);
        self.video_dirty = true;
    }
}

impl Bus for BusState {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => self.ram[addr as usize],
            0xC000..=0xC00F => self.kbd_latch,
            0xC010 => {
                let val = self.kbd_latch;
                self.kbd_latch &= 0x7F;
                val
            }
            0xC030 => {
                self.speaker.toggle(self.bus_cycle);
                0
            }
            // Display mode soft switches (read triggers side effect)
            0xC050..=0xC057 => {
                self.handle_display_switch(addr);
                0
            }
            // Disk II I/O (slot 6)
            0xC0E0..=0xC0EF => {
                if self.disk_controller_enabled {
                    self.disk.io_read(addr)
                } else {
                    0x00
                }
            }
            0xC011..=0xC02F => 0x00,
            0xC031..=0xC04F => 0x00,
            0xC058..=0xC0DF => 0x00,
            0xC0F0..=0xC0FF => 0x00,
            // Disk II slot ROM ($C600-$C6FF)
            0xC600..=0xC6FF => {
                if self.disk_controller_enabled {
                    self.disk.read_slot_rom(addr)
                } else {
                    0x00
                }
            }
            0xC100..=0xCFFF => 0x00,
            0xD000..=0xFFFF => self.rom[(addr - 0xD000) as usize],
        }
    }

    fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0xBFFF => {
                self.ram[addr as usize] = val;
                let is_video_ram =
                    (0x0400..0x0C00).contains(&addr) || (0x2000..0x6000).contains(&addr);
                if is_video_ram {
                    self.video_dirty = true;
                }
            }
            0xC010 => {
                self.kbd_latch &= 0x7F;
            }
            0xC030 => {
                self.speaker.toggle(self.bus_cycle);
            }
            // Display mode soft switches (write also triggers)
            0xC050..=0xC057 => {
                self.handle_display_switch(addr);
            }
            // Disk II I/O (slot 6)
            0xC0E0..=0xC0EF => {
                if self.disk_controller_enabled {
                    self.disk.io_write(addr, val);
                }
            }
            0xC000..=0xC00F => {}
            0xC011..=0xC02F => {}
            0xC031..=0xC04F => {}
            0xC058..=0xC0DF => {}
            0xC0F0..=0xC0FF => {}
            0xC100..=0xCFFF => {}
            0xD000..=0xFFFF => {}
        }
    }

    fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => self.ram[addr as usize],
            0xC000..=0xC00F => self.kbd_latch,
            0xC010..=0xCFFF => 0,
            0xD000..=0xFFFF => self.rom[(addr - 0xD000) as usize],
        }
    }

    fn set_cycle(&mut self, cycle: u64) {
        self.bus_cycle = cycle;
    }
}

/// Apple II emulator: CPU + bus (RAM/ROM/IO).
pub struct AppleII {
    pub cpu: Cpu,
    pub bus: BusState,
}

impl AppleII {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: BusState::new(),
        }
    }

    /// Get display mode reference.
    pub fn display_mode(&self) -> &DisplayMode {
        &self.bus.display
    }

    /// Get disk controller reference.
    pub fn disk(&self) -> &DiskII {
        &self.bus.disk
    }

    /// Get mutable disk controller reference.
    pub fn disk_mut(&mut self) -> &mut DiskII {
        &mut self.bus.disk
    }

    /// Check if video RAM is dirty (needs re-render).
    pub fn is_video_dirty(&self) -> bool {
        self.bus.video_dirty
    }

    /// Mark video as clean after rendering.
    pub fn clear_video_dirty(&mut self) {
        self.bus.video_dirty = false;
    }

    /// Flush all disk drives (persist pending writes).
    pub fn flush_all_drives(&mut self) -> Result<()> {
        self.bus.disk.flush_all_drives()
    }

    /// Load a ROM file into $D000-$FFFF.
    ///
    /// Supported sizes:
    ///   - 12K (12288): $D000-$FFFF directly (Apple II, Apple II+)
    ///   - 20K (20480): $B000-$FFFF image, uses $D000-$FFFF at offset $2000 (Apple II+)
    pub fn load_rom(&mut self, path: &Path) -> Result<()> {
        let data = std::fs::read(path)?;
        self.load_rom_data(&data)
    }

    /// Load ROM data directly from a byte slice.
    pub fn load_rom_data(&mut self, data: &[u8]) -> Result<()> {
        match data.len() {
            0x3000 => {
                // 12K ROM → $D000-$FFFF (Apple II / Apple II+)
                self.bus.rom.copy_from_slice(data);
                self.bus.disk.clear_slot_rom();
            }
            0x5000 => {
                // 20K ROM → $B000-$FFFF image, use $D000-$FFFF at offset $2000
                self.bus.rom.copy_from_slice(&data[0x2000..]);
                // Extract Disk II slot 6 ROM at $C600-$C6FF (offset $1600)
                self.bus.disk.load_slot_rom(&data[0x1600..0x1700]);
            }
            _ => {
                return Err(Error::UnsupportedRomSize { actual: data.len() });
            }
        }
        self.bus.rom_loaded = true;
        Ok(())
    }

    /// Reset the CPU: reads the reset vector from $FFFC-$FFFD.
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.bus.speaker.reset(self.cpu.cycles());
    }

    /// Execute one CPU instruction. Returns cycles consumed.
    pub fn step(&mut self) -> u32 {
        // Check for RWTS trap before executing the instruction
        if self.bus.fast_disk && self.cpu.pc() == 0xB7B5 {
            if let Some(cycles) = self.try_rwts_trap() {
                return cycles;
            }
        }
        let cycles = self.cpu.step(&mut self.bus);
        self.bus.disk.tick(cycles as u64);
        cycles
    }

    /// Run the CPU for at least `target` cycles. Returns actual cycles executed.
    pub fn run_cycles(&mut self, target: u64) -> u64 {
        if self.bus.fast_disk {
            // Auto-turbo while disk motor is spinning for fast boot
            let effective = if self.bus.disk.motor_on {
                target.saturating_mul(8)
            } else {
                target
            };
            // Use run_until so the CPU runs at full speed in a tight loop,
            // breaking only when PC hits the RWTS entry point.
            let start = self.cpu.cycles();
            while self.cpu.cycles() - start < effective {
                let remaining = effective - (self.cpu.cycles() - start);
                let ran = self.cpu.run_until(&mut self.bus, remaining, 0xB7B5);
                if ran != 0 {
                    self.bus.disk.tick(ran);
                }

                if self.cpu.pc() == 0xB7B5 && self.try_rwts_trap().is_none() {
                    // Not trappable (e.g. write), step past normally
                    let cycles = self.cpu.step(&mut self.bus);
                    self.bus.disk.tick(cycles as u64);
                    debug_assert!(
                        self.cpu.pc() != 0xB7B5 || cycles == 0,
                        "PC stuck at RWTS entry after step"
                    );
                }
            }
            self.cpu.cycles() - start
        } else {
            // Normal mode: step instruction by instruction to ensure disk.tick() is called
            let start = self.cpu.cycles();
            while self.cpu.cycles() - start < target {
                let cycles = self.cpu.step(&mut self.bus);
                self.bus.disk.tick(cycles as u64);
            }
            self.cpu.cycles() - start
        }
    }

    /// Simulate a key press: sets keyboard latch with strobe bit.
    /// `ascii` should be the 7-bit ASCII value (e.g., 0x41 for 'A').
    /// The latch stores `ascii | 0x80` (bit 7 = strobe).
    pub fn key_press(&mut self, ascii: u8) {
        self.bus.kbd_latch = ascii | 0x80;
    }

    pub fn load_disk_into_drive(&mut self, path: &Path, drive: usize) -> Result<()> {
        self.bus.disk_controller_enabled = true;
        self.bus.disk.load_disk(path, drive)
    }

    /// Load a .dsk disk image into drive 1.
    pub fn load_disk(&mut self, path: &Path) -> Result<()> {
        self.load_disk_into_drive(path, 0)
    }

    /// Enable or disable Disk II slot-6 mapping.
    pub fn set_disk_controller_enabled(&mut self, enabled: bool) {
        self.bus.disk_controller_enabled = enabled;
    }

    /// Enable or disable fast-disk mode (RWTS trap).
    pub fn set_fast_disk(&mut self, enabled: bool) {
        self.bus.fast_disk = enabled;
    }

    /// Returns whether fast-disk mode is active.
    pub fn is_fast_disk(&self) -> bool {
        self.bus.fast_disk
    }

    /// Read-only access to RAM (for video rendering).
    pub fn ram(&self) -> &[u8] {
        &self.bus.ram
    }

    /// Convenience: bus read (with side effects).
    pub fn read(&mut self, addr: u16) -> u8 {
        self.bus.read(addr)
    }

    /// Convenience: bus write.
    pub fn write(&mut self, addr: u16, val: u8) {
        self.bus.write(addr, val);
    }

    /// Convenience: bus peek (no side effects).
    pub fn peek(&self, addr: u16) -> u8 {
        self.bus.peek(addr)
    }

    /// Drain synthesized speaker PCM.
    ///
    /// `real_cycles` is the wall-clock-equivalent cycle budget (before turbo/
    /// fast-disk multiplication). Audio is rendered only for this many cycles
    /// to prevent buffer accumulation during accelerated execution.
    pub fn take_audio_samples_into(
        &mut self,
        sample_rate: u32,
        real_cycles: u64,
        out: &mut Vec<f32>,
    ) {
        let render_target = self
            .bus
            .speaker
            .position()
            .saturating_add(real_cycles)
            .min(self.cpu.cycles());
        self.bus
            .speaker
            .render_until_into(render_target, sample_rate, out);
        // Fast-forward past any accelerated cycles
        self.bus.speaker.skip_to(self.cpu.cycles());
    }

    pub fn take_audio_samples(&mut self, sample_rate: u32, real_cycles: u64) -> Vec<f32> {
        let mut out = Vec::new();
        self.take_audio_samples_into(sample_rate, real_cycles, &mut out);
        out
    }
}

impl AppleII {
    /// Try to intercept RWTS at $B7B5.
    /// Returns `Some(cycles)` if the trap handled the call, `None` to fall through.
    fn try_rwts_trap(&mut self) -> Option<u32> {
        // IOB pointer from A (lo) and Y (hi)
        let iob_addr = self.cpu.a() as u16 | ((self.cpu.y() as u16) << 8);

        // Read IOB fields via peek (no side effects)
        let command = self.bus.peek(iob_addr.wrapping_add(0x0C));
        let track = self.bus.peek(iob_addr.wrapping_add(0x04));
        let sector = self.bus.peek(iob_addr.wrapping_add(0x05));
        let buf_lo = self.bus.peek(iob_addr.wrapping_add(0x08));
        let buf_hi = self.bus.peek(iob_addr.wrapping_add(0x09));
        let buf_addr = buf_lo as u16 | ((buf_hi as u16) << 8);
        let drive_num = self.bus.peek(iob_addr.wrapping_add(0x02));
        let drive_idx = if drive_num <= 1 { 0 } else { 1 };

        match command {
            0x01 => {
                // Seek: update half_track, return success
                self.bus.disk.half_track = track * 2;
                // Clear error code in IOB
                self.bus.write(iob_addr.wrapping_add(0x0D), 0);
                // Clear carry (success) and simulate RTS
                self.cpu.set_flag(|p| p.set(C, false));
                self.simulate_rts();
                Some(50)
            }
            0x02 => {
                // Read: copy sector data from raw image to RAM buffer
                if let Some(data) = self.bus.disk.read_sector_raw(drive_idx, track, sector) {
                    for (i, &byte) in data.iter().enumerate() {
                        self.bus.write(buf_addr.wrapping_add(i as u16), byte);
                    }
                    // Update half_track to match
                    self.bus.disk.half_track = track * 2;
                    // Clear error code in IOB
                    self.bus.write(iob_addr.wrapping_add(0x0D), 0);
                    // Clear carry (success) and simulate RTS
                    self.cpu.set_flag(|p| p.set(C, false));
                    self.simulate_rts();
                    Some(100)
                } else {
                    None // fall through to normal emulation
                }
            }
            0x03 => {
                let mut data = [0u8; 256];
                for (i, byte) in data.iter_mut().enumerate() {
                    *byte = self.bus.peek(buf_addr.wrapping_add(i as u16));
                }

                match self
                    .bus
                    .disk
                    .write_sector_raw(drive_idx, track, sector, &data)
                {
                    Ok(()) => {
                        self.bus.disk.half_track = track * 2;
                        self.bus.write(iob_addr.wrapping_add(0x0D), 0);
                        self.cpu.set_flag(|p| p.set(C, false));
                    }
                    Err(_) => {
                        self.bus.write(iob_addr.wrapping_add(0x0D), 0x27);
                        self.cpu.set_flag(|p| p.set(C, true));
                    }
                }

                self.simulate_rts();
                Some(140)
            }
            _ => None, // write or unknown: fall through
        }
    }

    /// Simulate an RTS by pulling the return address from the stack.
    fn simulate_rts(&mut self) {
        let sp = self.cpu.sp();
        let lo = self.bus.read(0x0100 | sp.wrapping_add(1) as u16);
        let hi = self.bus.read(0x0100 | sp.wrapping_add(2) as u16);
        self.cpu.set_sp(sp.wrapping_add(2));
        self.cpu
            .set_pc((u16::from(hi) << 8 | u16::from(lo)).wrapping_add(1));
    }
}

impl Default for AppleII {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{create_temp_disk, create_temp_rom};

    fn require_paths(paths: &[&std::path::Path]) -> bool {
        paths.iter().all(|p| p.exists())
    }

    #[test]
    fn test_ram_read_write() {
        let mut apple = AppleII::new();
        apple.write(0x0400, 0xAA);
        assert_eq!(apple.read(0x0400), 0xAA);
    }

    #[test]
    fn test_rom_write_ignored() {
        let mut apple = AppleII::new();
        apple.bus.rom[0] = 0x42;
        apple.write(0xD000, 0xFF); // should be ignored
        assert_eq!(apple.read(0xD000), 0x42);
    }

    #[test]
    fn test_keyboard_latch() {
        let mut apple = AppleII::new();
        apple.key_press(b'A'); // 0x41
        assert_eq!(apple.read(0xC000), 0xC1); // 'A' | 0x80
        assert_eq!(apple.read(0xC000), 0xC1); // still latched
    }

    #[test]
    fn test_keyboard_strobe_clear() {
        let mut apple = AppleII::new();
        apple.key_press(b'A');
        assert_eq!(apple.read(0xC000), 0xC1); // bit 7 set
        apple.read(0xC010); // clear strobe
        assert_eq!(apple.read(0xC000), 0x41); // bit 7 cleared
    }

    #[test]
    fn test_peek_no_side_effects() {
        let mut apple = AppleII::new();
        apple.key_press(b'A');
        assert_eq!(apple.peek(0xC000), 0xC1);
        // peek at $C010 should NOT clear strobe
        assert_eq!(apple.peek(0xC010), 0x00);
        assert_eq!(apple.peek(0xC000), 0xC1); // strobe still set
    }

    #[test]
    fn test_speaker_toggle_produces_pcm() {
        let mut apple = AppleII::new();

        // Approx 1kHz toggling at 1.023MHz CPU clock.
        let half_period = 512u64;
        for i in 0..200u64 {
            let cycle = i * half_period;
            apple.bus.set_cycle(cycle);
            apple.bus.read(0xC030);
        }

        let total_cycles = 200 * half_period;
        apple.cpu.set_cycles(total_cycles);

        let pcm = apple.take_audio_samples(44_100, total_cycles);
        assert!(!pcm.is_empty());
        let energy: f32 = pcm.iter().map(|v| v.abs()).sum::<f32>() / pcm.len() as f32;
        assert!(energy > 0.005);
    }

    #[test]
    fn test_disk_controller_disable_hides_slot6() {
        let mut rom = vec![0u8; 0x5000];
        rom[0x1600] = 0xD5;
        rom[0x16FF] = 0xAA;
        let temp = create_temp_rom(&rom);

        let mut apple = AppleII::new();
        apple.load_rom(temp.path()).unwrap();
        apple.set_disk_controller_enabled(true);
        assert_eq!(apple.read(0xC600), 0xD5);

        apple.set_disk_controller_enabled(false);
        assert_eq!(apple.read(0xC600), 0x00);
        assert_eq!(apple.read(0xC0EC), 0x00);
    }

    #[test]
    fn test_apple2p_no_disk_controller_stays_out_of_slot6_boot() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let rom = root.join("roms/apple2p.rom");
        if !require_paths(&[rom.as_path()]) {
            return;
        }

        let mut apple = AppleII::new();
        apple.load_rom(&rom).unwrap();
        apple.set_disk_controller_enabled(false);
        apple.reset();
        apple.run_cycles(1_000_000);

        assert!(!(0xC600..=0xC6FF).contains(&apple.cpu.pc()));
    }

    #[test]
    fn test_boot0_loads_exact_sector0() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let rom = root.join("roms/apple2p.rom");
        let disk = root.join("disks/Apple DOS 3.3 January 1983.dsk");
        if !require_paths(&[rom.as_path(), disk.as_path()]) {
            return;
        }
        let raw = std::fs::read(&disk).unwrap();
        let sector0 = &raw[0..256];

        let mut apple = AppleII::new();
        apple.load_rom(&rom).unwrap();
        apple.load_disk(&disk).unwrap();
        apple.reset();

        for _ in 0..1_000_000 {
            if apple.cpu.pc() == 0x0801 {
                break;
            }
            apple.step();
        }

        assert_eq!(apple.cpu.pc(), 0x0801);

        for (i, expected) in sector0.iter().copied().enumerate() {
            let actual = apple.peek(0x0800 + i as u16);
            assert_eq!(actual, expected, "mismatch at byte {:02X}", i);
        }
    }

    #[test]
    fn test_dos33_boot_progresses_to_track2() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let rom = root.join("roms/apple2p.rom");
        let disk = root.join("disks/Apple DOS 3.3 January 1983.dsk");
        if !require_paths(&[rom.as_path(), disk.as_path()]) {
            return;
        }

        let mut apple = AppleII::new();
        apple.load_rom(&rom).unwrap();
        apple.load_disk(&disk).unwrap();
        apple.reset();
        let mut max_track = 0u8;
        for _ in 0..3_000_000 {
            apple.step();
            let track = apple.bus.disk.half_track / 2;
            if track > max_track {
                max_track = track;
            }
        }

        assert!(
            max_track >= 2,
            "pc={:04X} track={} max_track={} motor={} 0400={:02X} 0427={:02X} 07D0={:02X}",
            apple.cpu.pc(),
            apple.bus.disk.half_track / 2,
            max_track,
            apple.bus.disk.motor_on,
            apple.peek(0x0400),
            apple.peek(0x0427),
            apple.peek(0x07D0)
        );
    }

    #[test]
    fn test_rwts_write_trap_writes_sector_data() {
        let mut rom = vec![0u8; 0x5000];
        rom[0x2FFC] = 0x00;
        rom[0x2FFD] = 0xD0;
        let rom_temp = create_temp_rom(&rom);

        let raw_disk = vec![0u8; 143_360];
        let disk_temp = create_temp_disk(&raw_disk);

        let mut apple = AppleII::new();
        apple.load_rom(rom_temp.path()).unwrap();
        apple.load_disk(disk_temp.path()).unwrap();

        let iob = 0x0200u16;
        let buf = 0x0300u16;

        for i in 0..256u16 {
            apple.write(
                buf.wrapping_add(i),
                (i as u8).wrapping_mul(5).wrapping_add(1),
            );
        }

        apple.write(iob.wrapping_add(0x02), 1);
        apple.write(iob.wrapping_add(0x04), 0);
        apple.write(iob.wrapping_add(0x05), 0);
        apple.write(iob.wrapping_add(0x08), (buf & 0xFF) as u8);
        apple.write(iob.wrapping_add(0x09), (buf >> 8) as u8);
        apple.write(iob.wrapping_add(0x0C), 0x03);
        apple.write(iob.wrapping_add(0x0D), 0xFF);

        apple.cpu.set_a((iob & 0xFF) as u8);
        apple.cpu.set_y((iob >> 8) as u8);
        apple.cpu.set_sp(0xFD);
        apple.write(0x01FE, 0x34);
        apple.write(0x01FF, 0x12);

        let trap_cycles = apple.try_rwts_trap();
        assert_eq!(trap_cycles, Some(140));
        assert_eq!(apple.peek(iob.wrapping_add(0x0D)), 0x00);
        assert!(!apple.cpu.p().get(C));
        assert_eq!(apple.cpu.pc(), 0x1235);

        let sector = apple.bus.disk.read_sector_raw(0, 0, 0).unwrap();
        for (i, &actual) in sector.iter().enumerate() {
            assert_eq!(actual, (i as u8).wrapping_mul(5).wrapping_add(1));
        }
    }

    #[test]
    fn test_12k_rom_clears_slot_rom() {
        let mut rom_20k = vec![0u8; 0x5000];
        rom_20k[0x1600] = 0xD5;
        rom_20k[0x16FF] = 0xAA;

        let mut apple = AppleII::new();
        apple.load_rom_data(&rom_20k).unwrap();
        apple.set_disk_controller_enabled(true);
        assert_eq!(apple.read(0xC600), 0xD5);
        assert_eq!(apple.read(0xC6FF), 0xAA);

        let rom_12k = vec![0xAAu8; 0x3000];
        apple.load_rom_data(&rom_12k).unwrap();
        assert_eq!(apple.read(0xC600), 0x00);
        assert_eq!(apple.read(0xC6FF), 0x00);
    }
}
