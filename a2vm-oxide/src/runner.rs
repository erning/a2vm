//! Shared emulator runtime for TUI and GUI frontends.
//!
//! This module provides `EmulatorRunner`, which encapsulates the common
//! emulation loop logic: cycle accumulation, turbo mode, audio output,
//! mechanical noise, and performance statistics.

use std::borrow::Cow;
use std::path::Path;
use std::time::{Duration, Instant};

use a2vm_core::machine::AppleII;
use a2vm_core::timing::CPU_HZ;

#[cfg(feature = "audio")]
use rodio::buffer::SamplesBuffer;
#[cfg(feature = "audio")]
use rodio::source::Source;
#[cfg(feature = "audio")]
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
#[cfg(feature = "audio")]
use std::io::Cursor;

#[cfg(feature = "audio")]
use crate::noise::{DiskMechTracker, MechanicalEvent, MOVE_ARM_WAV};

/// Turbo multiplier for accelerated emulation.
const TURBO_MULTIPLIER: u64 = 4;

/// Maximum delta time to prevent spiral of death (100ms).
const MAX_DT: Duration = Duration::from_millis(100);

/// Default audio sample rate.
#[cfg(feature = "audio")]
const AUDIO_SAMPLE_RATE: u32 = 44_100;

/// Performance statistics sample interval.
const PERF_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);

/// The result of a single emulation tick.
#[derive(Debug, Clone)]
pub struct TickResult {
    /// Whether the emulation actually ran cycles.
    pub ran_cycles: bool,
    /// Real cycles executed (before turbo multiplier).
    pub real_cycles: u64,
    /// Current emulation speed in MHz.
    pub emu_mhz: f64,
}

/// Shared emulator runtime for TUI and GUI frontends.
///
/// Encapsulates the common emulation loop logic:
/// - Cycle accumulation and turbo mode
/// - Audio output (optional, via "audio" feature)
/// - Mechanical noise simulation (optional, via "audio" feature)
/// - Performance statistics
pub struct EmulatorRunner {
    apple: AppleII,
    turbo: bool,

    // Timing state
    last_tick: Instant,
    cycle_accum: u128,

    // Performance stats
    perf_last_time: Instant,
    perf_last_cycles: u64,
    emu_mhz: f64,

    // Audio (optional)
    #[cfg(feature = "audio")]
    _audio_stream: Option<OutputStream>,
    #[cfg(feature = "audio")]
    audio_sink: Option<Sink>,
    #[cfg(feature = "audio")]
    audio_buffer: Vec<f32>,

    // Mechanical noise (optional)
    #[cfg(feature = "audio")]
    mech_sink: Option<Sink>,
    #[cfg(feature = "audio")]
    mech_tracker: DiskMechTracker,
    #[cfg(feature = "audio")]
    noise_enabled: bool,
}

impl EmulatorRunner {
    /// Create a new emulator runner with the given ROM and disk configuration.
    ///
    /// # Arguments
    /// - `rom_data`: ROM data (12K or 20K)
    /// - `disks`: Disk image paths to load
    /// - `fast_disk`: Enable fast disk mode (RWTS trap)
    #[cfg(not(feature = "audio"))]
    pub fn new(
        rom_data: Cow<'_, [u8]>,
        disks: &[&Path],
        fast_disk: bool,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_inner(rom_data, disks, fast_disk, false)
    }

    /// Create a new emulator runner with the given ROM and disk configuration.
    ///
    /// # Arguments
    /// - `rom_data`: ROM data (12K or 20K)
    /// - `disks`: Disk image paths to load
    /// - `fast_disk`: Enable fast disk mode (RWTS trap)
    /// - `noise`: Enable mechanical noise simulation
    #[cfg(feature = "audio")]
    pub fn new(
        rom_data: Cow<'_, [u8]>,
        disks: &[&Path],
        fast_disk: bool,
        noise: bool,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_inner(rom_data, disks, fast_disk, noise)
    }

    fn new_inner(
        rom_data: Cow<'_, [u8]>,
        disks: &[&Path],
        fast_disk: bool,
        #[cfg(feature = "audio")] noise: bool,
        #[cfg(not(feature = "audio"))] _noise: bool,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut apple = AppleII::new();
        apple.load_rom_data(&rom_data)?;

        apple.set_disk_controller_enabled(!disks.is_empty());

        for (drive, disk) in disks.iter().enumerate() {
            apple.load_disk_into_drive(disk, drive)?;
        }

        if fast_disk {
            apple.set_fast_disk(true);
        }

        apple.reset();

        #[cfg(feature = "audio")]
        let (audio_stream, audio_sink, mech_sink) = match OutputStreamBuilder::open_default_stream()
        {
            Ok(stream) => {
                let speaker = Sink::connect_new(stream.mixer());
                let mech = Sink::connect_new(stream.mixer());
                (Some(stream), Some(speaker), Some(mech))
            }
            Err(_) => (None, None, None),
        };

        let now = Instant::now();

        Ok(Self {
            apple,
            turbo: false,
            last_tick: now,
            cycle_accum: 0,
            perf_last_time: now,
            perf_last_cycles: 0,
            emu_mhz: 0.0,
            #[cfg(feature = "audio")]
            _audio_stream: audio_stream,
            #[cfg(feature = "audio")]
            audio_sink,
            #[cfg(feature = "audio")]
            audio_buffer: Vec::with_capacity(4096),
            #[cfg(feature = "audio")]
            mech_sink,
            #[cfg(feature = "audio")]
            mech_tracker: DiskMechTracker::new(),
            #[cfg(feature = "audio")]
            noise_enabled: noise,
        })
    }

