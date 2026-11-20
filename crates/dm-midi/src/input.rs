//! What a MIDI input's callback does with a message: which notes count as drum hits, and when
//! each hit happened on this process's clock. Needs no MIDI device, so it is tested here.
//!
//! See "Milestone 14: MIDI in" in `docs/DESIGN.md`.

use std::time::{Duration, Instant};

use dm_audio::LiveHit;

/// The General MIDI drum keys. Keys outside them are ignored: the app has no sound for them.
pub(crate) const DRUM_KEYS: std::ops::RangeInclusive<u8> = 35..=81;

/// How far before the callback a hit may be placed. A mapped time further back than this means
/// the estimate no longer fits the input's clock, so it starts again from this message.
pub(crate) const MAX_LAG: Duration = Duration::from_millis(500);

/// A note-on that counts as a hit: velocity above 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NoteOn {
    /// The channel, 0 to 15.
    pub channel: u8,
    pub key: u8,
    /// 1 to 127.
    pub velocity: u8,
}

/// Reads a note-on with a velocity above 0. Anything else, a note-on with velocity 0 (a note-off
/// by another name) included, gives `None`.
pub(crate) fn note_on(message: &[u8]) -> Option<NoteOn> {
    let &[status, key, velocity, ..] = message else {
        return None;
    };
    if status & 0xF0 != 0x90 || key > 0x7F || velocity == 0 || velocity > 0x7F {
        return None;
    }
    Some(NoteOn {
        channel: status & 0x0F,
        key,
        velocity,
    })
}

/// Turns the input's timestamps into times on this process's clock.
///
/// The input gives each message a time in microseconds from a start of its own. The callback runs
/// some time after the message arrived, never before. So the smallest difference seen between the
/// callback's time and the timestamp is the best guess of how the two clocks line up, and a hit's
/// time is its timestamp plus that difference. Each connection starts its own estimate.
#[derive(Debug, Clone)]
pub(crate) struct Clock {
    /// The moment the times below count from.
    origin: Instant,
    /// The smallest (callback time - timestamp) seen, in microseconds since `origin`.
    offset: Option<i128>,
    /// The latest timestamp, to notice the input's clock starting again.
    last: Option<u64>,
}

impl Clock {
    pub(crate) fn new(origin: Instant) -> Self {
        Clock {
            origin,
            offset: None,
            last: None,
        }
    }

    /// Learns from a message with this timestamp whose callback ran at `now`, and returns when
    /// the message happened, on this process's clock. Never later than `now`.
    pub(crate) fn place(&mut self, timestamp: u64, now: Instant) -> Instant {
        let now_us = now.saturating_duration_since(self.origin).as_micros() as i128;
        let sample = now_us - i128::from(timestamp);
        let went_back = self.last.is_some_and(|last| timestamp < last);
        self.last = Some(timestamp);
        let offset = match self.offset {
            Some(offset) if !went_back && sample >= offset => {
                // A mapped time too far back means the clocks have drifted apart.
                if sample - offset > MAX_LAG.as_micros() as i128 {
                    sample
                } else {
                    offset
                }
            }
            _ => sample,
        };
        self.offset = Some(offset);
        let at = i128::from(timestamp) + offset;
        if at >= 0 {
            self.origin + Duration::from_micros(at as u64)
        } else {
            let before = Duration::from_micros(at.unsigned_abs() as u64);
            self.origin.checked_sub(before).unwrap_or(self.origin)
        }
    }
}

/// One input connection's notes: the channel it listens to and its clock.
#[derive(Debug, Clone)]
pub(crate) struct Notes {
    /// The channel taken by the first counted note-on, if one has come.
    channel: Option<u8>,
    clock: Clock,
}

impl Notes {
    /// For a connection that opened at `opened`.
    pub(crate) fn new(opened: Instant) -> Self {
        Notes {
            channel: None,
            clock: Clock::new(opened),
        }
    }

    /// Forgets the channel, so that the next counted note-on takes one again.
    pub(crate) fn release_channel(&mut self) {
        self.channel = None;
    }

