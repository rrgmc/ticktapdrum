//! The sender's bookkeeping, apart from any MIDI device: the list of messages waiting for their
//! moment, which of them to send or drop, and which notes are sounding.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use dm_audio::MidiEvent;

/// How late a message may be and still go out. A later one is dropped, so that a stall (or a
/// connection that opens while messages wait) does not burst out a run of stale notes.
pub(crate) const LATE_LIMIT: Duration = Duration::from_millis(50);

/// How many messages may wait at once. The engine's queue holds as many, so the list only fills
/// if the sender stops sending.
pub(crate) const MAX_PENDING: usize = dm_audio::MIDI_QUEUE_CAPACITY;

/// Controller 123, "All notes off".
const ALL_NOTES_OFF: u8 = 123;

/// The drum channel, channel 10, counted from zero.
const DRUM_CHANNEL: u8 = 9;

/// The messages waiting to be sent, in the order they fall due. Messages due at the same moment
/// keep the order they arrived in, so a note-off and a note-on for the same key stay in sequence.
#[derive(Debug, Default)]
pub(crate) struct Pending {
    events: VecDeque<MidiEvent>,
}

impl Pending {
    /// Adds a message in its place by due time. Returns false, and drops the message, when the
    /// list is full.
    pub(crate) fn push(&mut self, event: MidiEvent) -> bool {
        if self.events.len() >= MAX_PENDING {
            return false;
        }
        let at = self.events.partition_point(|e| e.at <= event.at);
        self.events.insert(at, event);
        true
    }

    /// When the first message falls due, if any waits.
    pub(crate) fn next_due(&self) -> Option<Instant> {
        self.events.front().map(|e| e.at)
    }

    /// Takes the first message if it is due at `now` or earlier.
    pub(crate) fn pop_due(&mut self, now: Instant) -> Option<MidiEvent> {
        if self.events.front()?.at <= now {
            self.events.pop_front()
        } else {
            None
        }
    }

    /// Forgets every waiting message.
    pub(crate) fn clear(&mut self) {
        self.events.clear();
    }

    /// How many messages wait.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }
}

/// Whether a due message goes out or is dropped as stale. A message later than [`LATE_LIMIT`] is
/// dropped, except a note-off for a sounding note: dropping that would leave the note hanging.
pub(crate) fn should_send(event: &MidiEvent, now: Instant, sounding: &Sounding) -> bool {
    let late = now.saturating_duration_since(event.at);
    late <= LATE_LIMIT || sounding.ends_a_sounding_note(event.message())
}

/// A note message, decoded: its channel (0..=15), key, and whether it starts the note.
fn note(message: &[u8]) -> Option<(u8, u8, bool)> {
    let [status, key, velocity] = *message else {
        return None;
    };
    let channel = status & 0x0F;
    match status & 0xF0 {
        0x90 if velocity > 0 => Some((channel, key & 0x7F, true)),
        // A note-on with velocity zero means note-off.
        0x90 | 0x80 => Some((channel, key & 0x7F, false)),
        _ => None,
    }
}

/// The notes that are sounding on the output: those sent a note-on and no note-off since.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Sounding {
    /// One bit per key, one word per channel.
    keys: [u128; 16],
}

impl Sounding {
    /// Notes a message that went out.
    pub(crate) fn track(&mut self, message: &[u8]) {
        if let Some((channel, key, on)) = note(message) {
            let bit = 1u128 << key;
            let keys = &mut self.keys[usize::from(channel)];
            if on {
                *keys |= bit;
            } else {
                *keys &= !bit;
            }
        }
    }

    /// Whether the message is a note-off for a note that is sounding.
    pub(crate) fn ends_a_sounding_note(&self, message: &[u8]) -> bool {
        match note(message) {
            Some((channel, key, false)) => self.keys[usize::from(channel)] & (1u128 << key) != 0,
            _ => false,
        }
    }

    /// Whether no note is sounding.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_empty(&self) -> bool {
        self.keys.iter().all(|&k| k == 0)
    }

    /// The messages that silence the output, and forgets every note: a note-off for each sounding
    /// note, then "All notes off" on the drum channel and on any other channel that had a note.
    pub(crate) fn silence(&mut self) -> Vec<[u8; 3]> {
        let mut messages = Vec::new();
        for (channel, keys) in (0u8..).zip(self.keys.iter()) {
            for key in 0..128u8 {
                if keys & (1u128 << key) != 0 {
                    messages.push([0x80 | channel, key, 0]);
                }
            }
        }
        for (channel, &keys) in (0u8..).zip(self.keys.iter()) {
            if keys != 0 || channel == DRUM_CHANNEL {
                messages.push([0xB0 | channel, ALL_NOTES_OFF, 0]);
            }
        }
        self.keys = [0; 16];
        messages
    }
}

