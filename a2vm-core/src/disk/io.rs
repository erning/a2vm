use super::{DiskII, NIBBLE_TRACK_SIZE, PHASE_DELTA};

impl DiskII {
    /// Handle I/O read at $C0E0-$C0EF.
    pub fn io_read(&mut self, addr: u16) -> u8 {
        let switch = (addr & 0x0F) as u8;
        self.handle_switch(switch);

        // Return register selected by Q6/Q7 after switch side effects.
        if self.q7 {
            // Write mode not implemented yet.
            0
        } else if self.q6 {
            // Status register: write-protect sense in bit 7.
            let drv = &self.drives[self.selected_drive];
            if drv.write_protected {
                0x80
            } else {
                0x00
            }
        } else {
            // Data register: bit 7 indicates fresh data in latch.
            if !self.data_ready && self.motor_on && self.drives[self.selected_drive].has_disk {
                self.read_latch = self.read_nibble();
                self.data_ready = true;
            }
            let out = if self.data_ready {
                self.read_latch
            } else {
                self.read_latch & 0x7F
            };
            self.data_ready = false;
            out
        }
    }

    /// Handle I/O write at $C0E0-$C0EF.
    pub fn io_write(&mut self, addr: u16, val: u8) {
        let switch = (addr & 0x0F) as u8;
        self.handle_switch(switch);
        if self.q6 && self.q7 {
            // Q6 on + Q7 on = write mode: latch data
            self.write_latch = val;
            self.write_nibble(val);
        }
    }

    /// Process a soft-switch toggle.
    fn handle_switch(&mut self, switch: u8) {
        match switch {
            0x00 => self.set_phase(0, false),
            0x01 => self.set_phase(0, true),
            0x02 => self.set_phase(1, false),
            0x03 => self.set_phase(1, true),
            0x04 => self.set_phase(2, false),
            0x05 => self.set_phase(2, true),
            0x06 => self.set_phase(3, false),
            0x07 => self.set_phase(3, true),
            0x08 => {
                if self.motor_on {
                    if let Err(e) = self.sync_nibble_to_raw(self.selected_drive) {
                        self.last_error = Some(e);
                    }
                }
                self.motor_on = false;
                self.data_ready = false;
            }
            0x09 => self.motor_on = true,
            0x0A => self.selected_drive = 0,
            0x0B => self.selected_drive = 1,
            0x0C => self.q6 = false,
            0x0D => self.q6 = true,
            0x0E => self.q7 = false,
            0x0F => self.q7 = true,
            _ => {}
        }
    }

    /// Stepper motor phase control.
    pub(super) fn set_phase(&mut self, phase: usize, on: bool) {
        self.phases[phase] = on;
        if !on || !self.motor_on {
            return;
        }

        let current_phase = (self.half_track as usize) % 4;
        let delta = PHASE_DELTA[current_phase][phase] as i16;
        let next = (self.half_track as i16 + delta).clamp(0, 69);
        self.half_track = next as u8;
    }

    /// Read one nibble from the current track position and advance rotation.
    pub(super) fn read_nibble(&mut self) -> u8 {
        let drv = &mut self.drives[self.selected_drive];
        if !drv.has_disk {
            return 0;
        }
        let track = (self.half_track / 2) as usize;
        let val = drv.nibble_data[track][drv.byte_position];
        drv.byte_position += 1;
        if drv.byte_position >= NIBBLE_TRACK_SIZE {
            drv.byte_position = 0;
        }
        val
    }

    fn write_nibble(&mut self, val: u8) {
        let drv = &mut self.drives[self.selected_drive];
        if !drv.has_disk || drv.write_protected || !self.motor_on {
            return;
        }
        let track = (self.half_track / 2) as usize;
        drv.nibble_data[track][drv.byte_position] = val;
        drv.byte_position += 1;
        if drv.byte_position >= NIBBLE_TRACK_SIZE {
            drv.byte_position = 0;
        }
        drv.dirty = true;
        drv.dirty_tracks[track] = true;
    }
}
