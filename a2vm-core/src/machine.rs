use std::io;
use std::mem;
use std::path::Path;

use crate::bus::Bus;
use crate::cpu::Cpu;
use crate::disk::DiskII;
use crate::video::DisplayMode;

/// Apple II emulator: CPU + memory + keyboard I/O.
///
/// Memory map (M2 minimal):
///   $0000-$BFFF  48K RAM
///   $C000        Keyboard latch (read: last key | bit7=strobe)
///   $C010        Keyboard strobe clear (read/write: clears bit 7 of latch)
///   $C011-$C04F  I/O stubs (read: $00)
///   $C050-$C057  Display mode soft switches
///   $C058-$C0FF  I/O stubs (read: $00)
///   $C100-$CFFF  Slot ROM stubs (read: $00)
///   $D000-$FFFF  12K ROM
pub struct AppleII {
    pub cpu: Cpu,
    pub display: DisplayMode,
    pub disk: DiskII,
    disk_controller_enabled: bool,
    ram: [u8; 0xC000], // 48K RAM
    rom: [u8; 0x3000], // 12K ROM ($D000-$FFFF)
    rom_loaded: bool,
    kbd_latch: u8, // $C000: keyboard latch (bit 7 = strobe)
}

impl AppleII {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            display: DisplayMode::default(),
            disk: DiskII::new(),
            disk_controller_enabled: true,
            ram: [0; 0xC000],
            rom: [0; 0x3000],
            rom_loaded: false,
            kbd_latch: 0,
        }
    }

    /// Load a ROM file into $D000-$FFFF.
    ///
    /// Supported sizes:
    ///   - 12K (12288): $D000-$FFFF directly (Apple II, Apple II+)
    ///   - 16K (16384): $C000-$FFFF, uses $D000-$FFFF portion (Apple IIe)
    ///   - 20K (20480): $B000-$FFFF image, uses $D000-$FFFF at offset $2000 (Apple II+)
    ///   - 32K (32768): Two 16K banks, uses first bank's $D000-$FFFF (Apple IIe)
    pub fn load_rom(&mut self, path: &Path) -> io::Result<()> {
        let data = std::fs::read(path)?;
        match data.len() {
            0x3000 => {
                // 12K ROM → $D000-$FFFF (Apple II / Apple II+)
                self.rom.copy_from_slice(&data);
            }
            0x4000 => {
                // 16K ROM → skip $C000-$CFFF, use $D000-$FFFF (Apple IIe)
                self.rom.copy_from_slice(&data[0x1000..]);
                // Extract Disk II slot 6 ROM at $C600-$C6FF (offset $0600)
                self.disk.load_slot_rom(&data[0x0600..0x0700]);
            }
            0x5000 => {
                // 20K ROM → $B000-$FFFF image, use $D000-$FFFF at offset $2000
                self.rom.copy_from_slice(&data[0x2000..]);
                // Extract Disk II slot 6 ROM at $C600-$C6FF (offset $1600)
                self.disk.load_slot_rom(&data[0x1600..0x1700]);
            }
            0x8000 => {
                // 32K ROM → first 16K bank's $D000-$FFFF (Apple IIe)
                self.rom.copy_from_slice(&data[0x1000..0x4000]);
                // Extract Disk II slot 6 ROM at $C600-$C6FF from first bank (offset $0600)
                self.disk.load_slot_rom(&data[0x0600..0x0700]);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "ROM must be 12K, 16K, 20K, or 32K bytes, got {} ({:#X})",
                        data.len(),
                        data.len()
                    ),
                ));
            }
        }
        self.rom_loaded = true;
        Ok(())
    }

    /// Reset the CPU: reads the reset vector from $FFFC-$FFFD.
    pub fn reset(&mut self) {
        let mut cpu = mem::take(&mut self.cpu);
        cpu.reset(self);
        self.cpu = cpu;
    }

    /// Execute one CPU instruction. Returns cycles consumed.
    pub fn step(&mut self) -> u32 {
        let mut cpu = mem::take(&mut self.cpu);
        let cycles = cpu.step(self);
        self.cpu = cpu;
        self.disk.tick(cycles);
        cycles
    }

    /// Run the CPU for at least `target` cycles. Returns actual cycles executed.
    pub fn run_cycles(&mut self, target: u64) -> u64 {
        let mut cpu = mem::take(&mut self.cpu);
        let cycles = cpu.run(self, target);
        self.cpu = cpu;
        cycles
    }

    /// Simulate a key press: sets keyboard latch with strobe bit.
    /// `ascii` should be the 7-bit ASCII value (e.g., 0x41 for 'A').
    /// The latch stores `ascii | 0x80` (bit 7 = strobe).
    pub fn key_press(&mut self, ascii: u8) {
        self.kbd_latch = ascii | 0x80;
    }

    /// Load a .dsk disk image into drive 1.
    pub fn load_disk(&mut self, path: &Path) -> io::Result<()> {
        self.disk_controller_enabled = true;
        self.disk.load_disk(path, 0)
    }

    /// Enable or disable Disk II slot-6 mapping.
    pub fn set_disk_controller_enabled(&mut self, enabled: bool) {
        self.disk_controller_enabled = enabled;
    }

    /// Read-only access to RAM (for video rendering).
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }

    /// Peek at any address without side effects (for debug/status display).
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => self.ram[addr as usize],
            0xC000..=0xC00F => self.kbd_latch,
            0xC010..=0xCFFF => 0,
            0xD000..=0xFFFF => self.rom[(addr - 0xD000) as usize],
        }
    }
}

