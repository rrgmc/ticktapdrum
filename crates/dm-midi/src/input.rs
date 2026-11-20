//! What a MIDI input's callback does with a message: which notes count as drum hits, and when
//! each hit happened on this process's clock. Needs no MIDI device, so it is tested here.
//!
//! See "Milestone 14: MIDI in" in `docs/DESIGN.md`.

use std::time::{Duration, Instant};

use dm_audio::LiveHit;
use dm_groove::KeyboardKit;

/// The General MIDI drum keys. Keys outside them are ignored: the app has no sound for them.
pub(crate) const DRUM_KEYS: std::ops::RangeInclusive<u8> = 35..=81;

/// Channel 10, counted from 0: General MIDI's drum channel.
const DRUM_CHANNEL: u8 = 9;

/// The programs of General MIDI's drum kits on channel 10, counted from 0: Standard, Room,
/// Power, Electronic, TR-808, Jazz, Brush, Orchestra and SFX. Other programs there are not
/// standard, so they say nothing about the kit.
const GENERAL_MIDI_KITS: [u8; 9] = [0, 8, 16, 24, 25, 32, 40, 48, 56];

/// Yamaha's manufacturer ID, the second byte of its SysEx messages.
const YAMAHA_ID: u8 = 0x43;

/// A drum kit the keyboard switched to, heard in a program change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KitChange {
    /// The channel of the program change, 0 to 15.
    pub channel: u8,
    /// The kit's name, for a Yamaha kit the key map knows. `None` for a General MIDI kit.
    pub name: Option<&'static str>,
    /// The General MIDI program of the nearest app kit: the table's choice for a Yamaha kit,
    /// the program itself for a General MIDI kit. `None` for Yamaha's percussion and effect
    /// kits.
    pub similar: Option<u8>,
}

/// The voice each channel of one input connection plays, as far as its messages tell.
///
/// A keyboard says nothing about the voice already chosen, so this knows a channel's voice only
/// from its next program change on. A Yamaha keyboard sends XG SysEx when a voice changes, so a
/// connection that has sent any Yamaha SysEx is taken to be a Yamaha keyboard, and its kits are
/// read from the key map.
#[derive(Debug, Clone, Default)]
pub(crate) struct Voices {
    /// The last bank select MSB (controller 0) on each channel.
    msb: [u8; 16],
    /// The Yamaha kit the last program change on each channel picked, if the key map knows it.
    kits: [Option<KeyboardKit>; 16],
    /// Whether the connection has sent a Yamaha SysEx message.
    yamaha: bool,
}

impl Voices {
    /// Learns from one message. Returns the kit a program change picks, if it is one the app
    /// can name or match: a Yamaha kit in the key map from a Yamaha keyboard, or a standard
    /// General MIDI kit on channel 10 from any other device.
    pub(crate) fn follow(&mut self, message: &[u8]) -> Option<KitChange> {
        let &status = message.first()?;
        if status == 0xF0 {
            if message.get(1) == Some(&YAMAHA_ID) {
                self.yamaha = true;
            }
            return None;
        }
        let channel = status & 0x0F;
        let slot = usize::from(channel);
        match (status & 0xF0, message.get(1..)) {
            (0xB0, Some(&[0, value, ..])) if value <= 0x7F => {
                self.msb[slot] = value;
                None
            }
            (0xC0, Some(&[program, ..])) if program <= 0x7F => {
                let kit = dm_groove::yamaha_kit(self.msb[slot], program);
                self.kits[slot] = kit;
                if self.yamaha {
                    kit.map(|kit| KitChange {
                        channel,
                        name: Some(kit.name()),
                        similar: kit.similar(),
                    })
                } else {
                    (channel == DRUM_CHANNEL && GENERAL_MIDI_KITS.contains(&program)).then_some(
                        KitChange {
                            channel,
                            name: None,
                            similar: Some(program),
                        },
                    )
                }
            }
            _ => None,
        }
    }

    /// The Yamaha kit a channel plays, if the connection is a Yamaha keyboard and the key map
    /// knows the kit.
    pub(crate) fn kit(&self, channel: u8) -> Option<KeyboardKit> {
        if !self.yamaha {
            return None;
        }
        self.kits.get(usize::from(channel)).copied().flatten()
    }
}

/// The drum a key below the General MIDI kit plays as, if it has a near one.
///
/// Yamaha's XG kits put extra sounds on keys 13 to 34, which General MIDI leaves empty, and an
/// arranger keyboard's lowest keys reach them. The app's bank has other sounds there, so the
/// keys with a near drum are moved onto it, and the rest (clicks, brushes, castanets) are left
/// out. Checked on a PSR-SX720, whose lowest key sends 24; the sound names are XG's.
pub(crate) fn near_drum(key: u8) -> Option<u8> {
    match key {
        29 => Some(38), // snare roll: acoustic snare
        31 => Some(38), // snare, soft: acoustic snare
        32 => Some(37), // sticks: side stick
        33 => Some(36), // bass drum, soft: kick
        34 => Some(37), // open rim shot: side stick
        _ if DRUM_KEYS.contains(&key) => Some(key),
        _ => None,
    }
}

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