    /// Reads a message with this timestamp whose callback ran at `now`. Returns the hit it
    /// makes, if any. Every message teaches the clock, hit or not.
    ///
    /// A note-on counts when its velocity is above 0, its channel is the one taken (or none is
    /// taken yet), and its key is a General MIDI drum. The first one to count takes its channel.
    pub(crate) fn read(
        &mut self,
        message: &[u8],
        timestamp: u64,
        now: Instant,
        sound: bool,
    ) -> Option<LiveHit> {
        let at = self.clock.place(timestamp, now);
        let note = note_on(message)?;
        if self.channel.is_some_and(|channel| channel != note.channel) {
            return None;
        }
        if !DRUM_KEYS.contains(&note.key) {
            return None;
        }
        self.channel = Some(note.channel);
        Some(LiveHit {
            at,
            key: note.key,
            velocity: note.velocity,
            sound,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn only_note_on_with_velocity_counts() {
        assert_eq!(
            note_on(&[0x99, 36, 100]),
            Some(NoteOn {
                channel: 9,
                key: 36,
                velocity: 100
            })
        );
        assert_eq!(note_on(&[0x90, 38, 1]).map(|n| n.channel), Some(0));
        assert_eq!(note_on(&[0x99, 36, 0]), None, "velocity 0 is a note-off");
        assert_eq!(note_on(&[0x89, 36, 64]), None, "note-off");
        assert_eq!(note_on(&[0xB0, 7, 100]), None, "control change");
        assert_eq!(note_on(&[0xFE]), None, "Active Sensing");
        assert_eq!(note_on(&[0x99, 36]), None, "too short");
        assert_eq!(note_on(&[]), None);
        assert_eq!(note_on(&[0x99, 0x80, 100]), None, "not a data byte");
    }

    #[test]
    fn the_first_counted_channel_is_the_only_one_heard() {
        let t = Instant::now();
        let mut notes = Notes::new(t);
        // A key outside the kit does not take the channel.
        assert_eq!(notes.read(&[0x99, 20, 100], 0, t, true), None);
        let hit = notes.read(&[0x90, 36, 90], 1000, t + ms(1), true).unwrap();
        assert_eq!((hit.key, hit.velocity, hit.sound), (36, 90, true));
        // Channel 10, where a keyboard's style drums play, is now ignored.
        assert_eq!(notes.read(&[0x99, 38, 100], 2000, t + ms(2), true), None);
        assert!(
            notes
                .read(&[0x90, 38, 100], 3000, t + ms(3), false)
                .is_some()
        );
        // Off and velocity 0 do nothing.
        assert_eq!(notes.read(&[0x80, 38, 100], 4000, t + ms(4), true), None);
        assert_eq!(notes.read(&[0x90, 38, 0], 5000, t + ms(5), true), None);

        notes.release_channel();
        assert!(
            notes
                .read(&[0x99, 42, 100], 6000, t + ms(6), true)
                .is_some()
        );
        assert_eq!(notes.read(&[0x90, 42, 100], 7000, t + ms(7), true), None);
    }

    #[test]
    fn keys_outside_the_kit_are_ignored() {
        let t = Instant::now();
        let mut notes = Notes::new(t);
        for key in [0, 34, 82, 127] {
            assert_eq!(notes.read(&[0x99, key, 100], 0, t, true), None, "{key}");
        }
        for key in [35, 81] {
            assert!(notes.read(&[0x99, key, 100], 0, t, true).is_some(), "{key}");
        }
    }

    #[test]
    fn the_clock_uses_the_least_delayed_callback() {
        let t = Instant::now();
        let mut clock = Clock::new(t);
        // The input's clock started about 5 s before ours. Nothing is known of the first
        // callback's delay, so it is taken as none.
        assert_eq!(clock.place(5_000_000, t + ms(10)), t + ms(10));
        // A callback 30 ms late is placed by its timestamp, not by when it ran.
        assert_eq!(clock.place(5_100_000, t + ms(140)), t + ms(110));
        // A callback that is less late than all before improves the estimate.
        assert_eq!(clock.place(5_200_000, t + ms(205)), t + ms(205));
        assert_eq!(clock.place(5_300_000, t + ms(320)), t + ms(305));
    }

    #[test]
    fn the_clock_never_places_a_message_after_its_callback() {
        let t = Instant::now();
        let mut clock = Clock::new(t);
        clock.place(1_000_000, t + ms(50));
        for i in 1..50u64 {
            let now = t + ms(50 + i * 7);
            let at = clock.place(1_000_000 + i * 5000, now);
            assert!(at <= now);
        }
    }

    #[test]
    fn the_clock_starts_again_when_its_estimate_stops_fitting() {
        let t = Instant::now();
        let mut clock = Clock::new(t);
        clock.place(1_000_000, t);
        // The input's clock went back, as when it restarts.
        assert_eq!(clock.place(10, t + ms(100)), t + ms(100));
        // A callback more than MAX_LAG behind the estimate means the clocks drifted apart.
        let late = MAX_LAG + ms(1);
        assert_eq!(clock.place(20_000, t + ms(120) + late), t + ms(120) + late);
        // Within MAX_LAG the estimate stands.
        assert_eq!(clock.place(30_000, t + ms(135) + late), t + ms(130) + late);
    }

    #[test]
    fn the_clock_handles_timestamps_far_beyond_ours() {
        // The input's clock has run far longer than this process, so the offset is a large
        // negative number.
        let t = Instant::now();
        let mut clock = Clock::new(t);
        assert_eq!(clock.place(u64::MAX / 4, t + ms(5)), t + ms(5));
        assert_eq!(clock.place(u64::MAX / 4 + 1000, t + ms(30)), t + ms(6));
    }
}
