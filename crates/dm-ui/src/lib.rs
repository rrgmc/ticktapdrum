//! TickTapDrum's interface. Hosts call [`DrumApp::ui`] once per frame.
//!
//! Get a bank; then edit patterns of one or more bars, chain them into a song, and play either
//! one pattern on a loop or the song; choose the kit and shape each row; save and open projects,
//! with undo and redo. The project autosaves, so closing the app or a phone stopping it loses
//! nothing.
//!
//! The screen is one strip of buttons at the top and the grid below it (`strip.rs`). Everything
//! else opens in a pop-up or a dialog over the grid, never in its place.
//!
//! How much of a pattern the grid shows follows the width the app actually has, never the kind
//! of device, so an iPad in Split View gets the phone layout and a wide desktop window the
//! tablet one:
//!
//! - wide enough for 16 steps of 44 points or more: one or more whole bars side by side, up to 4;
//! - narrower: half a bar. The bar button's arrows then move by half a bar.

#[macro_use]
mod i18n;
mod bank;
mod bars;
mod drums;
mod export;
mod files;
mod grid;
mod grooves;
mod history;
#[allow(dead_code)] // Not every icon is used.
mod icons;
mod incoming;
mod keyboard;
mod link;
mod menu;
mod midi;
mod offered;
mod paste_tab;
mod patterns;
mod picker;
mod pictures;
mod projects;
mod record;
mod rows;
mod song;
mod sounds;
mod store;
mod strip;
mod tempo;
mod widgets;

use std::path::{Path, PathBuf};

pub use incoming::IncomingFiles;
pub use picker::{PickAnswers, PickKind, PickRequest, PickedFile};

use bank::BankState;
use dm_audio::Engine;
use dm_project::{Change, DEFAULT_VELOCITY, MAX_PATTERNS, Project, STEPS_PER_BAR};
use history::{Group, History};
use strip::Mode;

/// The narrowest a step may be before the grid shows fewer steps.
const MIN_STEP_WIDTH: f32 = 44.0;
/// The same on a short screen, such as a phone in landscape, so a whole bar fits.
const COMPACT_MIN_STEP_WIDTH: f32 = 32.0;
/// The most bars shown side by side.
const MAX_BARS_SHOWN: usize = 4;
/// Below this height the screen is short, such as a phone in landscape: steps may then be as
/// narrow as [`COMPACT_MIN_STEP_WIDTH`].
const SHORT: f32 = 560.0;
/// How long after the last change the autosave is written, in seconds.
const AUTOSAVE_DELAY: f64 = 1.0;
/// How long a notice stays on screen, in seconds.
const NOTICE_SECONDS: f64 = 4.0;
/// How often the interface looks for hits from the MIDI input while nothing plays, in seconds,
/// so a key's flash shows soon after the key is played.
const INPUT_POLL: f64 = 0.05;

/// The whole application state the interface draws.
pub struct DrumApp {
    bank: BankState,
    project: Project,
    /// The pattern being edited, as an index into `project.patterns()`.
    current: usize,
    mode: Mode,
    playing: bool,
    /// The first bar the grid shows.
    view_bar: usize,
    /// On narrow screens, which half of the bar shows: 0 or 1.
    half: usize,
    /// While playing, the grid shows what is being heard.
    follow: bool,
    /// How row names show: names, pictures and names, or pictures. A device setting.
    row_labels: pictures::RowLabels,
    /// Whether the bar thumbnails show under the strip, or `None` until the person chooses,
    /// when [`bar_line_default`] decides. A device setting.
    bar_line: Option<bool>,
    grid: grid::Grid,
    bar_strip: bars::BarStrip,
    tabs: patterns::PatternTabs,
    song_row: song::SongRow,
    sounds: sounds::SoundsScreen,
    /// Whether the Sounds dialog is open.
    sounds_open: bool,
    menu: menu::Menu,
    row_popup: rows::RowPopup,
    /// The saved projects, read when the Open or Save as dialog opens.
    saved_list: Vec<String>,
    store: store::Store,
    /// The project's name, once it has been saved or opened under one.
    name: Option<String>,
    /// The project as last saved under its name, to tell whether it has unsaved changes.
    saved: Option<Project>,
    /// What the last save or open reported.
    message: Option<String>,
    /// What the last export reported, for the Export dialog.
    export_message: Option<String>,
    history: History,
    /// Where exports go on a phone, which has no save dialog. `None` uses the system dialog.
    export_dir: Option<PathBuf>,
    /// The message that tells people where `export_dir` is, if its path means little to them.
    export_place: Option<&'static str>,
    /// The MIDI output the drums go to, if one is chosen, and the input drum hits come from.
    /// Started beside the engine.
    midi: midi::Midi,
    /// The number of the take being recorded from the MIDI input, while one is: from Play to
    /// Stop of a pattern, with the input on and connected.
    recording: Option<u64>,
    /// How many takes have started, to number the next one.
    takes: u64,
    /// The drums played on the MIDI input that have no row, shown as inactive rows. This
    /// session's, never saved.
    offered: offered::Offered,
    /// A one-line notice shown over the grid, and the time (egui's clock, in seconds) it goes.
    notice: Option<(String, f64)>,
    /// The language the person chose in the menu, or `None` to follow the system.
    language_choice: Option<i18n::Language>,
    /// The system's language, if the interface is translated into it.
    system_language: Option<i18n::Language>,
    /// The language the window title was last set in.
    titled: Option<(i18n::Language, Option<String>)>,
    /// A WAV export in progress.
    wav_job: Option<export::WavJob>,
    /// The keyboard style the section step chose, kept while its file is being picked.
    style_export: Option<(dm_project::KeyboardKind, dm_project::StyleSections)>,
    /// A style is on its way to the keyboard, so the transfer's outcome is still to be told.
    sending_style: bool,
    /// A download from a link in progress. It runs only while the link dialog is open.
    link_job: Option<link::LinkJob>,
    /// The egui context, kept so worker threads can ask for a redraw.
    ctx: egui::Context,
    /// Which undo group this frame's edits belong to. Set by the event handlers.
    frame_group: Group,
    /// Set by undo and redo, whose change must not itself be recorded.
    skip_record: bool,
    /// When the project last changed and the autosave is still to be written.
    unsaved_since: Option<f64>,
    /// Whether the running engine has been given the project.
    synced: bool,
    /// Whether the icon font has been given to egui.
    fonts_installed: bool,
    padding: egui::Margin,
    /// Locks the screen in landscape (`true`) or lets it turn (`false`). Set by a host that can.
    orientation: Option<Box<dyn Fn(bool)>>,
    /// What the system's Back button does when nothing is open. Set by a host that has one.
    leave: Option<Box<dyn Fn()>>,
    /// Keeps the screen on (`true`) or lets it sleep (`false`). Set by a host that can.
    keep_awake: Option<Box<dyn Fn(bool)>>,
    /// What `keep_awake` was last told.
    awake: bool,
    /// Shows the host's file picker. Set by a host whose picker answers later, such as Android.
    picker: Option<Box<dyn Fn(PickRequest)>>,
    /// Where the host's answers to picks arrive.
    picks: picker::PickInbox,
    /// The picks sent to the host's picker and not yet answered.
    pending: files::Pending,
    /// Where files a host hands in, such as a shared groove pack, arrive.
    incoming: incoming::IncomingInbox,
    /// The grooves that come with the app, read once.
    magenta: dm_groove::Pack,
    /// The groove list's packs and their keys, read when the list opens.
    groove_packs: Vec<(String, dm_groove::Pack)>,
    /// The line at the top of the groove list: what the last import did, or why an add failed.
    groove_note: Option<String>,
    /// The groove playing on its own, as its pack's key and its place in the pack.
    preview: Option<(String, usize)>,
}

impl DrumApp {
    /// `data_dir` is a writable folder private to the app, where the bank, the projects and the
    /// autosave live. The autosaved project, if any, comes back.
    pub fn new(data_dir: PathBuf) -> Self {
        let store = store::Store::new(&data_dir);
        let (project, name) = store.restore().unwrap_or((Project::new(), None));
        let saved = name.as_deref().and_then(|n| store.load(n).ok());
        let language_choice = store.language();
        let system_language = i18n::system_language();
        let row_labels = store.row_labels();
        let bar_line = store.bar_strip();
        let midi = midi::Midi::new(store.midi_output(), store.midi_volume(), store.midi_input());
        // Only the names: reading the packs waits until the list opens.
        let app = Self {
            bank: BankState::new(data_dir),
            project,
            current: 0,
            mode: Mode::Pattern,
            playing: false,
            view_bar: 0,
            half: 0,
            follow: true,
            row_labels,
            bar_line,
            grid: grid::Grid::default(),
            bar_strip: bars::BarStrip::default(),
            tabs: patterns::PatternTabs::default(),
            song_row: song::SongRow::default(),
            sounds: sounds::SoundsScreen::default(),
            sounds_open: false,
            menu: menu::Menu::default(),
            row_popup: rows::RowPopup::default(),
            saved_list: Vec::new(),
            store,
            name,
            saved,
            message: None,
            export_message: None,
            history: History::default(),
            export_dir: None,
            export_place: None,
            midi,
            recording: None,
            takes: 0,
            offered: offered::Offered::default(),
            notice: None,
            language_choice,
            system_language,
            titled: None,
            wav_job: None,
            style_export: None,
            sending_style: false,
            link_job: None,
            ctx: egui::Context::default(),
            frame_group: Group::Single,
            skip_record: false,
            unsaved_since: None,
            synced: false,
            fonts_installed: false,
            padding: egui::Margin::same(8),
            orientation: None,
            leave: None,
            keep_awake: None,
            awake: false,
            picker: None,
            picks: picker::PickInbox::new(),
            pending: files::Pending::default(),
            incoming: incoming::IncomingInbox::new(),
            magenta: grooves::magenta_pack(),
            groove_packs: Vec::new(),
            groove_note: None,
            preview: None,
        };
        app.apply_language();
        app
    }

