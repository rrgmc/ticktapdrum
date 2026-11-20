//! The MIDI output setting: which output the drums go to, kept by name in the store, and what
//! the interface does when that output connects, disappears or will not open. The drum hits
//! for MIDI in come from the input of the same device, or from another input chosen by name,
//! such as drum pads (see "Milestone 14: MIDI in").
//!
//! Setting it up is one choice and no questions (see "Milestone 9: MIDI out" in
//! `docs/DESIGN.md`). While the chosen output is connected, the engine's MIDI out is on, which
//! silences the internal synthesizer. Otherwise it is off and the drums play on this device.

use dm_audio::Engine;
#[cfg(feature = "midi")]
pub(crate) use dm_midi::{
    InputChoice, KeyboardTarget, MidiOut, MidiStatus, TransferError, TransferStatus,
};
#[cfg(not(feature = "midi"))]
pub(crate) use off::{
    InputChoice, KeyboardTarget, MidiOut, MidiStatus, TransferError, TransferStatus,
};

/// Stands in for `dm-midi` in a build without the `midi` feature: it reports no MIDI, so the
/// setting is not offered and the engine's MIDI out stays off.
#[cfg(not(feature = "midi"))]
mod off {
    #[allow(dead_code)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum MidiStatus {
        Off,
        Connected,
        Missing,
        Failed(String),
    }

    pub(crate) struct MidiOut;

    impl MidiOut {
        pub(crate) const fn supported() -> bool {
            false
        }

        pub(crate) fn start(_events: dm_audio::MidiEvents) -> Self {
            MidiOut
        }

        pub(crate) fn outputs(&self) -> Vec<String> {
            Vec::new()
        }

        pub(crate) fn select(&mut self, _name: Option<String>) {}

        pub(crate) fn status(&self) -> MidiStatus {
            MidiStatus::Off
        }

        pub(crate) fn listen(&mut self, _hits: dm_audio::LiveHits) {}

        pub(crate) fn set_listening(&mut self, _on: bool) {}

        pub(crate) fn inputs(&self) -> Vec<String> {
            Vec::new()
        }

        pub(crate) fn select_input(&mut self, _choice: InputChoice) {}

        pub(crate) fn input_status(&self) -> MidiStatus {
            MidiStatus::Off
        }

        pub(crate) fn send_style(
            &mut self,
            _target: KeyboardTarget,
            _file_name: String,
            _data: Vec<u8>,
        ) {
        }

        pub(crate) fn transfer(&self) -> TransferStatus {
            TransferStatus::Idle
        }
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) enum InputChoice {
        #[default]
        SameAsOutput,
        Named(String),
        None,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum KeyboardTarget {
        Yamaha,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) enum TransferStatus {
        Idle,
        Running(f32),
        Done(String),
        Failed(TransferError),
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum TransferError {
        NotConnected,
        NoInput,
        NoAnswer,
        UnknownKeyboard(String),
        Refused(String),
    }
}

/// How often the interface looks at the status while an output is chosen, in seconds. The
/// interface may otherwise draw only on input, and a device plugged in or pulled out must be
/// noticed without a tap.
const POLL: f64 = 0.5;

/// The longest output name the menu item shows, in characters, before it is shortened.
pub(crate) const SHORT_NAME: usize = 12;

/// A one-line message about the output, in no language yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Note {
    /// The output connected: the drums go to it now.
    Sending(String),
    /// The output was connected and is not any more. The drums play on this device again.
    Gone(String),
    /// The output is there but another program holds it.
    InUse(String),
    /// The input went away while recording. Recording stopped and kept its notes.
    InputGone(String),
}

impl Note {
    /// The message in the interface's language.
    pub(crate) fn text(&self) -> String {
        match self {
            Note::Sending(name) => tr!("menu-midi-sending", device = name.as_str()),
            Note::Gone(name) => tr!("menu-midi-gone", device = name.as_str()),
            Note::InUse(name) => tr!("menu-midi-in-use", device = name.as_str()),
            Note::InputGone(name) => tr!("menu-midi-input-gone", device = name.as_str()),
        }
    }
}

/// What a change of status means for the person, if anything. `last` is the status the
/// interface last saw, `now` the current one, and `by_hand` says the person chose the output in
/// this session and it has not failed or gone missing since. Reconnecting at start-up is not by
/// hand, so an output that will not open then says nothing.
pub(crate) fn note(
    last: &MidiStatus,
    now: &MidiStatus,
    name: Option<&str>,
    by_hand: bool,
) -> Option<Note> {
    let name = name?.to_owned();
    let was_connected = *last == MidiStatus::Connected;
    match now {
        MidiStatus::Connected if !was_connected => Some(Note::Sending(name)),
        MidiStatus::Failed(_) if by_hand && !matches!(last, MidiStatus::Failed(_)) => {
            Some(Note::InUse(name))
        }
        MidiStatus::Missing | MidiStatus::Failed(_) if was_connected => Some(Note::Gone(name)),
        _ => None,
    }
}

