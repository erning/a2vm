use std::collections::VecDeque;

use crate::timing::CPU_HZ;

/// Speaker toggle timeline -> PCM conversion.
///
/// The Apple II speaker is a 1-bit device toggled by accesses to $C030.
/// We record toggle cycle timestamps and synthesize audio samples from them.
pub struct Speaker {
    state: bool,
    toggles: VecDeque<u64>,
    next_sample_cycle: f64,
    hp_prev_x: f32,
    hp_prev_y: f32,
    volume: f32,
}

impl Speaker {
    pub fn new() -> Self {
        Self {
            state: false,
            toggles: VecDeque::with_capacity(2048),
            next_sample_cycle: 0.0,
            hp_prev_x: 0.0,
            hp_prev_y: 0.0,
            volume: 1.0,
        }
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn reset(&mut self, cycle: u64) {
        self.state = false;
        self.toggles.clear();
        self.next_sample_cycle = cycle as f64;
        self.hp_prev_x = 0.0;
        self.hp_prev_y = 0.0;
    }

    /// Register one speaker toggle at the given CPU cycle.
    pub fn toggle(&mut self, cycle: u64) {
        self.toggles.push_back(cycle);
    }

    /// Current audio position in CPU cycles.
    pub fn position(&self) -> u64 {
        self.next_sample_cycle as u64
    }

    /// Fast-forward audio position to the given cycle without generating samples.
    /// Processes any speaker toggles in the skipped interval to keep state correct.
    pub fn skip_to(&mut self, cycle: u64) {
        let target = cycle as f64;
        if target <= self.next_sample_cycle {
            return;
        }
        while let Some(&edge) = self.toggles.front() {
            if (edge as f64) <= target {
                self.state = !self.state;
                self.toggles.pop_front();
            } else {
                break;
            }
        }
        self.next_sample_cycle = target;
    }

    /// Synthesize PCM samples up to `target_cycle`.
    pub fn render_until_into(&mut self, target_cycle: u64, sample_rate: u32, out: &mut Vec<f32>) {
        out.clear();
        if sample_rate == 0 {
            return;
        }

        let target = target_cycle as f64;
        if target <= self.next_sample_cycle {
            return;
        }

        let cycles_per_sample = CPU_HZ as f64 / sample_rate as f64;
        let expected = ((target - self.next_sample_cycle) / cycles_per_sample).ceil() as usize;
        out.reserve(expected);

        while self.next_sample_cycle < target {
            while let Some(&edge) = self.toggles.front() {
                if edge as f64 <= self.next_sample_cycle {
                    self.state = !self.state;
                    self.toggles.pop_front();
                } else {
                    break;
                }
            }

            let raw = if self.state { 0.25 } else { -0.25 };
            let raw = raw * self.volume;
            // High-pass to remove DC offset from 1-bit speaker state.
            let y = raw - self.hp_prev_x + 0.995 * self.hp_prev_y;
            self.hp_prev_x = raw;
            self.hp_prev_y = y;
            out.push(y);

            self.next_sample_cycle += cycles_per_sample;
        }
    }

    pub fn render_until(&mut self, target_cycle: u64, sample_rate: u32) -> Vec<f32> {
        let mut out = Vec::new();
        self.render_until_into(target_cycle, sample_rate, &mut out);
        out
    }
}

impl Default for Speaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_stream_generates_samples() {
        let mut sp = Speaker::new();
        let sr = 44_100;
        // ~1 kHz square wave: toggle every half period.
        let half_period_cycles = (CPU_HZ as f64 / 2000.0) as u64;
        let end = 100_000u64;
        let mut c = 0u64;
        while c <= end {
            sp.toggle(c);
            c += half_period_cycles;
        }
        let pcm = sp.render_until(end, sr);
        assert!(!pcm.is_empty());
        let energy: f32 = pcm.iter().map(|v| v.abs()).sum::<f32>() / pcm.len() as f32;
        assert!(energy > 0.01);
    }
}