    /// For a host that knows the system's language better than the app can ask, such as
    /// Android: `tag` is a BCP 47 tag such as `pt-BR`. Unless the person chose a language in the
    /// menu, text follows it.
    pub fn with_system_locale(mut self, tag: &str) -> Self {
        self.set_system_locale(tag);
        self
    }

    /// The same, for a host that learns of a change while the app runs.
    pub fn set_system_locale(&mut self, tag: &str) {
        self.system_language = i18n::Language::matching(tag);
        self.apply_language();
    }

    /// The language text is shown in: the person's choice, else the system's, else English.
    fn effective_language(&self) -> i18n::Language {
        self.language_choice
            .or(self.system_language)
            .unwrap_or(i18n::Language::English)
    }

    fn apply_language(&self) {
        i18n::set_language(self.effective_language());
    }

    /// Shows text in `choice` from now on, or in the system's language if `None`, and keeps the
    /// choice for the next start. Messages already shown were in the old language, so they go.
    pub(crate) fn set_language_choice(&mut self, choice: Option<i18n::Language>) {
        self.language_choice = choice;
        if let Err(e) = self.store.set_language(choice) {
            log::warn!("could not keep the language setting: {e}");
        }
        self.apply_language();
        self.message = None;
        self.export_message = None;
    }

    /// Where a phone saves exported MIDI and WAV files, since it has no save dialog. Desktops
    /// leave this unset and ask where to save each file.
    pub fn with_export_dir(mut self, dir: PathBuf) -> Self {
        self.export_dir = Some(dir);
        self
    }

    /// How the Export dialog names the export folder, instead of its path: the id of a message
    /// in `locales/*/app.ftl`, so it is translated with the rest. A phone's folder is reached
    /// through a file manager, where the path is not what people see.
    pub fn with_export_place(mut self, message: &'static str) -> Self {
        self.export_place = Some(message);
        self
    }

    /// For a host that can lock the screen's orientation: `lock(true)` keeps it in landscape,
    /// `lock(false)` lets it turn. The â˜° menu then offers the setting. The saved setting
    /// is applied at once.
    pub fn with_orientation_lock(mut self, lock: impl Fn(bool) + 'static) -> Self {
        if self.store.landscape() {
            lock(true);
        }
        self.orientation = Some(Box::new(lock));
        self
    }

    /// For a host with a system Back button, such as Android: what Back does when no pop-up or
    /// dialog is open, typically sending the app to the background. While one is open, Back
    /// closes it instead, as a tap outside it does.
    pub fn with_back_leaves(mut self, leave: impl Fn() + 'static) -> Self {
        self.leave = Some(Box::new(leave));
        self
    }

    /// For a host that can stop the screen from sleeping: `keep(true)` keeps it on, `keep(false)`
    /// lets it sleep again. The app keeps the screen on while it plays, so a pattern or song
    /// left playing is not cut short by the screen locking.
    pub fn with_keep_awake(mut self, keep: impl Fn(bool) + 'static) -> Self {
        self.keep_awake = Some(Box::new(keep));
        self
    }

    /// For a host with its own file picker that answers later, such as Android's: `pick` shows
    /// it for the request, and must return at once. The host hands back the result through
    /// [`DrumApp::pick_answers`]. The menu then offers opening and saving project files, and
    /// exports go through the picker instead of the export folder.
    pub fn with_file_picker(mut self, pick: impl Fn(PickRequest) + 'static) -> Self {
        self.picker = Some(Box::new(pick));
        self
    }

    /// Where the host hands back the files its picker returns. See [`DrumApp::with_file_picker`].
    pub fn pick_answers(&self) -> PickAnswers {
        self.picks.answers()
    }

    /// Where a host hands in files from outside the app to import as grooves, such as a MIDI
    /// file or a zip shared to the app. Each is imported on the next frame, as a file chosen with
    /// "Add grooves from a file" is. See [`IncomingFiles::send`].
    pub fn incoming_files(&self) -> IncomingFiles {
        self.incoming.files()
    }

    /// Space kept clear around the content, in points. A phone host uses it to stay clear of the
    /// status bar and the navigation bar, which eframe does not report on Android.
    pub fn with_padding(mut self, padding: egui::Margin) -> Self {
        self.padding = padding;
        self
    }

    /// Changes the space kept clear, for a host whose system bars move, such as a phone turning
    /// or the iOS safe area changing.
    pub fn set_padding(&mut self, padding: egui::Margin) {
        self.padding = padding;
    }

    /// Development shortcut: use this `.sf2` directly and skip the bank download.
    pub fn with_soundfont_file(mut self, path: PathBuf) -> Self {
        self.bank.use_file(path);
        self
    }

    /// Development shortcut: install the bank from this local archive instead of downloading it.
    /// The archive is verified exactly as a download would be.
    pub fn with_local_archive(mut self, path: PathBuf) -> Self {
        self.bank.use_local_archive(path);
        self
    }