/// The MIDI channel volume, 0..=127, for a volume in percent: 100% is 127, the controller's top.
fn channel_volume(percent: u8) -> u8 {
    ((u16::from(percent.min(100)) * 127 + 50) / 100) as u8
}

/// Which input to open: the chosen output's own device, or the input of this name. It is open
/// whether or not the MIDI in button is on, so the button can show while it is there.
fn input_choice(device: Option<&str>) -> InputChoice {
    match device {
        Some(name) => InputChoice::Named(name.to_owned()),
        None => InputChoice::SameAsOutput,
    }
}

/// `name` cut to at most `max` characters, the last of them an ellipsis, for a narrow label.
pub(crate) fn shorten(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        return name.to_owned();
    }
    let kept: String = name.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// The MIDI sender and the choice it follows.
#[derive(Default)]
pub(crate) struct Midi {
    /// The sender, once an engine has handed out its MIDI queue.
    out: Option<MidiOut>,
    /// The chosen output's name, or `None`. A device setting, kept in the store.
    choice: Option<String>,
    /// The status seen last frame.
    last: Option<MidiStatus>,
    /// Whether the engine was last told to send MIDI.
    sending: bool,
    /// The person chose the output in this session, and it has not failed or gone missing
    /// since.
    by_hand: bool,
    /// The outputs listed when the dialog last opened.
    outputs: Vec<String>,
    /// The MIDI output volume in percent, 0..=100. A device setting, kept in the store.
    volume: u8,
    /// Whether the chosen input was open when last looked at, whether or not it is on.
    input_open: bool,
    /// Whether the input's hits are listened to: the MIDI in button. Off by default. A device
    /// setting, kept in the store.
    input: bool,
    /// The chosen input's name, or `None` for the output's own device. A device setting, kept
    /// in the store.
    input_device: Option<String>,
    /// The inputs listed when the input dialog last opened.
    inputs: Vec<String>,
}

impl Midi {
    /// Follows the choices, the volume and the input setting kept in the store. Nothing is sent
    /// until an engine exists.
    pub(crate) fn new(
        choice: Option<String>,
        volume: u8,
        input: bool,
        input_device: Option<String>,
    ) -> Self {
        Self {
            choice,
            volume: volume.min(100),
            input,
            input_device,
            ..Self::default()
        }
    }

    /// Whether the input is on and open now, so drum hits arrive and playing a pattern records
    /// them.
    pub(crate) fn input_connected(&self) -> bool {
        self.input && self.input_open
    }

    /// Whether the chosen input is open now, on or off. The MIDI in button shows while it is.
    pub(crate) fn input_open(&self) -> bool {
        self.input_open
    }

    /// Whether the input is on.
    pub(crate) fn input(&self) -> bool {
        self.input
    }

    /// Turns the input on or off. Off, the input stays open but its hits are dropped. The
    /// caller keeps the setting.
    pub(crate) fn set_input(&mut self, on: bool) {
        self.input = on;
        if let Some(out) = &mut self.out {
            out.set_listening(on);
        }
    }

    /// The chosen input's name, or `None` for the output's own device.
    pub(crate) fn input_device(&self) -> Option<&str> {
        self.input_device.as_deref()
    }

    /// The name of the input the hits come from: the chosen one, or else the output's.
    pub(crate) fn input_name(&self) -> Option<&str> {
        self.input_device().or(self.choice())
    }

    /// The inputs listed when the input dialog last opened.
    pub(crate) fn inputs(&self) -> &[String] {
        &self.inputs
    }

    /// Reads the inputs there are now, for the dialog that just opened.
    pub(crate) fn list_inputs(&mut self) {
        self.inputs = self.out.as_ref().map(MidiOut::inputs).unwrap_or_default();
    }

    /// Chooses the input by name, or `None` for the output's own device. The caller keeps the
    /// choice.
    pub(crate) fn choose_input(&mut self, device: Option<String>) {
        if device == self.input_device {
            return;
        }
        self.input_device = device;
        // Not open until the sender says so.
        self.input_open = false;
        if let Some(out) = &mut self.out {
            out.select_input(input_choice(self.input_device.as_deref()));
        }
    }