/// The output list as the interface shows it: sorted, each name once, with the virtual port
/// added where the platform offers one.
pub(crate) fn output_list(mut names: Vec<String>, virtual_port: Option<&str>) -> Vec<String> {
    if let Some(name) = virtual_port {
        names.push(name.to_owned());
    }
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(at: Instant, bytes: [u8; 3]) -> MidiEvent {
        MidiEvent { at, bytes, len: 3 }
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn pending_orders_by_time_and_keeps_arrival_order_for_ties() {
        let t = Instant::now();
        let mut pending = Pending::default();
        pending.push(event(t + ms(20), [0x99, 36, 100]));
        pending.push(event(t + ms(10), [0x99, 38, 100]));
        pending.push(event(t + ms(20), [0x89, 36, 0]));
        pending.push(event(t + ms(5), [0x99, 42, 100]));
        assert_eq!(pending.next_due(), Some(t + ms(5)));

        let order: Vec<_> = std::iter::from_fn(|| pending.pop_due(t + ms(100)))
            .map(|e| (e.at, e.bytes))
            .collect();
        assert_eq!(
            order,
            vec![
                (t + ms(5), [0x99, 42, 100]),
                (t + ms(10), [0x99, 38, 100]),
                (t + ms(20), [0x99, 36, 100]),
                (t + ms(20), [0x89, 36, 0]),
            ]
        );
        assert_eq!(pending.next_due(), None);
    }

    #[test]
    fn pop_due_takes_only_what_is_due() {
        let t = Instant::now();
        let mut pending = Pending::default();
        pending.push(event(t + ms(10), [0x99, 36, 100]));
        pending.push(event(t + ms(30), [0x89, 36, 0]));
        assert!(pending.pop_due(t).is_none());
        assert!(pending.pop_due(t + ms(10)).is_some());
        assert!(pending.pop_due(t + ms(29)).is_none());
        assert_eq!(pending.len(), 1);
        pending.clear();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn pending_refuses_messages_when_full() {
        let t = Instant::now();
        let mut pending = Pending::default();
        for _ in 0..MAX_PENDING {
            assert!(pending.push(event(t, [0x99, 36, 100])));
        }
        assert!(!pending.push(event(t, [0x99, 36, 100])));
        assert_eq!(pending.len(), MAX_PENDING);
    }

    #[test]
    fn late_messages_are_dropped_except_note_offs_for_sounding_notes() {
        let t = Instant::now();
        let mut sounding = Sounding::default();
        let on = event(t, [0x99, 36, 100]);
        let off = event(t, [0x89, 36, 0]);

        // On time, slightly late and exactly at the limit: sent.
        assert!(should_send(&on, t, &sounding));
        assert!(should_send(&on, t + ms(10), &sounding));
        assert!(should_send(&on, t + LATE_LIMIT, &sounding));
        // Early cannot happen in the loop, but must not count as late.
        assert!(should_send(
            &event(t + ms(5), [0x99, 36, 100]),
            t,
            &sounding
        ));
        // Too late: dropped.
        assert!(!should_send(&on, t + ms(51), &sounding));
        assert!(!should_send(&off, t + ms(200), &sounding));

        // A late note-off still goes out while its note sounds.
        sounding.track(on.message());
        assert!(should_send(&off, t + ms(200), &sounding));
        // But a late note-on for a sounding key does not.
        assert!(!should_send(&on, t + ms(200), &sounding));
    }

    #[test]
    fn sounding_follows_note_on_and_note_off() {
        let mut sounding = Sounding::default();
        assert!(sounding.is_empty());
        sounding.track(&[0x99, 36, 100]);
        sounding.track(&[0x99, 38, 90]);
        assert!(sounding.ends_a_sounding_note(&[0x89, 36, 0]));
        assert!(!sounding.ends_a_sounding_note(&[0x89, 40, 0]));
        // A note-off on another channel is a different note.
        assert!(!sounding.ends_a_sounding_note(&[0x80, 36, 0]));
        // Velocity zero counts as note-off.
        assert!(sounding.ends_a_sounding_note(&[0x99, 36, 0]));

        sounding.track(&[0x89, 36, 0]);
        sounding.track(&[0x99, 38, 0]);
        assert!(sounding.is_empty());

        // Program changes and controllers change nothing.
        sounding.track(&[0xC9, 0, 0]);
        sounding.track(&[0xB9, 7, 100]);
        sounding.track(&[0xC9, 25]);
        assert!(sounding.is_empty());
    }

    #[test]
    fn silence_ends_each_sounding_note_then_sends_all_notes_off() {
        let mut sounding = Sounding::default();
        assert_eq!(sounding.silence(), vec![[0xB9, 123, 0]]);

        sounding.track(&[0x99, 49, 100]);
        sounding.track(&[0x99, 36, 100]);
        sounding.track(&[0x99, 127, 100]);
        sounding.track(&[0x90, 60, 100]);
        assert_eq!(
            sounding.silence(),
            vec![
                [0x80, 60, 0],
                [0x89, 36, 0],
                [0x89, 49, 0],
                [0x89, 127, 0],
                [0xB0, 123, 0],
                [0xB9, 123, 0],
            ]
        );
        assert!(sounding.is_empty());
    }

    #[test]
    fn output_list_is_sorted_without_repeats() {
        let names = vec!["b".to_owned(), "a".to_owned(), "b".to_owned()];
        assert_eq!(output_list(names.clone(), None), vec!["a", "b"]);
        assert_eq!(
            output_list(names, Some("TickTapDrum")),
            vec!["TickTapDrum", "a", "b"]
        );
        assert_eq!(
            output_list(vec!["TickTapDrum".to_owned()], Some("TickTapDrum")),
            vec!["TickTapDrum"]
        );
    }
}