    /// Draws one frame.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let language = self.effective_language();
        // The window's title names the project, once it has a name.
        let titled = Some((language, self.name.clone()));
        if self.titled != titled {
            let title = match &self.name {
                Some(name) => tr!("app-title-named", name = name.as_str()),
                None => tr!("app-name"),
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
            self.titled = titled;
        }
        if !self.fonts_installed {
            icons::install(&ctx);
            self.fonts_installed = true;
        }
        self.bank.poll(&ctx);
        self.picks.set_context(&ctx);
        self.incoming.set_context(&ctx);
        self.ctx = ctx.clone();
        while let Some(incoming) = self.incoming.take() {
            match incoming {
                incoming::Incoming::File { name, bytes } => self.import_grooves(&name, bytes),
                incoming::Incoming::Text(text) => self.incoming_text(&text),
            }
        }
        self.poll_link();
        self.dropped_files(&ctx);
        self.back(&ctx);
        let frame = egui::Frame::central_panel(ui.style()).inner_margin(self.padding);
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            widgets::set_safe_area(ui.ctx(), ui.max_rect());
            // Taken out of `self` for the frame, so the editor can borrow `self` mutably.
            if self.bank.engine_mut().is_none() {
                self.bank.ui(ui);
                return;
            }
            let mut engine = self.bank.take_engine();
            self.editor(ui, &mut engine);
            // After the editor, so a new engine has the project, and its kit, before MIDI out
            // turns on and sends the kit.
            self.style_transfer(ui.ctx());
            let mut notes: Vec<midi::Note> =
                self.midi.frame(ui.ctx(), &mut engine).into_iter().collect();
            // An input that goes away ends the take. What was recorded stays. Its one line
            // replaces the output's, since a keyboard takes both away at once.
            if self.recording.is_some() && !self.midi.input_connected() {
                self.recording = None;
                let name = self.midi.choice().unwrap_or_default().to_owned();
                notes = vec![midi::Note::InputGone(name)];
            }
            for note in notes {
                let text = note.text();
                let until = ui.input(|i| i.time) + NOTICE_SECONDS;
                self.notice = Some((text.clone(), until));
                self.message = Some(text);
            }
            self.show_notice(ui);
            // Sends changes that waited because the audio queue was full.
            if !engine.flush() {
                ui.ctx().request_repaint();
            }
            self.bank.put_engine(engine);
        });
        self.update_keep_awake();
    }

    /// Shows the current notice, if any, as one line near the bottom of the screen until it
    /// expires. Messages in the menu's status line are seen only when the menu opens; a notice is
    /// for news the person should see at once, such as a MIDI output going away.
    fn show_notice(&mut self, ui: &egui::Ui) {
        let Some((text, until)) = &self.notice else {
            return;
        };
        let now = ui.input(|i| i.time);
        if now >= *until {
            self.notice = None;
            return;
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(until - now));
        let rect = ui.max_rect();
        egui::Area::new(egui::Id::unique("dm-notice"))
            .order(egui::Order::Foreground)
            .interactable(false)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(egui::pos2(rect.center().x, rect.bottom() - 16.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(rect.width() - 32.0);
                    ui.label(text.as_str());
                });
            });
    }

    /// Tells the host to keep the screen on when playing starts, and to let it sleep when
    /// playing stops. Checked after every frame, so every way of starting or stopping counts.
    fn update_keep_awake(&mut self) {
        if self.awake == self.playing {
            return;
        }
        self.awake = self.playing;
        if let Some(keep) = &self.keep_awake {
            keep(self.playing);
        }
    }

    /// The system's Back button, which egui reports as `BrowserBack`. With a pop-up or dialog
    /// open it becomes Escape, which closes that the way a tap outside does. With nothing open it
    /// does what the host asked for, if anything.
    fn back(&mut self, ctx: &egui::Context) {
        if !ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack)) {
            return;
        }
        // Any window over the grid counts, the question windows included.
        let open = egui::Popup::is_any_open(ctx)
            || ctx.memory(|m| m.top_modal_layer().is_some())
            || self.row_popup.is_open()
            || self.menu.dialog_open()
            || self.sounds_open;
        if open {
            ctx.input_mut(|i| {
                i.events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            });
        } else if let Some(leave) = &self.leave {
            leave();
        }
    }

    /// One frame of editing, with undo recording and the autosave around it.
    fn editor(&mut self, ui: &mut egui::Ui, engine: &mut Engine) {
        let now = ui.input(|i| i.time);
        // A copy of the project before this frame's edits. Typically a few kilobytes.
        let before = self.project.clone();
        let message_before = self.message.clone();
        self.frame_group = Group::Single;
        self.skip_record = false;
        self.ctx = ui.ctx().clone();
        self.shortcuts(ui, engine);
        // Answers from the host's picker. Backing out, and answers to nothing asked, do nothing.
        while let Some((id, file)) = self.picks.take() {
            if let Some((kind, file)) = self.pending.collect(id, file) {
                self.carry_out(engine, kind, files::Target::Picked(file));
            }
        }
        if let Some(result) = self.wav_job.as_ref().and_then(export::WavJob::poll) {
            self.wav_job = None;
            self.message = Some(match result {
                Ok(target) => tr!("projects-wav-saved", place = self.saved_where(&target)),
                Err(e) => tr!("projects-wav-failed", reason = e),
            });
            self.export_message = self.message.clone();
        }
        // A drum on offer that has a row by now, added another way, is offered no more.
        self.offered.forget_rows_of(&self.project);
        self.editor_frame(ui, engine);

        if self.project != before {
            if !self.skip_record {
                self.history.record(&before, self.frame_group, now);
                // An edit makes "Saved" or "Opened" out of date, but not a message about the
                // edit itself, such as what adding a groove did.
                if self.message == message_before {
                    self.message = None;
                }
            }
            self.unsaved_since = Some(now);
        }
        self.record_hits(engine, now);
        // The autosave waits for a pause in editing, but not when the app loses focus: a phone
        // may end the app soon after the user leaves it.
        let focused = ui.input(|i| i.focused);
        if let Some(since) = self.unsaved_since {
            if now - since >= AUTOSAVE_DELAY || !focused {
                if let Err(e) = self.store.autosave(&self.project, self.name.as_deref()) {
                    log::warn!("autosave failed: {e}");
                }
                self.unsaved_since = None;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(AUTOSAVE_DELAY));
            }
        }
    }

    /// Undo, redo and save from a keyboard, when one is attached and no text box has focus.
    fn shortcuts(&mut self, ui: &mut egui::Ui, engine: &mut Engine) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        let redo_y = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Y);
        let redo_z = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
        let save = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
        // The longer shortcut first, so Ctrl+Shift+Z is not taken for Ctrl+Z.
        if ui.input_mut(|i| i.consume_shortcut(&redo_z) || i.consume_shortcut(&redo_y)) {
            self.redo(engine);
        } else if ui.input_mut(|i| i.consume_shortcut(&undo)) {
            self.undo(engine);
        }
        if ui.input_mut(|i| i.consume_shortcut(&save)) {
            if self.name.is_some() {
                self.handle_projects(engine, vec![projects::ProjectEvent::Save]);
            } else {
                self.saved_list = self.store.list();
                self.menu.open_save_as();
            }
        }
    }

    fn undo(&mut self, engine: &mut Engine) {
        if self.history.undo(&mut self.project) {
            // Undo works on the project alone; notes waiting on a drum on offer go.
            self.offered.drop_notes();
            self.skip_record = true;
            self.resync(engine);
        }
    }

    fn redo(&mut self, engine: &mut Engine) {
        if self.history.redo(&mut self.project) {
            self.offered.drop_notes();
            self.skip_record = true;
            self.resync(engine);
        }
    }

    /// Brings the engine fully in line with the project, after undo, redo, open or new. Every
    /// slot is cleared first, so nothing from the previous state stays behind.
    fn resync(&mut self, engine: &mut Engine) {
        for slot in 0..MAX_PATTERNS {
            engine.clear_pattern(slot);
        }
        engine.set_tempo(self.project.tempo);
        engine.set_song_loop(self.project.song_loop);
        apply(engine, self.project.full_sync());
        self.current = self.current.min(self.project.patterns().len() - 1);
        let bars = self.project.patterns()[self.current].bars();
        self.view_bar = self.view_bar.min(bars - 1);
    }

    fn editor_frame(&mut self, ui: &mut egui::Ui, engine: &mut Engine) {
        if !self.synced {
            engine.set_tempo(self.project.tempo);
            engine.set_song_loop(self.project.song_loop);
            apply(engine, self.project.full_sync());
            self.synced = true;
        }

        if self.playing && !engine.is_playing() {
            // A song that does not repeat has reached its end.
            self.playing = false;
        }
        let playhead = engine.playhead();
        if self.playing || playhead.is_some() {
            // The playhead moves on its own; keep drawing. The second condition covers the end
            // of a song, which the engine reports slightly before the last step stops sounding.
            ui.ctx().request_repaint();
        }
        let playing_index = playhead.and_then(|p| self.project.index_of_slot(p.pattern));

        // Follow: in song mode the editor shows the pattern being heard; in both modes the grid
        // shows the bar being heard.
        if self.playing
            && self.follow
            && let Some(p) = playhead
        {
            if self.mode == Mode::Song
                && let Some(index) = playing_index
            {
                self.current = index;
            }
            if playing_index == Some(self.current) {
                self.view_bar = p.step / STEPS_PER_BAR;
                self.half = (p.step % STEPS_PER_BAR) / (STEPS_PER_BAR / 2);
            }
        }
        self.current = self.current.min(self.project.patterns().len() - 1);

        // Which steps the grid shows. A short screen, such as a phone in landscape, allows
        // narrower steps, so a whole bar fits and the short rows are not split again.
        let width = ui.available_width();
        let short = ui.available_height() < SHORT;
        let min_step = if short {
            COMPACT_MIN_STEP_WIDTH
        } else {
            MIN_STEP_WIDTH
        };
        let pattern_bars = self.project.patterns()[self.current].bars();
        // The grid's height is not known yet: the strip and the bar line come first. The height
        // left now is a little more, so near the edge the guess may pick the narrower column.
        let rows_tall = grid::row_height(
            ui.available_height() - bars::INDICATOR_HEIGHT - bars::INDICATOR_GAP,
            self.project.rows().len() + self.offered.len(),
        );
        let labels = grid::label_width(
            ui.ctx(),
            &self.project,
            &self.offered.keys(),
            self.row_labels,
            width,
            rows_tall,
        );
        let fits = ((width - labels) / (STEPS_PER_BAR as f32 * min_step)).floor() as usize;
        let split = fits == 0;
        let shown_bars = fits.clamp(1, MAX_BARS_SHOWN).min(pattern_bars);
        self.view_bar = self.view_bar.min(pattern_bars - shown_bars);
        if !split {
            self.half = 0;
        }
        let shown = self.view_bar..self.view_bar + shown_bars;
        let heard = playing_index == Some(self.current);
        let playing_bar = playhead.filter(|_| heard).map(|p| p.step / STEPS_PER_BAR);
        let playing_half = playhead
            .filter(|p| split && heard && p.step / STEPS_PER_BAR == self.view_bar)
            .map(|p| (p.step % STEPS_PER_BAR) / (STEPS_PER_BAR / 2));

        // The strip.
        let can_redo = self.history.can_redo();
        let state = strip::StripState {
            playing: self.playing,
            mode: self.mode,
            pattern: self.project.patterns()[self.current].name(),
            pattern_heard: heard,
            bars: strip::BarView {
                first: self.view_bar,
                shown: shown_bars,
                total: pattern_bars,
                half: split.then_some(self.half),
                playing_half,
                can_prev: self.view_bar > 0 || (split && self.half == 1),
                can_next: if split {
                    self.half == 0 || self.view_bar + 1 < pattern_bars
                } else {
                    self.view_bar + shown_bars < pattern_bars
                },
            },
            bpm: self.project.tempo,
            can_undo: self.history.can_undo(),
            can_redo,
            step_levels: self.project.step_levels(),
            midi_device: self.midi.choice().filter(|_| self.midi.connected()),
            recording_from: self.midi.choice().filter(|_| self.midi.input_connected()),
            midi_in: self.midi.connected().then_some(self.midi.input()),
        };
        let strip = strip::ui(ui, &state);
        ui.add_space(4.0);
        // The bar thumbnails, all the time, when the setting says so.
        let bar_line = self.bar_line.unwrap_or_else(|| bar_line_default(ui.ctx()));
        let line_shown = bar_line;
        let line_events = if line_shown {
            let events = self.bar_strip.line(
                ui,
                &self.project.patterns()[self.current],
                shown.clone(),
                playing_bar,
                self.follow,
                self.mode == Mode::Pattern,
            );
            ui.add_space(4.0);
            events
        } else {
            Vec::new()
        };

        // The pop-ups under the strip's buttons.
        // The pattern pop-up edits the patterns and the song: what Play plays at the top, the
        // patterns in the middle, the song's play order at the end.
        let playing_entry = playhead.and_then(|p| p.song_index);
        let (pattern_events, song_events) = widgets::popup(&strip.pattern, |ui| {
            ui.set_width(patterns::popup_width(ui, patterns::POPUP_MAX_WIDTH));
            let mut song = self.song_row.mode_contents(ui, self.mode);
            ui.separator();
            let patterns = self
                .tabs
                .popup_contents(ui, &self.project, self.current, playing_index);
            ui.separator();
            song.extend(self.song_row.order_contents(
                ui,
                &self.project,
                self.current,
                playing_entry,
            ));
            (patterns, song)
        })
        .unwrap_or_default();
        let bar_events = widgets::popup(&strip.bar_label, |ui| {
            self.bar_strip.popup_contents(
                ui,
                &self.project.patterns()[self.current],
                shown.clone(),
                playing_bar,
                self.follow,
                !line_shown,
                self.mode == Mode::Pattern,
            )
        })
        .unwrap_or_default();
        let tempo_events = strip
            .tempo
            .as_ref()
            .and_then(|button| {
                widgets::popup(button, |ui| {
                    tempo::contents(ui, self.project.tempo, self.project.swing())
                })
            })
            .unwrap_or_default();
        // Questions the pattern and bar pop-ups asked, each in a window of its own.
        let mut pattern_events = pattern_events;
        pattern_events.extend(self.tabs.confirm(ui.ctx(), &self.project, self.current));
        let mut bar_events = bar_events;
        bar_events.extend(line_events);
        let bars_now = self.project.patterns()[self.current].bars();
        bar_events.extend(self.bar_strip.confirm(ui.ctx(), bars_now));
        // With a picker, each export asks where to go, so there is no folder to name.
        let export_place = if self.picker.is_some() {
            None
        } else {
            self.export_place.map(i18n::tr).or_else(|| {
                self.export_dir
                    .as_ref()
                    .map(|dir| dir.display().to_string())
            })
        };
        let view = menu::MenuView {
            project: &self.project,
            projects: projects::ProjectsView {
                name: self.name.as_deref(),
                unsaved: self.saved.as_ref() != Some(&self.project),
                saved: &self.saved_list,
                file_items: files::DIALOGS || self.picker.is_some(),
                message: self.message.as_deref(),
                export_message: self.export_message.as_deref(),
                exporting: self.wav_job.as_ref().map(export::WavJob::progress),
                sending: self.sending_style.then(|| match self.midi.transfer() {
                    midi::TransferStatus::Running(fraction) => fraction,
                    _ => 0.0,
                }),
                can_render: self.bank.soundfont().is_some(),
                export_place: export_place.as_deref(),
                landscape: self.orientation.is_some().then(|| self.store.landscape()),
                row_labels: self.row_labels,
                bar_line,
            },
            hidden: strip.hidden,
            can_undo: self.history.can_undo(),
            can_redo,
            bpm: self.project.tempo,
            swing: self.project.swing(),
            language: self.language_choice,
            system_language: self.system_language.unwrap_or(i18n::Language::English),
            midi: midi::MidiOut::supported().then(|| menu::MidiView {
                choice: self.midi.choice(),
                connected: self.midi.connected(),
                outputs: self.midi.outputs(),
                volume: self.midi.volume(),
            }),
            grooves: grooves::GroovesView {
                magenta: &self.magenta,
                packs: &self.groove_packs,
                playing: self.preview.as_ref().map(|(key, i)| (key.as_str(), *i)),
                note: self.groove_note.as_deref(),
            },
            downloading: self.link_job.as_ref().map(link::LinkJob::progress),
        };
        let mut menu_events =
            widgets::popup(&strip.menu, |ui| self.menu.contents(ui, &view)).unwrap_or_default();
        menu_events.extend(self.menu.dialogs(ui.ctx(), &view));

        // The dialogs the strip and the menu open.
        let mut sound_events = Vec::new();
        if self.sounds_open {
            let kits = engine.kits().to_vec();
            let dialog = widgets::dialog(ui.ctx(), "sounds", &tr!("menu-sounds"), |ui| {
                self.sounds
                    .dialog_contents(ui, &self.project, &kits, self.row_labels)
            });
            sound_events = dialog.inner;
            self.sounds_open = !dialog.closed;
        }

        // The grid. While anything is open above it, a tap only closes that and changes no step.
        let view = if split {
            grid::View {
                first_step: self.view_bar * STEPS_PER_BAR + self.half * STEPS_PER_BAR / 2,
                visible: STEPS_PER_BAR / 2,
            }
        } else {
            grid::View {
                first_step: self.view_bar * STEPS_PER_BAR,
                visible: shown_bars * STEPS_PER_BAR,
            }
        };
        // A row's own pop-up leaves the names live, so another name opens at the first tap.
        let other_open =
            egui::Popup::is_any_open(ui.ctx()) || self.menu.dialog_open() || self.sounds_open;
        self.grid
            .set_blocked(other_open || self.row_popup.is_open(), other_open);
        self.grid.set_highlight(self.row_popup.row());
        self.grid.set_row_labels(self.row_labels);
        self.grid.set_step_levels(self.project.step_levels());
        let grid_playhead = playhead.filter(|_| heard).map(|p| p.step);
        // The grid takes the rest of the screen but a thin line at the bottom, the bar
        // indicator, which is inside the safe area so a phone's gesture bar does not hide it.
        let rest = ui.available_rect_before_wrap();
        let indicator_top = rest.bottom() - bars::INDICATOR_HEIGHT;
        let grid_rect = egui::Rect::from_min_max(
            rest.min,
            egui::pos2(rest.right(), indicator_top - bars::INDICATOR_GAP),
        );
        let grid_events = ui
            .scope_builder(egui::UiBuilder::new().max_rect(grid_rect), |ui| {
                self.grid.ui(
                    ui,
                    &self.project,
                    &self.offered,
                    &self.project.patterns()[self.current],
                    view,
                    grid_playhead,
                )
            })
            .inner;
        bars::indicator(
            ui,
            egui::Rect::from_min_max(egui::pos2(rest.left(), indicator_top), rest.max),
            pattern_bars,
            shown.clone(),
        );
        for event in &grid_events {
            if let grid::GridEvent::OpenRow { row, rect } = event {
                self.row_popup.open(*row, *rect);
            }
        }
        let row_events = self.row_popup.show(ui.ctx(), &self.project, &self.grid);

        // Apply everything the user did this frame.
        for event in grid_events {
            match event {
                grid::GridEvent::Preview { row } => self.preview_row(engine, row),
                grid::GridEvent::OpenRow { .. } => {}
                grid::GridEvent::AddOffered { key } => {
                    let now = ui.input(|i| i.time);
                    self.add_offered(engine, key, now);
                }
                grid::GridEvent::Set {
                    row,
                    step,
                    velocity,
                } => {
                    self.frame_group = Group::Paint(self.grid.gesture());
                    if let Ok(changes) = self.project.set_step(self.current, row, step, velocity) {
                        apply(engine, changes);
                    }
                    if velocity > 0
                        && let Some(info) = self.project.rows().get(row)
                    {
                        // Heard at the level just set, so a soft or loud step sounds like one.
                        let heard = scaled(velocity, info.volume);
                        if heard > 0 {
                            engine.hit(info.key, heard);
                        }
                    }
                }
            }
        }
        for event in row_events {
            match event {
                rows::RowEvent::Sound(event) => self.handle_sounds(engine, vec![event]),
                rows::RowEvent::Fill { row, every } => self.fill_row(engine, row, Some(every)),
                rows::RowEvent::ClearRow { row } => self.fill_row(engine, row, None),
            }
        }
        self.handle_sounds(engine, sound_events);
        for event in strip.events {
            match event {
                strip::StripEvent::Play => self.play(engine),
                strip::StripEvent::Stop => {
                    engine.stop();
                    self.playing = false;
                    self.recording = None;
                }
                strip::StripEvent::MidiIn => {
                    let on = !self.midi.input();
                    if let Err(e) = self.store.set_midi_input(on) {
                        log::warn!("could not keep the MIDI input setting: {e}");
                    }
                    self.midi.set_input(on);
                    // Turning the input off ends the take, without the line about a keyboard
                    // that went away, and takes the drums on offer away with their notes.
                    if !on {
                        self.recording = None;
                        self.offered = offered::Offered::default();
                    }
                }
                strip::StripEvent::PrevPage => self.page(split, shown_bars, false),
                strip::StripEvent::NextPage => self.page(split, shown_bars, true),
                strip::StripEvent::Undo => self.undo(engine),
                strip::StripEvent::Redo => self.redo(engine),
                strip::StripEvent::StepLevels => {
                    let next = self.project.step_levels().next();
                    self.project.set_step_levels(next);
                }
            }
        }
        for event in tempo_events {
            self.handle_tempo(engine, event);
        }
        for event in menu_events {
            match event {
                menu::MenuEvent::Project(event) => self.handle_projects(engine, vec![event]),
                menu::MenuEvent::ListSaved => self.saved_list = self.store.list(),
                menu::MenuEvent::Undo => self.undo(engine),
                menu::MenuEvent::Redo => self.redo(engine),
                menu::MenuEvent::Sounds => self.sounds_open = true,
                menu::MenuEvent::Tempo(event) => self.handle_tempo(engine, event),
                menu::MenuEvent::ListMidiOutputs => {
                    self.midi.list();
                    // This frame drew the dialog with the old list.
                    ui.ctx().request_repaint();
                }
                menu::MenuEvent::MidiOutput(choice) => {
                    if let Err(e) = self.store.set_midi_output(choice.as_deref()) {
                        log::warn!("could not keep the MIDI output setting: {e}");
                    }
                    self.midi.choose(choice);
                }
                menu::MenuEvent::MidiVolume { percent, keep } => {
                    self.midi.set_volume(engine, percent);
                    if keep && let Err(e) = self.store.set_midi_volume(percent) {
                        log::warn!("could not keep the MIDI volume setting: {e}");
                    }
                }
                menu::MenuEvent::Language(choice) => {
                    self.set_language_choice(choice);
                    // This frame was drawn in the old language; the next shows the new one.
                    ui.ctx().request_repaint();
                }
                menu::MenuEvent::Grooves(event) => {
                    self.handle_grooves(engine, event);
                    // This frame drew the list as it was.
                    ui.ctx().request_repaint();
                }
            }
        }
        // A groove plays only while its list is open.
        if self.preview.is_some() && !self.menu.grooves_open() {
            self.stop_preview(engine);
        }
        for event in bar_events {
            let result = match event {
                bars::BarEvent::Select(bar) => {
                    self.view_bar = bar;
                    self.half = 0;
                    if self.playing {
                        self.follow = false;
                    }
                    Ok(Vec::new())
                }
                bars::BarEvent::AddBar => {
                    let result = self.project.add_bar(self.current);
                    if result.is_ok() {
                        self.view_bar = self.project.patterns()[self.current].bars() - 1;
                    }
                    result
                }
                // The bar goes with its notes: nothing is kept hidden for later.
                bars::BarEvent::RemoveBar => {
                    self.project
                        .remove_bar(self.current)
                        .and_then(|mut changes| {
                            changes.extend(self.project.clear_hidden_bars(self.current)?);
                            Ok(changes)
                        })
                }
                bars::BarEvent::CopyBar => {
                    let result = self.project.copy_bar(self.current, self.view_bar);
                    if result.is_ok() {
                        self.view_bar += 1;
                    }
                    result
                }
                bars::BarEvent::ToggleFollow => {
                    self.follow = !self.follow;
                    Ok(Vec::new())
                }
                bars::BarEvent::PlayFrom(bar) => {
                    self.play_from_bar(engine, bar);
                    Ok(Vec::new())
                }
            };
            self.finish(engine, result);
        }
        for event in pattern_events {
            match event {
                patterns::PatternEvent::Select(index) => self.select_pattern(engine, index),
                patterns::PatternEvent::Add => {
                    let result = self.project.add_pattern().map(|(index, changes)| {
                        apply(engine, changes);
                        index
                    });
                    if let Ok(index) = result {
                        self.select_pattern(engine, index);
                    }
                }
                patterns::PatternEvent::Copy => {
                    let result = self
                        .project
                        .copy_pattern(self.current)
                        .map(|(index, changes)| {
                            apply(engine, changes);
                            index
                        });
                    if let Ok(index) = result {
                        self.select_pattern(engine, index);
                    }
                }
                patterns::PatternEvent::Delete => {
                    let deleting_playing = self.playing && playing_index == Some(self.current);
                    let result = self.project.delete_pattern(self.current);
                    if result.is_ok() {
                        self.current = self.current.saturating_sub(1);
                        self.view_bar = 0;
                        if deleting_playing {
                            engine.stop();
                            self.playing = false;
                        }
                    }
                    self.finish(engine, result);
                }
                // Only the letters change; nothing that plays does.
                patterns::PatternEvent::Rename(name) => {
                    let result = self.project.rename_pattern(self.current, name);
                    self.finish(engine, result.map(|()| Vec::new()));
                }
            }
        }
        for event in song_events {
            let result = match event {
                song::SongEvent::Append => self.project.song_append(self.current),
                song::SongEvent::MoveLeft(entry) => self.project.song_move(entry, entry - 1),
                song::SongEvent::MoveRight(entry) => self.project.song_move(entry, entry + 1),
                song::SongEvent::Remove(entry) => self.project.song_remove(entry),
                song::SongEvent::SetMode(mode) => {
                    self.set_mode(engine, mode);
                    Ok(Vec::new())
                }
                song::SongEvent::ToggleLoop => {
                    self.project.song_loop = !self.project.song_loop;
                    engine.set_song_loop(self.project.song_loop);
                    Ok(Vec::new())
                }
            };
            self.finish(engine, result);
        }
    }

    fn set_mode(&mut self, engine: &mut Engine, mode: Mode) {
        self.mode = mode;
        if self.playing {
            self.play(engine);
        }
    }

    fn handle_tempo(&mut self, engine: &mut Engine, event: tempo::TempoEvent) {
        match event {
            tempo::TempoEvent::Tempo(bpm) => {
                self.frame_group = Group::Tempo;
                self.project.tempo = bpm;
                engine.set_tempo(bpm);
            }
            tempo::TempoEvent::Swing(amount) => {
                self.frame_group = Group::Swing;
                apply(engine, self.project.set_swing(amount));
            }
        }
    }

    /// Moves the grid one screen back or forward: half a bar when the grid shows half a bar,
    /// otherwise as many bars as it shows.
    /// Past the last screen it wraps round to the first, and before the first to the last.
    fn page(&mut self, split: bool, shown_bars: usize, forward: bool) {
        let bars = self.project.patterns()[self.current].bars();
        if split {
            let pages = bars * 2;
            let page = self.view_bar * 2 + self.half;
            let page = if forward {
                (page + 1) % pages
            } else {
                (page + pages - 1) % pages
            };
            self.view_bar = page / 2;
            self.half = page % 2;
        } else {
            let last = bars - shown_bars;
            self.view_bar = match (forward, self.view_bar) {
                (true, v) if v >= last => 0,
                (true, v) => (v + shown_bars).min(last),
                (false, 0) => last,
                (false, v) => v.saturating_sub(shown_bars),
            };
        }
        if self.playing {
            self.follow = false;
        }
    }

    /// Makes a row in the pattern being edited play on every `every`-th step and nowhere else,
    /// in every bar the pattern plays. `None` clears the row.
    fn fill_row(&mut self, engine: &mut Engine, row: usize, every: Option<usize>) {
        let steps = self.project.patterns()[self.current].bars() * STEPS_PER_BAR;
        for step in 0..steps {
            let velocity = if every.is_some_and(|n| step % n == 0) {
                DEFAULT_VELOCITY
            } else {
                0
            };
            if let Ok(changes) = self.project.set_step(self.current, row, step, velocity) {
                apply(engine, changes);
            }
        }
    }

    fn handle_sounds(&mut self, engine: &mut Engine, events: Vec<sounds::SoundEvent>) {
        use sounds::SoundEvent;
        for event in events {
            let result = match event {
                SoundEvent::Kit(program) => Ok(self.project.set_kit(program)),
                SoundEvent::Rename(row, name) => {
                    self.project.rename_row(row, &name).map(|()| Vec::new())
                }
                SoundEvent::Mute(row) => self.project.toggle_mute(row),
                SoundEvent::Solo(row) => self.project.toggle_solo(row),
                SoundEvent::Volume(row, percent) => {
                    self.frame_group = Group::Volume(row);
                    self.project.set_volume(row, percent)
                }
                SoundEvent::MoveUp(row) => self.project.move_row(row, row - 1),
                SoundEvent::MoveDown(row) => self.project.move_row(row, row + 1),
                SoundEvent::SetDrum(row, key) => self.project.set_row_key(row, key),
                SoundEvent::Remove(row) => self.project.remove_row(row),
                SoundEvent::Add(key) => self.project.add_row(key),
                SoundEvent::Preview(key, percent) => {
                    hit_at(engine, key, percent);
                    Ok(Vec::new())
                }
            };
            self.finish(engine, result);
        }
    }

    fn handle_projects(&mut self, engine: &mut Engine, events: Vec<projects::ProjectEvent>) {
        use projects::ProjectEvent;
        for event in events {
            match event {
                ProjectEvent::Save => {
                    if let Some(name) = self.name.clone() {
                        self.save_as(&name);
                    }
                }
                ProjectEvent::SaveAs(name) => self.save_as(&name),
                ProjectEvent::New => {
                    self.replace_project(engine, Project::new(), None);
                    self.message = Some(tr!("projects-new"));
                }
                ProjectEvent::Open(name) => match self.store.load(&name) {
                    Ok(project) => {
                        self.replace_project(engine, project, Some(name.clone()));
                        self.message = Some(tr!("projects-opened", name = name.as_str()));
                    }
                    Err(e) => {
                        self.message = Some(tr!(
                            "projects-open-failed",
                            name = name.as_str(),
                            reason = e
                        ))
                    }
                },
                ProjectEvent::Delete(name) => {
                    match self.store.delete(&name) {
                        Ok(()) => {
                            if self.name.as_deref() == Some(name.as_str()) {
                                // The project stays open, but no longer has a saved copy.
                                self.name = None;
                                self.saved = None;
                                self.unsaved_since = Some(f64::NEG_INFINITY);
                            }
                            self.message = Some(tr!("projects-deleted", name = name.as_str()));
                        }
                        Err(e) => {
                            self.message = Some(tr!(
                                "projects-delete-failed",
                                name = name.as_str(),
                                reason = e
                            ))
                        }
                    }
                    self.saved_list = self.store.list();
                }
                ProjectEvent::OpenFile => self.pick(engine, PickKind::OpenProject),
                ProjectEvent::SaveFile => self.pick(engine, PickKind::SaveProject),
                ProjectEvent::Preset(name) => {
                    if let Some(project) = dm_project::preset_project(name) {
                        self.replace_project(engine, project, None);
                        self.message = Some(tr!("projects-preset-started", name = name));
                    }
                }
                ProjectEvent::ExportMidi => self.pick(engine, PickKind::ExportMidi),
                ProjectEvent::ExportWav => {
                    // A recording needs the bank.
                    if self.bank.soundfont().is_some() {
                        self.pick(engine, PickKind::ExportWav);
                    }
                }
                ProjectEvent::ExportStyle { kind, sections } => {
                    self.style_export = Some((kind, sections));
                    self.pick(engine, PickKind::ExportStyle);
                }
                ProjectEvent::SendStyle { kind, sections } => {
                    let name = self.export_name();
                    match kind.write(&self.project, &sections, &name) {
                        Ok(bytes) => {
                            // Playing would fight the transfer for the output.
                            if self.playing {
                                engine.stop();
                                self.playing = false;
                            }
                            self.midi.send_style(kind, kind.file_name(&name), bytes);
                            self.sending_style = true;
                            self.export_message = None;
                        }
                        Err(problem) => {
                            let reason = keyboard::problem_text(&problem);
                            self.message = Some(tr!("projects-style-send-failed", reason = reason));
                            self.export_message = self.message.clone();
                        }
                    }
                }
                ProjectEvent::CancelExport => {
                    if let Some(job) = &self.wav_job {
                        job.cancel();
                    }
                }
                ProjectEvent::Landscape(on) => {
                    if let Some(lock) = &self.orientation {
                        lock(on);
                    }
                    if let Err(e) = self.store.set_landscape(on) {
                        log::warn!("the landscape setting was not saved: {e}");
                    }
                }
                ProjectEvent::PasteTab(text, options) => self.paste_tab(engine, &text, options),
                ProjectEvent::Magenta(index) => {
                    let Some(groove) = self.magenta.grooves.get(index).cloned() else {
                        continue;
                    };
                    if let Ok((project, _)) =
                        dm_project::Project::from_imported(&groove.bars, groove.bpm)
                    {
                        self.stop_preview(engine);
                        self.replace_project(engine, project, None);
                        self.message =
                            Some(tr!("projects-preset-started", name = groove.name.as_str()));
                    }
                }
                ProjectEvent::BarLine(on) => {
                    self.bar_line = Some(on);
                    if let Err(e) = self.store.set_bar_strip(on) {
                        log::warn!("the bar strip setting was not saved: {e}");
                    }
                }
                ProjectEvent::RowLabels(labels) => {
                    self.row_labels = labels;
                    if let Err(e) = self.store.set_row_labels(labels) {
                        log::warn!("the row labels setting was not saved: {e}");
                    }
                }
            }
        }
    }

    /// Names a saved export for a message: a picked file by the name the picker reported; a path
    /// by its file name and the export folder's place when the host gave one, otherwise in full.
    fn saved_where(&self, target: &files::Target) -> String {
        let files::Target::Path(path) = target else {
            return target.name();
        };
        match (self.export_place.map(i18n::tr), path.file_name()) {
            (Some(place), Some(file)) => tr!(
                "projects-saved-where",
                file = file.to_string_lossy().into_owned(),
                place = place
            ),
            _ => path.display().to_string(),
        }
    }

    /// Asks where the file for `kind` is: through the host's picker, whose answer arrives on a
    /// later frame, or else the export folder or a desktop dialog, which answer at once. The new
    /// file is named after the project, or "My beat" in the current language.
    fn pick(&mut self, engine: &mut Engine, kind: PickKind) {
        let name = self.export_name();
        let path = match files::route(
            kind,
            self.picker.is_some(),
            self.export_dir.as_deref(),
            &name,
        ) {
            files::Route::Picker => {
                if let Some(picker) = &self.picker {
                    picker(files::request(self.pending.ask(kind), kind, &name));
                }
                return;
            }
            files::Route::Folder(path) => {
                if let Some(dir) = path.parent()
                    && let Err(e) = std::fs::create_dir_all(dir)
                {
                    log::warn!("could not create the export folder: {e}");
                    return;
                }
                path
            }
            files::Route::Dialog => match files::dialog(kind, &name) {
                Some(path) => path,
                None => return,
            },
        };
        self.carry_out(engine, kind, files::Target::Path(path));
    }

    /// Follows a style on its way to the keyboard, and says how it ended.
    fn style_transfer(&mut self, ctx: &egui::Context) {
        if !self.sending_style {
            return;
        }
        let text = match self.midi.transfer() {
            midi::TransferStatus::Idle | midi::TransferStatus::Running(_) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            midi::TransferStatus::Done(name) => tr!("projects-style-sent", name = name),
            midi::TransferStatus::Failed(error) => match error {
                midi::TransferError::NoAnswer | midi::TransferError::UnknownKeyboard(_) => {
                    tr!("projects-style-send-no-answer")
                }
                midi::TransferError::NotConnected => tr!("projects-style-send-not-connected"),
                midi::TransferError::NoInput => tr!("projects-style-send-no-input"),
                midi::TransferError::Refused(detail) => {
                    log::warn!("Send to keyboard: {detail}");
                    tr!("projects-style-send-refused")
                }
            },
        };
        self.sending_style = false;
        self.message = Some(text);
        self.export_message = self.message.clone();
    }

    /// The project's name, or "My beat" in the current language: the name of an exported file,
    /// and of a keyboard style.
    fn export_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| tr!("projects-default-name"))
    }

    /// Does what a pick was for, with the file it chose.
    fn carry_out(&mut self, engine: &mut Engine, kind: PickKind, target: files::Target) {
        match kind {
            PickKind::OpenProject => match store::read_file(&target) {
                Ok(project) => {
                    // Opened from outside the app: it gets a name, but no saved copy inside the
                    // app until it is saved.
                    let name = target.stem().and_then(|s| store::clean_name(&s));
                    self.replace_project(engine, project, name);
                    self.saved = None;
                    self.message = Some(tr!("projects-opened-file", path = target.name()));
                }
                Err(e) => {
                    self.message = Some(tr!(
                        "projects-open-file-failed",
                        path = target.name(),
                        reason = e
                    ))
                }
            },
            PickKind::SaveProject => {
                self.message = Some(match store::write_file(&target, &self.project) {
                    Ok(()) => tr!("projects-saved-file", path = target.name()),
                    Err(e) => tr!("projects-not-saved", reason = e),
                });
            }
            PickKind::ExportMidi => {
                self.message = Some(match export::write_midi(&target, &self.project) {
                    Ok(()) => tr!("projects-midi-saved", place = self.saved_where(&target)),
                    Err(e) => tr!("projects-midi-failed", reason = e.to_string()),
                });
                self.export_message = self.message.clone();
            }
            PickKind::ExportStyle => {
                let Some((kind, sections)) = self.style_export.take() else {
                    return;
                };
                let written = kind
                    .write(&self.project, &sections, &self.export_name())
                    .map_err(|problem| keyboard::problem_text(&problem))
                    .and_then(|bytes| target.write_all(&bytes).map_err(|e| e.to_string()));
                self.message = Some(match written {
                    Ok(()) => tr!("projects-style-saved", place = self.saved_where(&target)),
                    Err(reason) => tr!("projects-style-failed", reason = reason),
                });
                self.export_message = self.message.clone();
            }
            PickKind::OpenGrooves => {
                let name = target.file_name();
                match target.read_limited(grooves::MAX_FILE) {
                    Ok(bytes) => self.import_grooves(&name, bytes),
                    Err(e) => self.import_failed(tr!(
                        "grooves-read-failed",
                        name = name.as_str(),
                        reason = e.to_string()
                    )),
                }
            }
            PickKind::ExportWav => {
                let Some(soundfont) = self.bank.soundfont() else {
                    return;
                };
                if let Some(job) = &self.wav_job {
                    job.cancel();
                }
                self.wav_job = Some(export::WavJob::start(
                    soundfont,
                    &self.project,
                    target,
                    self.ctx.clone(),
                ));
            }
        }
    }

    fn save_as(&mut self, name: &str) {
        match self.store.save(name, &self.project) {
            Ok(()) => {
                self.name = Some(name.to_owned());
                self.saved = Some(self.project.clone());
                self.message = Some(tr!("projects-saved", name = name));
                // The autosave now names this project.
                self.unsaved_since = Some(f64::NEG_INFINITY);
                self.saved_list = self.store.list();
            }
            Err(e) => self.message = Some(tr!("projects-not-saved", reason = e)),
        }
    }

    /// Opens a different project: stops playback, forgets undo, and resyncs the engine.
    fn replace_project(&mut self, engine: &mut Engine, project: Project, name: Option<String>) {
        engine.stop();
        self.playing = false;
        self.saved = name.is_some().then(|| project.clone());
        self.project = project;
        self.name = name;
        self.history.clear();
        // Notes waiting on a drum on offer were played against the old project.
        self.offered.drop_notes();
        self.skip_record = true;
        self.current = 0;
        self.view_bar = 0;
        self.half = 0;
        self.resync(engine);
        self.unsaved_since = Some(f64::NEG_INFINITY);
    }

    /// Plays a row's drum once at the row's volume. Mute and solo do not apply: the user asked
    /// to hear it.
    fn preview_row(&self, engine: &mut Engine, row: usize) {
        if let Some(info) = self.project.rows().get(row) {
            hit_at(engine, info.key, info.volume);
        }
    }

    fn play(&mut self, engine: &mut Engine) {
        log::info!(
            "play: output latency {} frames at {} Hz",
            engine.output_latency_frames(),
            engine.sample_rate()
        );
        match self.mode {
            Mode::Pattern => {
                engine.play_pattern(self.project.patterns()[self.current].slot());
            }
            Mode::Song => {
                if self.project.song().is_empty() {
                    return;
                }
                engine.play_song(0);
            }
        }
        // Playing the project also stops a groove playing on its own.
        self.preview = None;
        self.playing = true;
        self.follow = true;
    }

    /// Adds the drum on offer with `key` as a row at the bottom, with the notes recorded on it.
    /// One undo step, as the frame records it. With the rows full, it says so and stays on offer.
    fn add_offered(&mut self, engine: &mut Engine, key: u8, now: f64) {
        match self.offered.add_to(&mut self.project, key) {
            Ok(changes) => apply(engine, changes),
            Err(dm_project::Refused::TooManyRows) => {
                let text = tr!("sounds-max-rows", count = dm_project::MAX_ROWS);
                self.notice = Some((text, now + NOTICE_SECONDS));
            }
            Err(refused) => log::info!("refused: {refused:?}"),
        }
    }

    /// Takes the hits the engine played from the MIDI input. Each lights its row's name, and a
    /// drum with no row is offered. While a pattern plays with the input on and connected, a
    /// take records: the hits heard in the pattern being edited become steps, or wait on a drum
    /// on offer. Stopped, or playing the song, nothing is recorded. The hits are taken every
    /// frame, so their queue never fills. Each take, from Play to Stop, undoes as one step.
    fn record_hits(&mut self, engine: &mut Engine, now: f64) {
        let hits = engine.take_heard_hits();
        let input = self.midi.input_connected();
        // A take runs from Play to Stop of a pattern. Anything that stops the pattern looping
        // ends it: Stop, a song that is played, a groove preview, opening a project, the input
        // turned off.
        let armed = input && self.playing && self.mode == Mode::Pattern;
        if !armed {
            self.recording = None;
        } else if self.recording.is_none() {
            self.takes += 1;
            self.recording = Some(self.takes);
        }
        if input && !self.playing {
            // The interface may otherwise draw only on input, and a key's flash must show at
            // once.
            self.ctx
                .request_repaint_after(std::time::Duration::from_secs_f64(INPUT_POLL));
        }
        if hits.is_empty() {
            return;
        }
        for hit in &hits {
            self.grid.flash(hit.key, now);
            if !self.project.rows().iter().any(|r| r.key == hit.key) {
                self.offered.offer(hit.key);
            }
        }
        self.ctx.request_repaint();
        let Some(take) = self.recording else {
            return;
        };
        let slot = self.project.patterns()[self.current].slot();
        let heard: Vec<_> = hits
            .into_iter()
            .filter_map(|hit| {
                let p = hit.position?;
                (p.song_index.is_none() && p.pattern == slot).then_some((hit, p.step))
            })
            .collect();
        if heard.is_empty() {
            return;
        }
        let before = self.project.clone();
        for (hit, step) in heard {
            match record::record_hit(
                &mut self.project,
                &mut self.offered,
                self.current,
                hit.key,
                hit.velocity,
                step,
            ) {
                Ok(changes) => apply(engine, changes),
                Err(refused) => log::info!("recording refused: {refused:?}"),
            }
        }
        if self.project != before {
            self.history.record(&before, Group::Recording(take), now);
            self.unsaved_since = Some(now);
        }
    }

    /// Plays the pattern being edited from the first step of `bar`, and follows it.
    fn play_from_bar(&mut self, engine: &mut Engine, bar: usize) {
        let pattern = &self.project.patterns()[self.current];
        if self.mode != Mode::Pattern || bar >= pattern.bars() {
            return;
        }
        engine.play_pattern_from(pattern.slot(), bar * STEPS_PER_BAR);
        self.preview = None;
        self.playing = true;
        self.follow = true;
        self.view_bar = bar;
        self.half = 0;
    }

    fn select_pattern(&mut self, engine: &mut Engine, index: usize) {
        self.current = index;
        self.view_bar = 0;
        self.half = 0;
        if self.playing && self.mode == Mode::Pattern {
            engine.cue_pattern(self.project.patterns()[index].slot());
        } else if self.playing {
            // In song mode, choosing a pattern to edit stops following the song.
            self.follow = false;
        }
    }

    /// Adds a pasted drum tab as a new pattern and shows it. If the tab cannot be read or added,
    /// the project stays as it was and the dialog says why, keeping the text to fix.
    fn paste_tab(&mut self, engine: &mut Engine, text: &str, options: dm_tab::Options) {
        let tab = match dm_tab::parse_with(text, options) {
            Ok(tab) => tab,
            Err(refusal) => {
                self.menu.refuse_paste(paste_tab::refusal(&refusal));
                return;
            }
        };
        match dm_tab::add_to_project(&mut self.project, &tab) {
            Ok((added, changes)) => {
                apply(engine, changes);
                self.select_pattern(engine, added.index);
                self.menu.close_paste();
                let pattern = self.project.patterns()[added.index].name();
                self.message = Some(paste_tab::added(&added, &tab, pattern));
            }
            Err(error) => self.menu.refuse_paste(paste_tab::add_error(&error)),
        }
    }

    /// Imports grooves from a file: a MIDI file or a zip of them, by its name (such as
    /// `Chilled Beat.zip`) and its whole contents. The grooves accepted are kept in the groove
    /// list as one pack, named after the file, which replaces a pack of the same name. Then the
    /// list opens on that pack, and its note and the menu's message say what was added and what
    /// was left out, and why. A file with no grooves adds no pack, and a notice says why.
    ///
    /// Every way a file arrives ends here: the menu's file item, a file dropped on a desktop
    /// window, and files a host hands in through [`DrumApp::incoming_files`], such as a share on
    /// Android or a download from a link. The groove list then opens on the new pack.
    pub(crate) fn import_grooves(&mut self, name: &str, bytes: Vec<u8>) {
        let result = if bytes.len() > grooves::MAX_FILE {
            Err(dm_groove::ImportError::TooLarge)
        } else {
            dm_groove::import(name, &bytes)
        };
        let import = match result {
            Ok(import) => import,
            Err(error) => return self.import_failed(grooves::import_error(&error, name)),
        };
        if import.grooves.is_empty() {
            return self.import_failed(grooves::nothing_added(&import, name));
        }
        let message = grooves::added(&import, name);
        let pack = dm_groove::Pack {
            name: Path::new(name)
                .file_stem()
                .map_or_else(|| name.to_owned(), |s| s.to_string_lossy().into_owned()),
            grooves: import.grooves,
        };
        let key = match self.store.save_pack(&pack) {
            Ok(key) => key,
            Err(e) => return self.import_failed(tr!("grooves-not-saved", reason = e)),
        };
        self.message = Some(message.clone());
        self.groove_note = Some(message);
        self.open_groove_list(Some(key));
    }

    /// Says why an import added nothing: in the menu's message, in the groove list's note or the
    /// link dialog if one is open, and otherwise in a notice over the grid.
    fn import_failed(&mut self, message: String) {
        self.message = Some(message.clone());
        if self.menu.grooves_open() {
            self.groove_note = Some(message);
        } else if self.menu.link_open() {
            self.menu.link_failed(message);
        } else {
            self.show_notice_text(message);
        }
    }

    /// Shows a notice over the grid for a few seconds.
    fn show_notice_text(&mut self, text: String) {
        let until = self.ctx.input(|i| i.time) + NOTICE_SECONDS;
        self.notice = Some((text, until));
        self.ctx.request_repaint();
    }

    /// Imports the files dropped on a desktop's window. Each file is read from its path.
    fn dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            let target = files::Target::Path(file.path().to_path_buf());
            let name = target.file_name();
            match target.read_limited(grooves::MAX_FILE) {
                Ok(bytes) => self.import_grooves(&name, bytes),
                Err(e) => self.import_failed(tr!(
                    "grooves-read-failed",
                    name = name.as_str(),
                    reason = e.to_string()
                )),
            }
        }
    }

    /// Reads the groove list again and opens it, showing the pack with key `focus` if given.
    fn open_groove_list(&mut self, focus: Option<String>) {
        self.reload_grooves();
        self.menu.open_grooves(focus);
    }

    /// Reads the groove list again, after an import or a delete.
    fn reload_grooves(&mut self) {
        self.groove_packs = self.store.load_packs();
    }

    /// The groove at `index` in the pack with `key`, if both are there.
    fn groove(&self, key: &str, index: usize) -> Option<&dm_groove::Groove> {
        if key == grooves::MAGENTA_KEY {
            return self.magenta.grooves.get(index);
        }
        self.groove_packs
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, pack)| pack.grooves.get(index))
    }

    /// Stops the groove playing on its own, if one is.
    fn stop_preview(&mut self, engine: &mut Engine) {
        if self.preview.take().is_some() {
            engine.stop_preview();
        }
    }

    fn handle_grooves(&mut self, engine: &mut Engine, event: grooves::GrooveEvent) {
        use grooves::GrooveEvent;
        match event {
            GrooveEvent::AddFromFile => self.pick(engine, PickKind::OpenGrooves),
            GrooveEvent::Download(link) => self.start_download(link),
            GrooveEvent::StopDownload => self.stop_download(),
            GrooveEvent::Open => {
                // The menu opened the list; the note was about an earlier visit.
                self.groove_note = None;
                self.stop_preview(engine);
                self.reload_grooves();
            }
            GrooveEvent::Play { pack, groove } => {
                let Some((hits, steps)) = self.groove(&pack, groove).map(grooves::preview_hits)
                else {
                    return;
                };
                // The preview stops the project's playback.
                engine.preview(hits, steps);
                self.playing = false;
                self.preview = Some((pack, groove));
            }
            GrooveEvent::Stop => self.stop_preview(engine),
            GrooveEvent::Add { pack, groove } => {
                let Some(groove) = self.groove(&pack, groove).cloned() else {
                    return;
                };
                match self.project.add_imported_pattern(&groove.bars) {
                    Ok((added, changes)) => {
                        apply(engine, changes);
                        self.stop_preview(engine);
                        self.select_pattern(engine, added.index);
                        self.menu.close_grooves();
                        let pattern = self.project.patterns()[added.index].name();
                        self.message = Some(grooves::added_pattern(&added, &groove, pattern));
                    }
                    Err(refused) => self.groove_note = Some(grooves::add_refused(refused)),
                }
            }
            GrooveEvent::DeletePack(key) => {
                if self.preview.as_ref().is_some_and(|(k, _)| *k == key) {
                    self.stop_preview(engine);
                }
                if let Err(e) = self.store.delete_pack(&key) {
                    self.groove_note = Some(tr!("grooves-delete-failed", reason = e));
                }
                self.reload_grooves();
            }
            GrooveEvent::DeleteGroove { pack, groove } => {
                if self.preview.as_ref().is_some_and(|(k, _)| *k == pack) {
                    self.stop_preview(engine);
                }
                if let Err(e) = self.store.delete_groove(&pack, groove) {
                    self.groove_note = Some(tr!("grooves-delete-failed", reason = e));
                }
                self.reload_grooves();
            }
        }
    }

    /// Text a host handed in, such as a link shared to the app. A link opens the link dialog and
    /// downloads at once, so a share needs no further tap. Other text goes to the Paste a drum
    /// tab dialog. Anything else is refused with a notice.
    fn incoming_text(&mut self, text: &str) {
        match link::shared_text(text) {
            link::SharedText::Download(address) => {
                self.menu.open_link(address.clone());
                self.start_download(address);
            }
            link::SharedText::PasteTab => self.menu.open_paste_tab(text.to_owned()),
            link::SharedText::Refuse => {
                let message = tr!("grooves-link-share-refused");
                self.message = Some(message.clone());
                self.show_notice_text(message);
            }
        }
    }

    /// Starts downloading `link` on a worker thread, stopping any download before it. The link
    /// dialog shows its progress; [`DrumApp::poll_link`] takes the result.
    fn start_download(&mut self, link: String) {
        self.stop_download();
        self.link_job = Some(link::LinkJob::start(link, self.ctx.clone()));
    }

    /// Stops the download, quietly. The worker ends at its next progress report, and its result
    /// is not waited for.
    fn stop_download(&mut self) {
        if let Some(job) = self.link_job.take() {
            job.cancel();
        }
    }

    /// Takes the result of the download from a link, once it has finished. Closing the link
    /// dialog stops the download. A file goes to the import, which opens the groove list on it.
    /// A failure stays in the dialog, with the link, to be fixed or tried again.
    fn poll_link(&mut self) {
        if self.link_job.is_some() && !self.menu.link_open() {
            self.stop_download();
        }
        let Some(result) = self.link_job.as_ref().and_then(link::LinkJob::poll) else {
            return;
        };
        self.link_job = None;
        match result {
            // The groove list replaces the link dialog, or the dialog shows why the file was not
            // taken.
            Ok((name, bytes)) => self.import_grooves(&name, bytes),
            Err(error) => self.import_failed(link::error_text(&error)),
        }
    }

    fn finish(&mut self, engine: &mut Engine, result: Result<Vec<Change>, dm_project::Refused>) {
        match result {
            Ok(changes) => apply(engine, changes),
            Err(refused) => log::info!("refused: {refused:?}"),
        }
    }
}