    /// The MIDI output volume in percent.
    pub(crate) fn volume(&self) -> u8 {
        self.volume
    }

    /// Sets the MIDI output volume in percent. The engine sends it to the device now if MIDI
    /// out is on, and again each time it turns on. The caller keeps the setting.
    pub(crate) fn set_volume(&mut self, engine: &mut Engine, percent: u8) {
        self.volume = percent.min(100);
        engine.set_midi_volume(channel_volume(self.volume));
    }

    /// The chosen output, or `None`.
    pub(crate) fn choice(&self) -> Option<&str> {
        self.choice.as_deref()
    }

    /// Whether the drums go to the chosen output now.
    pub(crate) fn connected(&self) -> bool {
        self.sending
    }

    /// Starts sending a style file to the keyboard behind the chosen output. Does nothing before
    /// the sender has started.
    pub(crate) fn send_style(
        &mut self,
        kind: dm_project::KeyboardKind,
        file_name: String,
        data: Vec<u8>,
    ) {
        let target = match kind {
            dm_project::KeyboardKind::Yamaha => KeyboardTarget::Yamaha,
        };
        if let Some(out) = &mut self.out {
            out.send_style(target, file_name, data);
        }
    }

    /// What the style transfer is doing.
    pub(crate) fn transfer(&self) -> TransferStatus {
        self.out
            .as_ref()
            .map_or(TransferStatus::Idle, MidiOut::transfer)
    }

    /// The outputs listed when the dialog last opened.
    pub(crate) fn outputs(&self) -> &[String] {
        &self.outputs
    }

    /// Reads the outputs there are now, for the dialog that just opened.
    pub(crate) fn list(&mut self) {
        self.outputs = self.out.as_ref().map(MidiOut::outputs).unwrap_or_default();
    }

    /// Chooses the output by name, or `None` for this device. The caller keeps the choice.
    pub(crate) fn choose(&mut self, choice: Option<String>) {
        if choice == self.choice {
            return;
        }
        self.choice = choice.clone();
        self.by_hand = choice.is_some();
        if choice.is_some() {
            // Seen afresh, so a switch from one connected output to another still says where
            // the drums go now, even if the sender never reports a gap between them.
            self.last = Some(MidiStatus::Off);
        }
        if let Some(out) = &mut self.out {
            out.select(choice);
        }
    }

