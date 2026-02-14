use super::*;

impl AppleII {
    /// Execute one CPU instruction. Returns cycles consumed.
    pub fn step(&mut self) -> u32 {
        // Check for RWTS trap before executing the instruction.
        if let Some(cycles) = self.handle_rwts_trap() {
            return cycles;
        }
        let cycles = self.cpu.step(&mut self.bus);
        self.bus.disk.tick(cycles);
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
                let ran = self.cpu.run_until(&mut self.bus, remaining, RWTS_ENTRY_PC);
                if ran != 0 {
                    self.bus.disk.tick(ran.min(u32::MAX as u64) as u32);
                }

                if self.cpu.pc() == RWTS_ENTRY_PC && self.handle_rwts_trap().is_none() {
                    // Not trappable, step past normally.
                    let cycles = self.cpu.step(&mut self.bus);
                    self.bus.disk.tick(cycles);
                }
            }
            self.cpu.cycles() - start
        } else {
            // Normal mode: step instruction by instruction to ensure disk.tick() is called
            let start = self.cpu.cycles();
            while self.cpu.cycles() - start < target {
                let cycles = self.cpu.step(&mut self.bus);
                self.bus.disk.tick(cycles);
            }
            self.cpu.cycles() - start
        }
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

    fn handle_rwts_trap(&mut self) -> Option<u32> {
        if !self.bus.fast_disk || self.cpu.pc() != RWTS_ENTRY_PC {
            return None;
        }
        let cycles = self.try_rwts_trap()?;
        self.cpu.add_cycles(cycles);
        self.bus.disk.tick(cycles);
        Some(cycles)
    }
}
