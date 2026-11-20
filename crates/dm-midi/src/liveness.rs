//! Whether a keyboard behind the chosen output is still switched on, from the Active Sensing it
//! sends, apart from any MIDI device.
//!
//! Some keyboards send Active Sensing (`FE`) several times a second. When such a keyboard is
//! switched off, the messages stop at once, but the output can stay listed, and sends to it keep
//! working, for half a minute or more. After that the old connections stay dead even when the
//! keyboard comes back, so the input and the output must both be opened afresh.
//!
//! The rule: once Active Sensing has come from the input of the same name, a second without it
//! means the keyboard is gone. Then the output is dropped, and at each port check the input is
//! opened afresh, until Active Sensing arrives on it again. Only then is the output opened again.
//! A device that never sends Active Sensing is never judged this way.

use std::time::{Duration, Instant};

/// How long without Active Sensing means the keyboard is gone. The MIDI standard allows 300 ms
/// between messages; the SX720 sends about 5 a second.
pub(crate) const SENSING_TIMEOUT: Duration = Duration::from_secs(1);

/// What is known about the keyboard from its Active Sensing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presence {
    /// It has never sent Active Sensing, so nothing can be told from it.
    Unknown,
    /// It sent Active Sensing within the last [`SENSING_TIMEOUT`].
    Present,
    /// It sent Active Sensing before, but none lately on the open input, or no input is open.
    Absent,
}

/// What the sender should do with the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// Leave things as they are.
    Keep,
    /// The keyboard is gone: drop the output without silencing it, and keep the input open.
    Drop,
    /// Open the output.
    Open,
    /// The keyboard is still gone: open the input afresh and wait for Active Sensing on it.
    Listen,
}

/// The Active Sensing seen from the chosen device.
#[derive(Debug, Default, Clone)]
pub(crate) struct Watch {
    /// Whether the device has sent Active Sensing since it was chosen.
    sends_sensing: bool,
    /// The latest Active Sensing on the open input, or the end of a file transfer.
    last: Option<Instant>,
}

impl Watch {
    /// Notes Active Sensing that arrived at `at` on the open input.
    pub(crate) fn heard(&mut self, at: Instant) {
        self.sends_sensing = true;
        self.last = Some(self.last.map_or(at, |last| last.max(at)));
    }

    /// Forgets the Active Sensing of the input that just closed. A new input must show its own.
    pub(crate) fn input_closed(&mut self) {
        self.last = None;
    }

    /// Counts `now` as if Active Sensing had arrived. A keyboard may pause it during a file
    /// transfer, and the pause must not count as the keyboard going away.
    pub(crate) fn excuse(&mut self, now: Instant) {
        if self.sends_sensing {
            self.heard(now);
        }
    }

    /// What Active Sensing says about the keyboard at `now`.
    pub(crate) fn presence(&self, now: Instant) -> Presence {
        if !self.sends_sensing {
            return Presence::Unknown;
        }
        match self.last {
            Some(last) if now.saturating_duration_since(last) <= SENSING_TIMEOUT => {
                Presence::Present
            }
            _ => Presence::Absent,
        }
    }

    /// What to do with the output at `now`. `open` says whether it is open; `check` whether this
    /// is a port check, the only time the output or the input may be opened again.
    pub(crate) fn action(&self, open: bool, check: bool, now: Instant) -> Action {
        match (open, self.presence(now)) {
            (true, Presence::Absent) => Action::Drop,
            (true, _) => Action::Keep,
            (false, _) if !check => Action::Keep,
            (false, Presence::Absent) => Action::Listen,
            (false, _) => Action::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_device_without_active_sensing_keeps_the_old_behaviour() {
        let t = Instant::now();
        let watch = Watch::default();
        assert_eq!(watch.presence(t + ms(5000)), Presence::Unknown);
        assert_eq!(watch.action(true, true, t + ms(5000)), Action::Keep);
        assert_eq!(watch.action(false, true, t), Action::Open);
        assert_eq!(watch.action(false, false, t), Action::Keep);
    }

    #[test]
    fn a_second_without_active_sensing_drops_the_output() {
        let t = Instant::now();
        let mut watch = Watch::default();
        watch.heard(t);
        watch.heard(t + ms(200));
        assert_eq!(watch.action(true, false, t + ms(1200)), Action::Keep);
        assert_eq!(watch.action(true, false, t + ms(1201)), Action::Drop);
        assert_eq!(watch.presence(t + ms(1201)), Presence::Absent);
        // An older message arriving late does not move the time back.
        watch.heard(t + ms(100));
        assert_eq!(watch.action(true, false, t + ms(1201)), Action::Drop);
    }

    #[test]
    fn while_gone_each_check_listens_afresh_until_active_sensing_returns() {
        let t = Instant::now();
        let mut watch = Watch::default();
        watch.heard(t);
        // Between checks nothing is opened.
        assert_eq!(watch.action(false, false, t + ms(2000)), Action::Keep);
        // At a check, with the keyboard still silent, the input is opened afresh, which
        // forgets what the old one heard.
        assert_eq!(watch.action(false, true, t + ms(2000)), Action::Listen);
        watch.input_closed();
        assert_eq!(watch.action(false, true, t + ms(3000)), Action::Listen);
        watch.input_closed();
        // The new input hears Active Sensing: the next check opens the output.
        watch.heard(t + ms(3200));
        assert_eq!(watch.action(false, true, t + ms(4000)), Action::Open);
        assert_eq!(watch.action(true, false, t + ms(4000)), Action::Keep);
    }

    #[test]
    fn a_closed_input_counts_as_absent_once_active_sensing_was_seen() {
        let t = Instant::now();
        let mut watch = Watch::default();
        watch.heard(t);
        watch.input_closed();
        assert_eq!(watch.presence(t), Presence::Absent);
        assert_eq!(watch.action(false, true, t), Action::Listen);
    }

    #[test]
    fn a_transfer_excuses_a_pause() {
        let t = Instant::now();
        let mut watch = Watch::default();
        watch.excuse(t + ms(5000));
        assert_eq!(watch.presence(t + ms(5000)), Presence::Unknown);
        watch.heard(t);
        watch.excuse(t + ms(5000));
        assert_eq!(watch.action(true, false, t + ms(5500)), Action::Keep);
        assert_eq!(watch.action(true, false, t + ms(6500)), Action::Drop);
    }
}