/// Tells the engine about project changes.
fn apply(engine: &mut Engine, changes: Vec<Change>) {
    for change in changes {
        match change {
            Change::Step {
                slot,
                row,
                step,
                velocity,
            } => engine.set_step(slot, row, step, velocity),
            Change::Length { slot, steps } => engine.set_length(slot, steps),
            Change::Song(order) => engine.set_song(&order),
            Change::RowKey { row, key } => engine.set_row_key(row, key),
            Change::RowGain { row, percent } => {
                engine.set_row_gain(row, f32::from(percent) / 100.0)
            }
            Change::ClearSlot { slot } => engine.clear_pattern(slot),
            Change::Kit(program) => engine.set_kit(program),
            Change::Swing(thousandths) => engine.set_swing(f32::from(thousandths) / 1000.0),
        }
    }
}

/// Plays a drum once at a volume in percent; nothing at 0.
fn hit_at(engine: &mut Engine, key: u8, percent: u8) {
    let velocity = scaled(DEFAULT_VELOCITY, percent);
    if velocity > 0 {
        engine.hit(key, velocity);
    }
}

/// A velocity scaled by a volume in percent, at least 1 unless the volume is 0.
fn scaled(velocity: u8, percent: u8) -> u8 {
    if percent == 0 {
        return 0;
    }
    ((u32::from(velocity) * u32::from(percent)).div_ceil(100)).clamp(1, 127) as u8
}

/// A button that shows whether it is on. Unlike egui's selectable button it always draws its
/// frame, so it looks tappable when off.
pub(crate) fn toggle<'a>(selected: bool, text: impl Into<egui::WidgetText>) -> egui::Button<'a> {
    egui::Button::new(text).selected(selected)
}

/// Whether the bar thumbnails show under the strip before the person chooses: on a desktop,
/// and on a tablet, whose short side is at least 600 points. A phone keeps the room for the grid.
fn bar_line_default(ctx: &egui::Context) -> bool {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        let screen = ctx.content_rect();
        screen.width().min(screen.height()) >= 600.0
    } else {
        true
    }
}