/// One input connection's notes: the channel it listens to, the voice of each channel and its
/// clock. A connection that opens again starts with a new one, so nothing carries over.
#[derive(Debug, Clone)]
pub(crate) struct Notes {
    /// The channel taken by the first counted note-on, if one has come.
    channel: Option<u8>,
    voices: Voices,
    clock: Clock,
}

impl Notes {
    /// For a connection that opened at `opened`.
    pub(crate) fn new(opened: Instant) -> Self {
        Notes {
            channel: None,
            voices: Voices::default(),
            clock: Clock::new(opened),
        }
    }

    /// Forgets the channel, so that the next counted note-on takes one again. The voices stay.
    pub(crate) fn release_channel(&mut self) {
        self.channel = None;
    }

    /// Learns the voices from a message, whether or not its notes count. Returns the kit a
    /// program change picks (see [`Voices::follow`]) when it is on the channel taken, or on any
    /// channel while none is taken.
    pub(crate) fn follow(&mut self, message: &[u8]) -> Option<KitChange> {
        let change = self.voices.follow(message)?;
        self.channel
            .is_none_or(|channel| channel == change.channel)
            .then_some(change)
    }

    /// Reads a message with this timestamp whose callback ran at `now`. Returns the hit it
    /// makes, if any. Every message teaches the clock, hit or not.
    ///
    /// A note-on counts when its velocity is above 0, its channel is the one taken (or none is
    /// taken yet), and its key is a drum. On a Yamaha keyboard whose channel plays a kit the key
    /// map knows, the kit's map says which drum a key is, or that it is none. Otherwise the key
    /// is a General MIDI drum or one with a near drum (see [`near_drum`]). The first note-on to
    /// count takes its channel.
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
        let key = match self.voices.kit(note.channel) {
            Some(kit) => kit.key(note.key)?,
            None => near_drum(note.key)?,
        };
        self.channel = Some(note.channel);
        Some(LiveHit {
            at,
            key,
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
        for key in [0, 24, 28, 30, 82, 127] {
            assert_eq!(notes.read(&[0x99, key, 100], 0, t, true), None, "{key}");
        }
        for key in [35, 81] {
            assert!(notes.read(&[0x99, key, 100], 0, t, true).is_some(), "{key}");
        }
    }

    #[test]
    fn low_yamaha_keys_play_as_a_near_drum() {
        let t = Instant::now();
        let mut notes = Notes::new(t);
        for (key, drum) in [(29, 38), (31, 38), (32, 37), (33, 36), (34, 37)] {
            let hit = notes.read(&[0x92, key, 70], 0, t, true).unwrap();
            assert_eq!(hit.key, drum, "{key}");
        }
        // A mapped key takes the channel like any drum.
        let mut notes = Notes::new(t);
        assert!(notes.read(&[0x92, 33, 70], 0, t, true).is_some());
        assert_eq!(notes.read(&[0x99, 36, 70], 0, t, true), None);
    }

    /// What the PSR-SX720 sent when its right-hand part, on channel 3, switched to ElectroKit.
    const SX720_ELECTRO_KIT: [&[u8]; 4] = [
        &[0xF0, 0x43, 0x10, 0x4C, 0x08, 0x02, 0x07, 0x01, 0xF7],
        &[0xB2, 0x00, 0x7F],
        &[0xB2, 0x20, 0x00],
        &[0xC2, 0x46],
    ];

    /// Follows each message and returns the last kit change.
    fn follow_all(notes: &mut Notes, messages: &[&[u8]]) -> Option<KitChange> {
        messages
            .iter()
            .fold(None, |last, m| notes.follow(m).or(last))
    }

    fn key_of(notes: &mut Notes, message: &[u8]) -> Option<u8> {
        notes
            .read(message, 0, Instant::now(), true)
            .map(|hit| hit.key)
    }

    #[test]
    fn a_yamaha_kit_change_maps_the_keys_through_the_kit() {
        let mut notes = Notes::new(Instant::now());
        let change = follow_all(&mut notes, &SX720_ELECTRO_KIT);
        assert_eq!(
            change,
            Some(KitChange {
                channel: 2,
                name: Some("ElectroKit"),
                similar: Some(24),
            })
        );
        // The table puts key 24 on the kick and key 27 on the clap.
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), Some(36));
        assert_eq!(key_of(&mut notes, &[0x92, 27, 100]), Some(39));
        // Key 35 keeps its General MIDI place.
        assert_eq!(key_of(&mut notes, &[0x92, 35, 100]), Some(35));
    }

    #[test]
    fn without_yamaha_sysex_the_table_is_not_used() {
        let mut notes = Notes::new(Instant::now());
        assert_eq!(follow_all(&mut notes, &SX720_ELECTRO_KIT[1..]), None);
        // The default rule: 24 is outside the kit, 27 too, 33 is the soft kick.
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), None);
        assert_eq!(key_of(&mut notes, &[0x92, 27, 100]), None);
        assert_eq!(key_of(&mut notes, &[0x92, 33, 100]), Some(36));
    }

    #[test]
    fn a_key_the_kit_leaves_out_is_ignored_and_takes_no_channel() {
        let mut notes = Notes::new(Instant::now());
        follow_all(&mut notes, &SX720_ELECTRO_KIT);
        // ElectroKit leaves key 74 out, though General MIDI has a drum there.
        assert_eq!(key_of(&mut notes, &[0x92, 74, 100]), None);
        // So channel 1 can still take the lock with the default rule.
        assert_eq!(key_of(&mut notes, &[0x90, 74, 100]), Some(74));
        assert_eq!(key_of(&mut notes, &[0x92, 36, 100]), None);
    }

    #[test]
    fn an_unknown_yamaha_voice_uses_the_default_rule() {
        let mut notes = Notes::new(Instant::now());
        follow_all(&mut notes, &SX720_ELECTRO_KIT);
        // Bank 0, program 0: a piano. No report, and the kit's map no longer applies.
        assert_eq!(
            follow_all(&mut notes, &[&[0xB2, 0x00, 0x00], &[0xC2, 0x00]]),
            None
        );
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), None);
        assert_eq!(key_of(&mut notes, &[0x92, 74, 100]), Some(74));
    }

    #[test]
    fn reopening_forgets_the_voices() {
        let t = Instant::now();
        let mut notes = Notes::new(t);
        follow_all(&mut notes, &SX720_ELECTRO_KIT);
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), Some(36));
        // A connection that opens again starts afresh, SysEx included: the next program change
        // does not use the table until Yamaha SysEx comes again.
        let mut notes = Notes::new(t);
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), None);
        assert_eq!(follow_all(&mut notes, &SX720_ELECTRO_KIT[1..]), None);
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), None);
        // Releasing the channel keeps them.
        let mut notes = Notes::new(t);
        follow_all(&mut notes, &SX720_ELECTRO_KIT);
        notes.release_channel();
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), Some(36));
    }

    #[test]
    fn only_a_program_change_reports_a_kit() {
        let mut notes = Notes::new(Instant::now());
        for message in &SX720_ELECTRO_KIT[..3] {
            assert_eq!(notes.follow(message), None);
        }
        assert!(notes.follow(SX720_ELECTRO_KIT[3]).is_some());
        // Volume, reverb, more SysEx and notes report nothing.
        for message in [
            &[0xB2, 0x07, 0x64][..],
            &[0xB2, 0x5B, 0x28],
            &[0xF0, 0x43, 0x10, 0x4C, 0x08, 0x02, 0x11, 0x7F, 0xF7],
            &[0x92, 36, 100],
        ] {
            assert_eq!(notes.follow(message), None, "{message:?}");
        }
        // The same kit chosen again is reported again.
        assert!(notes.follow(&[0xC2, 0x46]).is_some());
    }

    #[test]
    fn a_kit_is_reported_only_for_the_channel_taken() {
        let mut notes = Notes::new(Instant::now());
        notes.follow(SX720_ELECTRO_KIT[0]);
        assert_eq!(key_of(&mut notes, &[0x90, 36, 100]), Some(36));
        // Channel 3 is not the one taken: its kit is kept but not reported.
        assert_eq!(follow_all(&mut notes, &SX720_ELECTRO_KIT[1..]), None);
        assert_eq!(
            follow_all(&mut notes, &[&[0xB0, 0, 127], &[0xC0, 70]]).map(|c| c.channel),
            Some(0)
        );
        notes.release_channel();
        assert_eq!(key_of(&mut notes, &[0x92, 24, 100]), Some(36));
    }

    #[test]
    fn a_general_midi_kit_is_reported_from_channel_ten() {
        let mut notes = Notes::new(Instant::now());
        assert_eq!(
            notes.follow(&[0xC9, 25]),
            Some(KitChange {
                channel: 9,
                name: None,
                similar: Some(25),
            })
        );
        // Not a standard kit, or not channel 10.
        assert_eq!(notes.follow(&[0xC9, 26]), None);
        assert_eq!(notes.follow(&[0xC0, 25]), None);
        // A Yamaha keyboard's kits come from the table alone.
        notes.follow(SX720_ELECTRO_KIT[0]);
        assert_eq!(notes.follow(&[0xC9, 25]), None);
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