    /// Run emulation for one time step.
    ///
    /// Call this regularly (e.g., every frame). The runner will accumulate
    /// time and run the appropriate number of CPU cycles.
    ///
    /// # Returns
    /// `TickResult` containing information about what happened during this tick.
    pub fn tick(&mut self) -> TickResult {
        let now = Instant::now();
        let mut dt = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        // Cap delta time to prevent spiral of death
        if dt > MAX_DT {
            dt = MAX_DT;
        }

        // Accumulate cycles based on elapsed time
        self.cycle_accum += dt.as_nanos() * CPU_HZ as u128;
        let real_cycles = (self.cycle_accum / 1_000_000_000) as u64;
        self.cycle_accum %= 1_000_000_000;

        let mut cycles_to_run = real_cycles;
        if self.turbo {
            cycles_to_run = cycles_to_run.saturating_mul(TURBO_MULTIPLIER);
        }

        let ran_cycles = if cycles_to_run > 0 {
            self.apple.run_cycles(cycles_to_run);
            true
        } else {
            false
        };

        // Process audio
        #[cfg(feature = "audio")]
        if ran_cycles {
            self.process_audio(real_cycles);
        }

        // Process mechanical noise
        #[cfg(feature = "audio")]
        if self.noise_enabled {
            self.process_mechanical_noise();
        }

        // Update performance statistics
        self.update_perf_stats();

        TickResult {
            ran_cycles,
            real_cycles,
            emu_mhz: self.emu_mhz,
        }
    }

    /// Get a reference to the AppleII machine.
    pub fn apple(&self) -> &AppleII {
        &self.apple
    }

    /// Get a mutable reference to the AppleII machine.
    pub fn apple_mut(&mut self) -> &mut AppleII {
        &mut self.apple
    }

    /// Check if turbo mode is enabled.
    pub fn is_turbo(&self) -> bool {
        self.turbo
    }

    /// Set turbo mode.
    pub fn set_turbo(&mut self, turbo: bool) {
        self.turbo = turbo;
    }

    /// Toggle turbo mode.
    pub fn toggle_turbo(&mut self) -> bool {
        self.turbo = !self.turbo;
        self.turbo
    }

    /// Get current emulation speed in MHz.
    pub fn emu_mhz(&self) -> f64 {
        self.emu_mhz
    }

    /// Reset the emulator.
    pub fn reset(&mut self) {
        self.apple.reset();
        self.cycle_accum = 0;
    }

    /// Flush all drives (persist any pending writes).
    pub fn flush_drives(&mut self) -> Result<(), a2vm_core::error::Error> {
        self.apple.bus.disk.flush_all_drives()
    }

    /// Process audio output.
    #[cfg(feature = "audio")]
    fn process_audio(&mut self, real_cycles: u64) {
        if let Some(ref sink) = self.audio_sink {
            self.apple.take_audio_samples_into(
                AUDIO_SAMPLE_RATE,
                real_cycles,
                &mut self.audio_buffer,
            );
            if !self.audio_buffer.is_empty() {
                sink.append(SamplesBuffer::new(
                    1,
                    AUDIO_SAMPLE_RATE,
                    std::mem::take(&mut self.audio_buffer),
                ));
            }
        }
    }

    /// Process mechanical noise events.
    #[cfg(feature = "audio")]
    fn process_mechanical_noise(&mut self) {
        if let Some(ref sink) = self.mech_sink {
            let event = self
                .mech_tracker
                .check(self.apple.bus.disk.motor_on, self.apple.bus.disk.half_track);
            if let Some(evt) = event {
                match evt {
                    MechanicalEvent::MotorStart => {
                        let cursor = Cursor::new(MOVE_ARM_WAV);
                        if let Ok(source) = Decoder::new(cursor) {
                            sink.append(source.repeat_infinite());
                        }
                    }
                    MechanicalEvent::TrackSeek => {
                        sink.stop();
                        let cursor = Cursor::new(MOVE_ARM_WAV);
                        if let Ok(source) = Decoder::new(cursor) {
                            sink.append(source.repeat_infinite());
                        }
                    }
                    MechanicalEvent::MotorStop => {
                        sink.stop();
                    }
                }
            }
        }
    }

    /// Update performance statistics.
    fn update_perf_stats(&mut self) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.perf_last_time);

        if elapsed >= PERF_SAMPLE_INTERVAL {
            let delta_cycles = self
                .apple
                .cpu
                .cycles()
                .saturating_sub(self.perf_last_cycles);
            let secs = elapsed.as_secs_f64();
            if secs > 0.0 {
                self.emu_mhz = delta_cycles as f64 / secs / 1_000_000.0;
            }
            self.perf_last_cycles = self.apple.cpu.cycles();
            self.perf_last_time = now;
        }
    }
}

impl Drop for EmulatorRunner {
    fn drop(&mut self) {
        // Flush drives on drop to ensure data persistence
        let _ = self.flush_drives();
    }
}

