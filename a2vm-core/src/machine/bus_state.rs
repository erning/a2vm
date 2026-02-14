use super::*;

impl BusState {
    pub(super) fn new() -> Self {
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
