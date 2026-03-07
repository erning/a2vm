use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn write_temp_file(bytes: &[u8], suffix: &str) -> std::result::Result<TempFile, Box<dyn std::error::Error>> {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    path.push(format!("a2vm-rom-test-{nanos}-{suffix}.bin"));
    fs::write(&path, bytes)?;
    Ok(TempFile { path })
}

fn write_temp_disk(bytes: &[u8], suffix: &str) -> std::result::Result<TempFile, Box<dyn std::error::Error>> {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    path.push(format!("a2vm-disk-test-{nanos}-{suffix}.dsk"));
    fs::write(&path, bytes)?;
    Ok(TempFile { path })
}

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
fn test_disk_controller_disable_hides_slot6() -> TestResult {
    let mut rom = vec![0u8; 0x5000];
    rom[0x1600] = 0xD5;
    rom[0x16FF] = 0xAA;
    let temp = write_temp_file(&rom, "20k-disable")?;

    let mut apple = AppleII::new();
    apple.load_rom(temp.path())?;
    apple.set_disk_controller_enabled(true);
    assert_eq!(apple.read(0xC600), 0xD5);

    apple.set_disk_controller_enabled(false);
    assert_eq!(apple.read(0xC600), 0x00);
    assert_eq!(apple.read(0xC0EC), 0x00);
    Ok(())
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
fn test_rwts_write_trap_writes_sector_data() -> TestResult {
    let mut rom = vec![0u8; 0x5000];
    rom[0x2FFC] = 0x00;
    rom[0x2FFD] = 0xD0;
    let rom_temp = write_temp_file(&rom, "rwts-write-rom")?;

    let raw_disk = vec![0u8; 143_360];
    let disk_temp = write_temp_disk(&raw_disk, "rwts-write-disk")?;

    let mut apple = AppleII::new();
    apple.load_rom(rom_temp.path())?;
    apple.load_disk(disk_temp.path())?;

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
    Ok(())
}

#[test]
fn test_step_rwts_trap_advances_cpu_cycles() {
    let mut apple = AppleII::new();
    apple.set_fast_disk(true);

    let iob = 0x0200u16;
    apple.write(iob.wrapping_add(RWTS_IOB_COMMAND_OFFSET), RWTS_CMD_SEEK);
    apple.write(iob.wrapping_add(RWTS_IOB_TRACK_OFFSET), 3);
    apple.write(iob.wrapping_add(RWTS_IOB_SECTOR_OFFSET), 0);
    apple.write(iob.wrapping_add(RWTS_IOB_DRIVE_OFFSET), 1);
    apple.write(iob.wrapping_add(RWTS_IOB_ERROR_OFFSET), 0xFF);

    apple.cpu.set_a((iob & 0xFF) as u8);
    apple.cpu.set_y((iob >> 8) as u8);
    apple.cpu.set_pc(RWTS_ENTRY_PC);
    apple.cpu.set_sp(0xFD);
    apple.write(0x01FE, 0x78);
    apple.write(0x01FF, 0x56);

    let before = apple.cpu.cycles();
    let consumed = apple.step();

    assert_eq!(consumed, 50);
    assert_eq!(apple.cpu.cycles() - before, 50);
    assert_eq!(apple.bus.disk.half_track, 6);
    assert_eq!(
        apple.peek(iob.wrapping_add(RWTS_IOB_ERROR_OFFSET)),
        RWTS_ERROR_OK
    );
    assert!(!apple.cpu.p().get(C));
    assert_eq!(apple.cpu.pc(), 0x5679);
}

#[test]
fn test_12k_rom_clears_slot_rom() -> TestResult {
    let mut rom_20k = vec![0u8; 0x5000];
    rom_20k[0x1600] = 0xD5;
    rom_20k[0x16FF] = 0xAA;

    let mut apple = AppleII::new();
    apple.load_rom_data(&rom_20k)?;
    apple.set_disk_controller_enabled(true);
    assert_eq!(apple.read(0xC600), 0xD5);
    assert_eq!(apple.read(0xC6FF), 0xAA);

    let rom_12k = vec![0xAAu8; 0x3000];
    apple.load_rom_data(&rom_12k)?;
    assert_eq!(apple.read(0xC600), 0x00);
    assert_eq!(apple.read(0xC6FF), 0x00);
    Ok(())
}
