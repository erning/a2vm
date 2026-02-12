//! Disk II mechanical noise simulation.
//!
//! Provides embedded audio assets and state tracking for realistic
//! mechanical disk drive sounds.

/// Embedded WAV audio for disk arm movement sound.
pub const MOVE_ARM_WAV: &[u8] = include_bytes!("../../assets/move_arm.wav");

/// Mechanical noise events for Disk II.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MechanicalEvent {
    MotorStart,
    TrackSeek,
    MotorStop,
}

/// Tracks Disk II motor and track state to generate mechanical noise events.
pub struct DiskMechTracker {
    motor_was_on: bool,
    last_half_track: u8,
}

impl DiskMechTracker {
    pub fn new() -> Self {
        Self {
            motor_was_on: false,
            last_half_track: 0,
        }
    }

    pub fn check(&mut self, motor_on: bool, half_track: u8) -> Option<MechanicalEvent> {
        if !self.motor_was_on && motor_on {
            self.motor_was_on = true;
            self.last_half_track = half_track;
            return Some(MechanicalEvent::MotorStart);
        }

        if self.motor_was_on && !motor_on {
            self.motor_was_on = false;
            self.last_half_track = half_track;
            return Some(MechanicalEvent::MotorStop);
        }

        if motor_on && half_track != self.last_half_track {
            self.last_half_track = half_track;
            return Some(MechanicalEvent::TrackSeek);
        }

        self.motor_was_on = motor_on;
        self.last_half_track = half_track;
        None
    }

    pub fn reset(&mut self) {
        self.motor_was_on = false;
        self.last_half_track = 0;
    }
}

impl Default for DiskMechTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_motor_on() {
        let mut tracker = DiskMechTracker::new();

        let event = tracker.check(true, 0);
        assert_eq!(event, Some(MechanicalEvent::MotorStart));

        let event = tracker.check(true, 0);
        assert_eq!(event, None);
    }

    #[test]
    fn test_motor_off() {
        let mut tracker = DiskMechTracker::new();

        tracker.check(true, 0);

        let event = tracker.check(false, 0);
        assert_eq!(event, Some(MechanicalEvent::MotorStop));
    }

    #[test]
    fn test_track_seek() {
        let mut tracker = DiskMechTracker::new();

        tracker.check(true, 0);

        let event = tracker.check(true, 2);
        assert_eq!(event, Some(MechanicalEvent::TrackSeek));

        let event = tracker.check(true, 4);
        assert_eq!(event, Some(MechanicalEvent::TrackSeek));
    }

    #[test]
    fn test_no_event_when_motor_off() {
        let mut tracker = DiskMechTracker::new();

        let event = tracker.check(false, 2);
        assert_eq!(event, None);

        let event = tracker.check(false, 4);
        assert_eq!(event, None);
    }

    #[test]
    fn test_reset() {
        let mut tracker = DiskMechTracker::new();

        tracker.check(true, 5);

        tracker.reset();

        let event = tracker.check(true, 0);
        assert_eq!(event, Some(MechanicalEvent::MotorStart));
    }
}