    /// Called once per frame with the running engine. Starts the sender the first time an
    /// engine hands out its MIDI queue (again if a new engine does), keeps the engine's MIDI out
    /// in step with the connection, and returns a message when there is something to say.
    pub(crate) fn frame(&mut self, ctx: &egui::Context, engine: &mut Engine) -> Option<Note> {
        if !MidiOut::supported() {
            return None;
        }
        if let Some(events) = engine.take_midi_events() {
            // A new engine starts with MIDI out off and the default volume, not the chosen one.
            engine.set_midi_volume(channel_volume(self.volume));
            let mut out = MidiOut::start(events);
            out.select(self.choice.clone());
            if let Some(hits) = engine.take_live_hits() {
                out.listen(hits);
            }
            // The hits come from the input named like the output unless another is chosen: one
            // choice for a keyboard. The input opens either way; the MIDI in button lets its
            // hits through.
            out.select_input(input_choice(self.input_device.as_deref()));
            out.set_listening(self.input);
            self.out = Some(out);
            self.last = None;
            self.sending = false;
            self.by_hand = false;
        }
        let out = self.out.as_ref()?;
        self.input_open = out.input_status() == MidiStatus::Connected;
        let now = out.status();
        let last = self.last.replace(now.clone()).unwrap_or(MidiStatus::Off);

        let connected = now == MidiStatus::Connected;
        if connected != self.sending {
            engine.set_midi_out(connected);
            self.sending = connected;
        }
        let note = if now == last {
            None
        } else {
            if let MidiStatus::Failed(reason) = &now {
                log::warn!("the MIDI output would not open: {reason}");
            }
            note(&last, &now, self.choice.as_deref(), self.by_hand)
        };
        if matches!(now, MidiStatus::Missing | MidiStatus::Failed(_)) {
            self.by_hand = false;
        }
        if self.choice.is_some() || self.input_device.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(POLL));
        }
        note
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_maps_onto_the_channel_volume() {
        assert_eq!(channel_volume(0), 0);
        assert_eq!(channel_volume(1), 1);
        assert_eq!(channel_volume(50), 64);
        assert_eq!(channel_volume(79), 100);
        assert_eq!(channel_volume(100), 127);
        assert_eq!(channel_volume(200), 127);
    }

    const OFF: MidiStatus = MidiStatus::Off;
    const CONNECTED: MidiStatus = MidiStatus::Connected;
    const MISSING: MidiStatus = MidiStatus::Missing;

    fn failed() -> MidiStatus {
        MidiStatus::Failed("busy".into())
    }

    fn sending() -> Option<Note> {
        Some(Note::Sending("Synth".into()))
    }

    #[test]
    fn connecting_says_where_the_drums_go() {
        for last in [OFF, MISSING, failed()] {
            assert_eq!(note(&last, &CONNECTED, Some("Synth"), false), sending());
            assert_eq!(note(&last, &CONNECTED, Some("Synth"), true), sending());
        }
    }

    #[test]
    fn a_connected_output_that_goes_says_so() {
        let gone = Some(Note::Gone("Synth".into()));
        assert_eq!(note(&CONNECTED, &MISSING, Some("Synth"), false), gone);
        assert_eq!(note(&CONNECTED, &failed(), Some("Synth"), false), gone);
    }

    #[test]
    fn a_busy_output_says_so_only_when_just_chosen() {
        let in_use = Some(Note::InUse("Synth".into()));
        assert_eq!(note(&OFF, &failed(), Some("Synth"), true), in_use);
        assert_eq!(note(&CONNECTED, &failed(), Some("Synth"), true), in_use);
        // Reconnecting at start-up, or trying again, says nothing.
        assert_eq!(note(&OFF, &failed(), Some("Synth"), false), None);
        assert_eq!(note(&failed(), &failed(), Some("Synth"), true), None);
    }

    #[test]
    fn other_changes_say_nothing() {
        // Missing at start-up, the output turned off, and no output chosen.
        assert_eq!(note(&OFF, &MISSING, Some("Synth"), false), None);
        assert_eq!(note(&OFF, &MISSING, Some("Synth"), true), None);
        assert_eq!(note(&CONNECTED, &OFF, Some("Synth"), true), None);
        assert_eq!(note(&MISSING, &OFF, None, false), None);
        assert_eq!(note(&OFF, &CONNECTED, None, false), None);
    }

    #[test]
    fn long_names_are_shortened() {
        assert_eq!(shorten("Synth", 14), "Synth");
        assert_eq!(shorten("Exactly 14 cha", 14), "Exactly 14 cha");
        assert_eq!(shorten("Microsoft GS Wavetable Synth", 14), "Microsoft GS…");
        assert_eq!(shorten("Ünïcödé nämé ẞynth", 6), "Ünïcö…");
        assert_eq!(shorten("abc", 0), "…");
    }

    #[test]
    fn a_choice_is_kept_before_any_engine_exists() {
        let mut midi = Midi::new(Some("Synth".into()), 100, false, None);
        assert_eq!(midi.choice(), Some("Synth"));
        assert!(!midi.connected());
        midi.list();
        assert!(midi.outputs().is_empty());
        midi.choose(None);
        assert_eq!(midi.choice(), None);
        assert!(!midi.by_hand);
        midi.choose(Some("Other".into()));
        assert!(midi.by_hand);
    }

    #[test]
    fn the_input_is_the_outputs_unless_another_is_chosen() {
        assert_eq!(input_choice(None), InputChoice::SameAsOutput);
        assert_eq!(
            input_choice(Some("Pads")),
            InputChoice::Named("Pads".into())
        );

        let mut midi = Midi::new(Some("Keyboard".into()), 100, true, None);
        assert_eq!(midi.input_device(), None);
        assert_eq!(midi.input_name(), Some("Keyboard"));
        midi.choose_input(Some("Pads".into()));
        assert_eq!(midi.input_device(), Some("Pads"));
        assert_eq!(midi.input_name(), Some("Pads"));
        // Pads work with no output at all.
        midi.choose(None);
        assert_eq!(midi.input_name(), Some("Pads"));
        midi.list_inputs();
        assert!(midi.inputs().is_empty());
        midi.choose_input(None);
        assert_eq!(midi.input_name(), None);
    }

    #[test]
    fn the_button_gates_an_open_input() {
        let mut midi = Midi::new(None, 100, false, Some("Pads".into()));
        midi.input_open = true;
        // Open but off: the button shows, nothing is recorded.
        assert!(midi.input_open());
        assert!(!midi.input_connected());
        midi.set_input(true);
        assert!(midi.input_connected());
        // Turning it off leaves the input open.
        midi.set_input(false);
        assert!(midi.input_open());
        assert!(!midi.input_connected());
        // A new choice is not open until the sender says so.
        midi.set_input(true);
        midi.choose_input(None);
        assert!(!midi.input_connected());
    }
}
