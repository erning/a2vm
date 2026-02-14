use super::*;

impl AppleII {
    /// Try to intercept RWTS at $B7B5.
    /// Returns `Some(cycles)` if the trap handled the call, `None` to fall through.
    pub(super) fn try_rwts_trap(&mut self) -> Option<u32> {
        // IOB pointer from A (lo) and Y (hi)
        let iob_addr = self.cpu.a() as u16 | ((self.cpu.y() as u16) << 8);

        // Read IOB fields via peek (no side effects)
        let command = self
            .bus
            .peek(iob_addr.wrapping_add(RWTS_IOB_COMMAND_OFFSET));
        let track = self.bus.peek(iob_addr.wrapping_add(RWTS_IOB_TRACK_OFFSET));
        let sector = self.bus.peek(iob_addr.wrapping_add(RWTS_IOB_SECTOR_OFFSET));
        let buf_lo = self
            .bus
            .peek(iob_addr.wrapping_add(RWTS_IOB_BUFFER_LO_OFFSET));
        let buf_hi = self
            .bus
            .peek(iob_addr.wrapping_add(RWTS_IOB_BUFFER_HI_OFFSET));
        let buf_addr = buf_lo as u16 | ((buf_hi as u16) << 8);
        let drive_num = self.bus.peek(iob_addr.wrapping_add(RWTS_IOB_DRIVE_OFFSET));
        let drive_idx = if drive_num <= 1 { 0 } else { 1 };

        match command {
            RWTS_CMD_SEEK => {
                // Seek: update half_track, return success
                self.bus.disk.half_track = track * 2;
                // Clear error code in IOB
                self.bus
                    .write(iob_addr.wrapping_add(RWTS_IOB_ERROR_OFFSET), RWTS_ERROR_OK);
                // Clear carry (success) and simulate RTS
                self.cpu.set_flag(|p| p.set(C, false));
                self.simulate_rts();
                Some(50)
            }
            RWTS_CMD_READ => {
                // Read: copy sector data from raw image to RAM buffer
                if let Some(data) = self.bus.disk.read_sector_raw(drive_idx, track, sector) {
                    for (i, &byte) in data.iter().enumerate() {
                        self.bus.write(buf_addr.wrapping_add(i as u16), byte);
                    }
                    // Update half_track to match
                    self.bus.disk.half_track = track * 2;
                    // Clear error code in IOB
                    self.bus
                        .write(iob_addr.wrapping_add(RWTS_IOB_ERROR_OFFSET), RWTS_ERROR_OK);
                    // Clear carry (success) and simulate RTS
                    self.cpu.set_flag(|p| p.set(C, false));
                    self.simulate_rts();
                    Some(100)
                } else {
                    None // fall through to normal emulation
                }
            }
            RWTS_CMD_WRITE => {
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
                        self.bus
                            .write(iob_addr.wrapping_add(RWTS_IOB_ERROR_OFFSET), RWTS_ERROR_OK);
                        self.cpu.set_flag(|p| p.set(C, false));
                    }
                    Err(_) => {
                        self.bus
                            .write(iob_addr.wrapping_add(RWTS_IOB_ERROR_OFFSET), RWTS_ERROR_IO);
                        self.cpu.set_flag(|p| p.set(C, true));
                    }
                }

                self.simulate_rts();
                Some(140)
            }
            _ => None, // unknown command: fall through
        }
    }

    /// Simulate an RTS by pulling the return address from the stack.
    fn simulate_rts(&mut self) {
        let sp = self.cpu.sp();
        let lo = self.bus.peek(0x0100 | sp.wrapping_add(1) as u16);
        let hi = self.bus.peek(0x0100 | sp.wrapping_add(2) as u16);
        self.cpu.set_sp(sp.wrapping_add(2));
        self.cpu
            .set_pc((u16::from(hi) << 8 | u16::from(lo)).wrapping_add(1));
    }
}