impl Bus for AppleII {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0xBFFF => self.ram[addr as usize],
            0xC000..=0xC00F => self.kbd_latch,
            0xC010 => {
                let val = self.kbd_latch;
                self.kbd_latch &= 0x7F;
                val
            }
            // Display mode soft switches (accent on read triggers side effect)
            0xC050 => {
                self.display.text = false;
                0
            }
            0xC051 => {
                self.display.text = true;
                0
            }
            0xC052 => {
                self.display.mixed = false;
                0
            }
            0xC053 => {
                self.display.mixed = true;
                0
            }
            0xC054 => {
                self.display.page2 = false;
                0
            }
            0xC055 => {
                self.display.page2 = true;
                0
            }
            0xC056 => {
                self.display.hires = false;
                0
            }
            0xC057 => {
                self.display.hires = true;
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
            0xC011..=0xC0FF => 0x00,
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
            0x0000..=0xBFFF => self.ram[addr as usize] = val,
            0xC010 => {
                self.kbd_latch &= 0x7F;
            }
            // Display mode soft switches (write also triggers)
            0xC050 => {
                self.display.text = false;
            }
            0xC051 => {
                self.display.text = true;
            }
            0xC052 => {
                self.display.mixed = false;
            }
            0xC053 => {
                self.display.mixed = true;
            }
            0xC054 => {
                self.display.page2 = false;
            }
            0xC055 => {
                self.display.page2 = true;
            }
            0xC056 => {
                self.display.hires = false;
            }
            0xC057 => {
                self.display.hires = true;
            }
            // Disk II I/O (slot 6)
            0xC0E0..=0xC0EF => {
                if self.disk_controller_enabled {
                    self.disk.io_write(addr, val);
                }
            }
            0xC000..=0xC0FF => {}
            0xC100..=0xCFFF => {}
            0xD000..=0xFFFF => {}
        }
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
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_file(bytes: &[u8], suffix: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("a2vm-rom-test-{nanos}-{suffix}.bin"));
        fs::write(&path, bytes).unwrap();
        path
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
        apple.rom[0] = 0x42;
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
    fn test_load_16k_rom_loads_slot6_rom() {
        let mut rom = vec![0u8; 0x4000];
        rom[0x0600] = 0xD5;
        rom[0x06FF] = 0xAA;
        let path = write_temp_file(&rom, "16k");

        let mut apple = AppleII::new();
        apple.load_rom(&path).unwrap();

        assert_eq!(apple.read(0xC600), 0xD5);
        assert_eq!(apple.read(0xC6FF), 0xAA);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_load_32k_rom_loads_slot6_rom_from_first_bank() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0600] = 0xD5;
        rom[0x06FF] = 0xAA;
        // Put different bytes in second bank to ensure first bank is used.
        rom[0x4600] = 0x11;
        rom[0x46FF] = 0x22;
        let path = write_temp_file(&rom, "32k");

        let mut apple = AppleII::new();
        apple.load_rom(&path).unwrap();

        assert_eq!(apple.read(0xC600), 0xD5);
        assert_eq!(apple.read(0xC6FF), 0xAA);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_disk_controller_disable_hides_slot6() {
        let mut rom = vec![0u8; 0x4000];
        rom[0x0600] = 0xD5;
        rom[0x06FF] = 0xAA;
        let path = write_temp_file(&rom, "16k-disable");

        let mut apple = AppleII::new();
        apple.load_rom(&path).unwrap();
        assert_eq!(apple.read(0xC600), 0xD5);

        apple.set_disk_controller_enabled(false);
        assert_eq!(apple.read(0xC600), 0x00);
        assert_eq!(apple.read(0xC0EC), 0x00);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_apple2p_no_disk_controller_stays_out_of_slot6_boot() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let rom = root.join("roms/apple2p.rom");

        let mut apple = AppleII::new();
        apple.load_rom(&rom).unwrap();
        apple.set_disk_controller_enabled(false);
        apple.reset();
        apple.run_cycles(1_000_000);

        assert!(!(0xC600..=0xC6FF).contains(&apple.cpu.pc));
    }

    #[test]
    fn test_boot0_loads_exact_sector0() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let rom = root.join("roms/apple2p.rom");
        let disk = root.join("disks/Apple DOS 3.3 January 1983.dsk");
        let raw = std::fs::read(&disk).unwrap();
        let sector0 = &raw[0..256];

        let mut apple = AppleII::new();
        apple.load_rom(&rom).unwrap();
        apple.load_disk(&disk).unwrap();
        apple.reset();

        for _ in 0..1_000_000 {
            if apple.cpu.pc == 0x0801 {
                break;
            }
            apple.step();
        }

        assert_eq!(apple.cpu.pc, 0x0801);

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

        let mut apple = AppleII::new();
        apple.load_rom(&rom).unwrap();
        apple.load_disk(&disk).unwrap();
        apple.reset();
        let mut max_track = 0u8;
        for _ in 0..3_000_000 {
            apple.step();
            let track = apple.disk.half_track / 2;
            if track > max_track {
                max_track = track;
            }
        }

        assert!(
            max_track >= 2,
            "pc={:04X} track={} max_track={} motor={} 0400={:02X} 0427={:02X} 07D0={:02X}",
            apple.cpu.pc,
            apple.disk.half_track / 2,
            max_track,
            apple.disk.motor_on,
            apple.peek(0x0400),
            apple.peek(0x0427),
            apple.peek(0x07D0)
        );
    }
}
