//! The menu under ☰, and the dialogs its items open: Save as, Open, Load default style, Export,
//! Language, MIDI output, Paste a drum tab, Add grooves from a link, Grooves, and the tempo when
//! the strip has no room for it. Its Sounds item opens the Sounds dialog. See "One control strip" in `docs/DESIGN.md`.
//!
//! It does everything the old Projects screen did. Starting a new project, opening one, opening
//! a file or starting from a beat asks first when there are unsaved changes, and deleting always
//! asks. Each question asks in a small window of its own, over the dialog it came from.

use egui::{RichText, Vec2};

use crate::grooves::{self, GrooveEvent, GroovesView};
use crate::i18n::{self, Language};
use crate::icons;
use crate::link;
use crate::pictures::RowLabels;
use crate::projects::{ProjectEvent, ProjectsView};
use crate::strip::Hidden;
use crate::tempo::{self, TempoEvent};
use crate::widgets::{self, TAP, TEXT_SIZE};

pub(crate) enum MenuEvent {
    /// Save, open, start, export, and the rest the old Projects screen did.
    Project(ProjectEvent),
    /// The Open or Save as dialog has just opened: read the list of saved projects again.
    ListSaved,
    Undo,
    Redo,
    /// Open the Sounds dialog.
    Sounds,
    Tempo(TempoEvent),
    /// Show text in this language, or in the system's if `None`.
    Language(Option<Language>),
    /// The MIDI output dialog has just opened: read the list of outputs again.
    ListMidiOutputs,
    /// Send the drums to this MIDI output, or play them on this device if `None`.
    MidiOutput(Option<String>),
    /// Import grooves, or use the groove list.
    Grooves(GrooveEvent),
}

/// What the MIDI output item and its dialog show.
pub(crate) struct MidiView<'a> {
    /// The chosen output's name, or `None`.
    pub(crate) choice: Option<&'a str>,
    /// Whether the drums go to the chosen output now.
    pub(crate) connected: bool,
    /// The outputs listed when the dialog opened.
    pub(crate) outputs: &'a [String],
}

/// What the menu and its dialogs need to know to draw themselves.
pub(crate) struct MenuView<'a> {
    pub(crate) projects: ProjectsView<'a>,
    /// The strip buttons that did not fit, which the menu offers instead.
    pub(crate) hidden: Hidden,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    pub(crate) bpm: f32,
    /// The swing amount, 0.0 to 0.5 of a step.
    pub(crate) swing: f32,
    /// The language the person chose, or `None` while text follows the system.
    pub(crate) language: Option<Language>,
    /// The language text follows when the person chose none.
    pub(crate) system_language: Language,
    /// The MIDI output setting, or `None` where the platform cannot send MIDI.
    pub(crate) midi: Option<MidiView<'a>>,
    pub(crate) grooves: GroovesView<'a>,
    /// How far a download from a link has got, while one runs.
    pub(crate) downloading: Option<link::Progress>,
}

/// The dialog a menu item opened.
enum Shown {
    /// The name being typed.
    SaveAs(String),
    Open,
    Presets,
    Export,
    Tempo,
    Language,
    MidiOutput,
    /// The tab being pasted, and why the last try was refused, if it was.
    PasteTab {
        text: String,
        refusal: Option<String>,
    },
    /// The link being typed, why the last download failed, if it did, and whether the box still
    /// has to ask for focus once.
    AddLink {
        text: String,
        error: Option<String>,
        focus: bool,
    },
    /// The groove list, and a pack to open and show once, such as the one just imported.
    Grooves {
        focus: Option<String>,
    },
    /// "Start new?" when there are unsaved changes.
    New,
}

/// An "are you sure" question waiting for an answer, over the open dialog.
#[derive(Clone, PartialEq)]
enum Ask {
    Open(String),
    OpenFile,
    Delete(String),
    Preset(&'static str),
    Groove(grooves::Ask),
}

#[derive(Default)]
pub(crate) struct Menu {
    dialog: Option<Shown>,
    ask: Option<Ask>,
    /// How the Paste a drum tab dialog reads a tab. Kept while the app runs, so a second paste
    /// reads like the first.
    tab_options: dm_tab::Options,
    /// Which of the pop-up's sections are open. They start closed and stay as left while the
    /// app runs.
    import_section: bool,
    settings_section: bool,
}

/// The width of a menu item.
const ITEM_WIDTH: f32 = 240.0;

fn item<'a>(icon: &str, word: &str) -> egui::Button<'a> {
    widgets::button(icon, word).min_size(Vec2::new(ITEM_WIDTH, TAP))
}

/// A heading that opens and closes a section of the pop-up in place, with a caret that points
/// down while it is open. A section keeps the pop-up one list, which works the same with a
/// finger and a mouse, where a submenu beside it would not.
fn section(ui: &mut egui::Ui, open: &mut bool, title: &str) {
    let caret = if *open {
        icons::CARET_DOWN
    } else {
        icons::CARET_RIGHT
    };
    if ui
        .add(item(caret, title).frame(false))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        *open = !*open;
    }
}

fn big(text: &str) -> RichText {
    RichText::new(text).size(TEXT_SIZE)
}

/// A plain button with a word, at least [`TAP`] high, for the dialogs.
fn word_button<'a>(text: &str) -> egui::Button<'a> {
    egui::Button::new(big(text)).min_size(Vec2::new(TAP, TAP))
}

impl Menu {
    /// Opens the Save as dialog, for Ctrl+S on a project with no name yet.
    pub(crate) fn open_save_as(&mut self) {
        self.dialog = Some(Shown::SaveAs(String::new()));
        self.ask = None;
    }

    /// Whether one of the menu's dialogs is open.
    pub(crate) fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// Shows why a pasted tab was refused, in the Paste a drum tab dialog, which stays open with
    /// the text so it can be fixed.
    pub(crate) fn refuse_paste(&mut self, message: String) {
        if let Some(Shown::PasteTab { refusal, .. }) = &mut self.dialog {
            *refusal = Some(message);
        }
    }

    /// Closes the Paste a drum tab dialog once its tab has been added.
    pub(crate) fn close_paste(&mut self) {
        if matches!(self.dialog, Some(Shown::PasteTab { .. })) {
            self.dialog = None;
            self.ask = None;
        }
    }

    /// Opens the Paste a drum tab dialog with `text` in its box, such as text shared to the app.
    /// Nothing is added until the person taps "Add as pattern".
    pub(crate) fn open_paste_tab(&mut self, text: String) {
        self.open(Shown::PasteTab {
            text,
            refusal: None,
        });
    }

    /// Opens the Add grooves from a link dialog with `link` in its box, such as a link shared to
    /// the app, whose download the app starts at once. The box does not ask for focus, which on
    /// a phone would bring up the keyboard over the progress.
    pub(crate) fn open_link(&mut self, link: String) {
        self.open(Shown::AddLink {
            text: link,
            error: None,
            focus: false,
        });
    }

    /// Whether the Add grooves from a link dialog is open. A download runs only while it is.
    pub(crate) fn link_open(&self) -> bool {
        matches!(self.dialog, Some(Shown::AddLink { .. }))
    }

    /// Shows why a download or its import failed, in the link dialog, which stays open with the
    /// link so it can be fixed or tried again.
    pub(crate) fn link_failed(&mut self, message: String) {
        if let Some(Shown::AddLink { error, .. }) = &mut self.dialog {
            *error = Some(message);
        }
    }

    /// Closes the link dialog, once its download has finished.
    pub(crate) fn close_link(&mut self) {
        if self.link_open() {
            self.dialog = None;
            self.ask = None;
        }
    }

    /// Opens the groove list, showing the pack with key `focus` if there is one.
    pub(crate) fn open_grooves(&mut self, focus: Option<String>) {
        self.open(Shown::Grooves { focus });
    }

    /// Whether the groove list is open.
    pub(crate) fn grooves_open(&self) -> bool {
        matches!(self.dialog, Some(Shown::Grooves { .. }))
    }

    /// Closes the groove list, once a groove from it has been added.
    pub(crate) fn close_grooves(&mut self) {
        if self.grooves_open() {
            self.dialog = None;
            self.ask = None;
        }
    }

    /// Opens a dialog, dropping any question left from before.
    fn open(&mut self, dialog: Shown) {
        self.dialog = Some(dialog);
        self.ask = None;
    }

    /// Draws the pop-up under ☰. Items that open a dialog close the pop-up.
    pub(crate) fn contents(&mut self, ui: &mut egui::Ui, view: &MenuView) -> Vec<MenuEvent> {
        let mut events = Vec::new();
        let p = &view.projects;
        ui.set_min_width(ITEM_WIDTH);
        ui.set_max_width(ITEM_WIDTH + 40.0);
        let hidden = view.hidden;

        // The strip buttons that did not fit come first, on one line and as icons, as in the
        // strip: Undo, Redo and the tempo are the items most often tapped in a hurry, and at the top they stay put whatever the
        // status below says. Undo and Redo keep the menu open, so they can be tapped again.
        let extras = [hidden.undo, hidden.redo, hidden.tempo]
            .iter()
            .filter(|&&h| h)
            .count();
        if extras > 0 {
            ui.horizontal_wrapped(|ui| {
                let gaps = ui.spacing().item_spacing.x * (extras - 1) as f32;
                let size = Vec2::new((ITEM_WIDTH - gaps) / extras as f32, TAP);
                if hidden.undo
                    && ui
                        .add_enabled(
                            view.can_undo,
                            widgets::button(icons::UNDO, "").min_size(size),
                        )
                        .on_hover_text(tr!("menu-undo"))
                        .clicked()
                {
                    events.push(MenuEvent::Undo);
                }
                if hidden.redo
                    && ui
                        .add_enabled(
                            view.can_redo,
                            widgets::button(icons::REDO, "").min_size(size),
                        )
                        .on_hover_text(tr!("menu-redo"))
                        .clicked()
                {
                    events.push(MenuEvent::Redo);
                }
                if hidden.tempo
                    && ui
                        .add(
                            widgets::button(icons::METRONOME, &format!("{:.0}", view.bpm))
                                .min_size(size),
                        )
                        .on_hover_text(tr!("menu-tempo-tip"))
                        .clicked()
                {
                    self.open(Shown::Tempo);
                    ui.close();
                }
            });
            ui.separator();
        }

        // A short status: the name, unsaved changes, and the last message.
        // A project's name is what the person typed, so it is shown as it is.
        let title = p.name.map_or_else(|| tr!("menu-no-name"), str::to_owned);
        ui.label(big(&title).strong());
        if p.unsaved {
            ui.label(tr!("menu-unsaved"));
        }
        if let Some(message) = p.message {
            ui.label(RichText::new(message).color(ui.visuals().warn_fg_color));
        }
        ui.separator();

        match p.name {
            // Saving under the same name stays in the menu, so the message shows it worked.
            Some(_) => {
                if ui
                    .add_enabled(p.unsaved, item(icons::FLOPPY_DISK, &tr!("menu-save")))
                    .clicked()
                {
                    events.push(MenuEvent::Project(ProjectEvent::Save));
                }
                if ui.add(item(icons::COPY, &tr!("menu-save-as"))).clicked() {
                    self.open(Shown::SaveAs(String::new()));
                    events.push(MenuEvent::ListSaved);
                    ui.close();
                }
            }
            None => {
                if ui
                    .add(item(icons::FLOPPY_DISK, &tr!("menu-save")))
                    .clicked()
                {
                    self.open(Shown::SaveAs(String::new()));
                    events.push(MenuEvent::ListSaved);
                    ui.close();
                }
            }
        }
        if ui
            .add(item(icons::FOLDER_OPEN, &tr!("menu-open")))
            .clicked()
        {
            self.open(Shown::Open);
            events.push(MenuEvent::ListSaved);
            ui.close();
        }
        if ui.add(item(icons::FILE_PLUS, &tr!("menu-new"))).clicked() {
            if p.unsaved {
                self.open(Shown::New);
            } else {
                events.push(MenuEvent::Project(ProjectEvent::New));
            }
            ui.close();
        }
        if ui.add(item(icons::SPARKLE, &tr!("menu-presets"))).clicked() {
            self.open(Shown::Presets);
            ui.close();
        }
        if ui
            .add(item(icons::SPEAKER_HIGH, &tr!("menu-sounds")))
            .clicked()
        {
            events.push(MenuEvent::Sounds);
            ui.close();
        }
        if ui.add(item(icons::EXPORT, &tr!("menu-export"))).clicked() {
            self.open(Shown::Export);
            ui.close();
        }
        // Grooves and tabs become new patterns, so basic mode, with its one pattern, has no use
        // for them and no section.
        if !p.basic {
            ui.separator();
            section(ui, &mut self.import_section, &tr!("menu-import"));
            if self.import_section {
                self.import_items(ui, view, &mut events);
            }
        }
        // The settings belong to the device, not to the beat.
        ui.separator();
        section(ui, &mut self.settings_section, &tr!("menu-settings"));
        if self.settings_section {
            settings_items(ui, view, &mut events, |dialog| self.open(dialog));
        }
        events
    }

    /// The Import section: adding grooves from a file or a link, pasting a drum tab, and the groove
    /// list.
    fn import_items(&mut self, ui: &mut egui::Ui, view: &MenuView, events: &mut Vec<MenuEvent>) {
        let p = &view.projects;
        // A file can be chosen on a desktop and where the host has a picker.
        if p.file_items
            && ui
                .add(item(icons::FILE_ARROW_UP, &tr!("grooves-add-from-file")))
                .clicked()
        {
            events.push(MenuEvent::Grooves(GrooveEvent::AddFromFile));
            ui.close();
        }
        // A link needs no file picker, so it is offered everywhere.
        if ui
            .add(item(icons::LINK, &tr!("grooves-add-from-link")))
            .clicked()
        {
            self.open(Shown::AddLink {
                text: String::new(),
                error: None,
                focus: true,
            });
            ui.close();
        }
        if ui
            .add(item(icons::MUSIC_NOTES_PLUS, &tr!("menu-paste-tab")))
            .clicked()
        {
            self.open(Shown::PasteTab {
                text: String::new(),
                refusal: None,
            });
            ui.close();
        }
        if view.grooves.any
            && ui
                .add(item(icons::PLAYLIST, &tr!("grooves-title")))
                .clicked()
        {
            self.open(Shown::Grooves { focus: None });
            events.push(MenuEvent::Grooves(GrooveEvent::Open));
            ui.close();
        }
    }
}

/// The Settings section. `open` opens a dialog. Basic mode always comes last.
fn settings_items(
    ui: &mut egui::Ui,
    view: &MenuView,
    events: &mut Vec<MenuEvent>,
    mut open: impl FnMut(Shown),
) {
    let p = &view.projects;
    // How rows are labelled: each tap moves to the next choice, and the item says which
    // one is on. It stays open, so the grid behind can be seen to change.
    let labels = match p.row_labels {
        RowLabels::Names => tr!("menu-rows-names"),
        RowLabels::PicturesAndNames => tr!("menu-rows-pictures-and-names"),
        RowLabels::Pictures => tr!("menu-rows-pictures"),
    };
    if ui
        .add(
            widgets::toggle_button(p.row_labels.shows_pictures(), icons::IMAGE, &labels)
                .min_size(Vec2::new(ITEM_WIDTH, TAP)),
        )
        .clicked()
    {
        events.push(MenuEvent::Project(ProjectEvent::RowLabels(
            p.row_labels.next(),
        )));
    }
    if let Some(landscape) = p.landscape
        && ui
            .add(
                widgets::toggle_button(landscape, icons::DEVICE_ROTATE, &tr!("menu-landscape"))
                    .min_size(Vec2::new(ITEM_WIDTH, TAP)),
            )
            .clicked()
    {
        events.push(MenuEvent::Project(ProjectEvent::Landscape(!landscape)));
    }
    if ui.add(item(icons::GLOBE, &tr!("menu-language"))).clicked() {
        open(Shown::Language);
        ui.close();
    }
    // The item names the output chosen, shortened to fit.
    if let Some(midi) = &view.midi {
        let device = midi.choice.map_or_else(
            || tr!("menu-midi-output-none"),
            |name| crate::midi::shorten(name, crate::midi::SHORT_NAME),
        );
        let text = tr!("menu-midi-output-choice", device = device);
        let mut response = ui.add(item(icons::MUSIC_NOTES, &text));
        if let Some(name) = midi.choice
            && name.chars().count() > crate::midi::SHORT_NAME
        {
            response = response.on_hover_text(name);
        }
        if response.clicked() {
            open(Shown::MidiOutput);
            events.push(MenuEvent::ListMidiOutputs);
            ui.close();
        }
    }
    // Basic mode always comes last.
    if ui
        .add(
            widgets::toggle_button(p.basic, icons::SLIDERS_HORIZONTAL, &tr!("menu-basic"))
                .min_size(Vec2::new(ITEM_WIDTH, TAP)),
        )
        .clicked()
    {
        events.push(MenuEvent::Project(ProjectEvent::Basic(!p.basic)));
    }
}

impl Menu {
    /// Shows the dialog a menu item opened, if any. Call it every frame.
    pub(crate) fn dialogs(&mut self, ctx: &egui::Context, view: &MenuView) -> Vec<MenuEvent> {
        let mut events = Vec::new();
        // Taken out while drawn, and put back unless it closes.
        let Some(mut dialog) = self.dialog.take() else {
            return events;
        };
        let p = &view.projects;
        // Set by a choice that needs nothing more.
        let mut done = false;
        let closed = match &mut dialog {
            Shown::SaveAs(text) => {
                let title = if p.name.is_some() {
                    tr!("menu-save-as")
                } else {
                    tr!("menu-save")
                };
                widgets::dialog_at_top(ctx, "save-as", &title, |ui| {
                    done = save_as(ui, text, p.saved, &mut events);
                })
                .closed
            }
            Shown::Open => {
                widgets::dialog(ctx, "open", &tr!("menu-open"), |ui| {
                    done = self.open_list(ui, p, &mut events);
                })
                .closed
            }
            Shown::Presets => {
                widgets::dialog(ctx, "presets", &tr!("menu-presets"), |ui| {
                    done = self.presets(ui, p.unsaved, &mut events);
                })
                .closed
            }
            Shown::Export => {
                widgets::dialog(ctx, "export", &tr!("menu-export"), |ui| {
                    export(ui, p, &mut events)
                })
                .closed
            }
            Shown::Tempo => {
                widgets::dialog(ctx, "tempo", &tr!("menu-tempo"), |ui| {
                    let changes = tempo::contents(ui, view.bpm, view.swing);
                    events.extend(changes.into_iter().map(MenuEvent::Tempo));
                })
                .closed
            }
            Shown::Language => {
                widgets::dialog(ctx, "language", &tr!("menu-language"), |ui| {
                    done = language(ui, view, &mut events);
                })
                .closed
            }
            Shown::MidiOutput => {
                widgets::dialog(ctx, "midi-output", &tr!("menu-midi-output"), |ui| {
                    done = view
                        .midi
                        .as_ref()
                        .is_none_or(|midi| midi_output(ui, midi, &mut events));
                })
                .closed
            }
            Shown::PasteTab { text, refusal } => {
                let options = &mut self.tab_options;
                widgets::dialog_at_top(ctx, "paste-tab", &tr!("menu-paste-tab"), |ui| {
                    done = paste_tab(ui, text, refusal, options, &mut events);
                })
                .closed
            }
            Shown::AddLink { text, error, focus } => {
                widgets::dialog_at_top(ctx, "add-link", &tr!("grooves-add-from-link"), |ui| {
                    done = add_link(ui, text, error, focus, view.downloading, &mut events);
                })
                .closed
            }
            Shown::Grooves { focus } => {
                let mut ask = None;
                let mut list_events = Vec::new();
                let closed = widgets::dialog(ctx, "grooves", &tr!("grooves-title"), |ui| {
                    grooves::list(ui, &view.grooves, focus, &mut ask, &mut list_events);
                })
                .closed;
                if let Some(ask) = ask {
                    self.ask = Some(Ask::Groove(ask));
                }
                events.extend(list_events.into_iter().map(MenuEvent::Grooves));
                closed
            }
            Shown::New => match widgets::confirm(
                ctx,
                "new",
                &tr!("menu-new-question"),
                &tr!("menu-start-new"),
                &tr!("menu-keep-this"),
            ) {
                Some(true) => {
                    events.push(MenuEvent::Project(ProjectEvent::New));
                    true
                }
                Some(false) => true,
                None => false,
            },
        };
        // A question a dialog asked, in its own window over the dialog.
        if let Some(ask) = self.ask.clone() {
            let (question, yes, no) = match &ask {
                Ask::Open(name) => (
                    tr!("menu-open-question", name = name.as_str()),
                    tr!("menu-open"),
                    tr!("menu-keep-this"),
                ),
                Ask::OpenFile => (
                    tr!("menu-open-file-question"),
                    tr!("menu-choose-file"),
                    tr!("menu-keep-this"),
                ),
                Ask::Delete(name) => (
                    tr!("menu-delete-question", name = name.as_str()),
                    tr!("menu-delete"),
                    tr!("menu-keep"),
                ),
                // Style names such as Rock are not translated.
                Ask::Preset(name) => (
                    tr!("menu-preset-question", name = *name),
                    tr!("menu-start"),
                    tr!("menu-keep-this"),
                ),
                Ask::Groove(ask) => ask.words(),
            };
            match widgets::confirm(ctx, "menu-question", &question, &yes, &no) {
                Some(true) => {
                    self.ask = None;
                    match ask {
                        Ask::Open(name) => {
                            events.push(MenuEvent::Project(ProjectEvent::Open(name)));
                            done = true;
                        }
                        Ask::OpenFile => {
                            events.push(MenuEvent::Project(ProjectEvent::OpenFile));
                            done = true;
                        }
                        // The list stays open, to delete another or open one.
                        Ask::Delete(name) => {
                            events.push(MenuEvent::Project(ProjectEvent::Delete(name)));
                        }
                        Ask::Preset(name) => {
                            events.push(MenuEvent::Project(ProjectEvent::Preset(name)));
                            done = true;
                        }
                        // The list stays open, to play, add or delete another.
                        Ask::Groove(ask) => events.push(MenuEvent::Grooves(ask.event())),
                    }
                }
                Some(false) => self.ask = None,
                None => {}
            }
        }
        if closed || done {
            self.ask = None;
        } else {
            self.dialog = Some(dialog);
        }
        events
    }

    /// The Open dialog: the saved projects, each with Delete, and, where the platform can,
    /// "Open a file" and "Save to a file". Returns true when a project was opened or a file
    /// dialog started.
    fn open_list(
        &mut self,
        ui: &mut egui::Ui,
        p: &ProjectsView,
        events: &mut Vec<MenuEvent>,
    ) -> bool {
        let mut done = false;
        if p.file_items {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(widgets::button(
                        icons::FILE_ARROW_UP,
                        &tr!("menu-open-file"),
                    ))
                    .clicked()
                {
                    if p.unsaved {
                        self.ask = Some(Ask::OpenFile);
                    } else {
                        events.push(MenuEvent::Project(ProjectEvent::OpenFile));
                        done = true;
                    }
                }
                if ui
                    .add(widgets::button(icons::FLOPPY_DISK, &tr!("menu-save-file")))
                    .clicked()
                {
                    events.push(MenuEvent::Project(ProjectEvent::SaveFile));
                    done = true;
                }
            });
            ui.separator();
        }

        if p.saved.is_empty() {
            ui.label(big(&tr!("menu-nothing-saved")));
        }
        // Each saved beat: its name, to open it, and Delete.
        for name in p.saved {
            ui.horizontal_wrapped(|ui| {
                let current = p.name == Some(name.as_str());
                if ui
                    .add(crate::toggle(current, big(name)).min_size(Vec2::new(180.0, TAP)))
                    .clicked()
                    && !current
                {
                    if p.unsaved {
                        self.ask = Some(Ask::Open(name.clone()));
                    } else {
                        events.push(MenuEvent::Project(ProjectEvent::Open(name.clone())));
                        done = true;
                    }
                }
                if ui
                    .add(widgets::button(icons::TRASH, &tr!("menu-delete")))
                    .clicked()
                {
                    self.ask = Some(Ask::Delete(name.clone()));
                }
            });
        }
        done
    }

    /// The preset beats. Returns true when one was chosen.
    fn presets(&mut self, ui: &mut egui::Ui, unsaved: bool, events: &mut Vec<MenuEvent>) -> bool {
        let mut done = false;
        ui.label(tr!("menu-presets-intro"));
        // Each beat: its button, then its description on a line of its own, so a long
        // description does not wrap around the button.
        // The names are names of styles, such as Rock or Bossa nova, which Portuguese uses as
        // they are, so they are not translated. Their descriptions are.
        for preset in dm_project::presets() {
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new(big(preset.name)).min_size(Vec2::new(140.0, TAP)))
                .clicked()
            {
                if unsaved {
                    self.ask = Some(Ask::Preset(preset.name));
                } else {
                    events.push(MenuEvent::Project(ProjectEvent::Preset(preset.name)));
                    done = true;
                }
            }
            ui.label(preset_description(preset));
        }
        done
    }
}

/// The Save as dialog: a name box. Returns true when the project was saved or the box
/// cancelled.
fn save_as(
    ui: &mut egui::Ui,
    text: &mut String,
    saved: &[String],
    events: &mut Vec<MenuEvent>,
) -> bool {
    let mut done = false;
    ui.label(big(&tr!("menu-name")));
    let edit = ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(tr!("projects-default-name"))
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Heading),
    );
    // Focus the box when the dialog opens, while nothing else has focus. Asking every frame
    // would also interrupt the input method each frame, which on Android closes and reopens the
    // on-screen keyboard over and over.
    if !edit.has_focus() && ui.memory(|m| m.focused().is_none()) {
        edit.request_focus();
    }
    let typed = crate::store::clean_name(text);
    // Enter saves, whether or not the box still has focus: on Android the keyboard's Enter
    // comes as a press and a release in one frame, and the box reports losing focus only on a
    // later one. The box is the only text box in the dialog.
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    if let Some(name) = &typed
        && saved.contains(name)
    {
        ui.label(tr!("menu-replaces", name = name.as_str()));
    }
    ui.horizontal_wrapped(|ui| {
        let save = ui
            .add_enabled(
                typed.is_some(),
                widgets::button(icons::FLOPPY_DISK, &tr!("menu-save")),
            )
            .clicked();
        if (save || enter)
            && let Some(name) = typed.clone()
        {
            events.push(MenuEvent::Project(ProjectEvent::SaveAs(name)));
            done = true;
        }
        if ui.add(word_button(&tr!("menu-cancel"))).clicked() {
            done = true;
        }
    });
    done
}

/// The Paste a drum tab dialog: a box for the tab, a choice to leave out bars that do not fit,
/// and why the last try was refused. "Add as pattern" sends the text and leaves the dialog open:
/// the app closes it once the pattern is added, or shows why not. Returns true when cancelled.
fn paste_tab(
    ui: &mut egui::Ui,
    text: &mut String,
    refusal: &mut Option<String>,
    options: &mut dm_tab::Options,
    events: &mut Vec<MenuEvent>,
) -> bool {
    let mut done = false;
    ui.label(tr!("menu-paste-tab-hint"));
    // The box is not focused when the dialog opens. A tap on it brings up a phone's keyboard,
    // with its paste button, and nothing then asks for focus again, which on Android would
    // close and reopen the keyboard.
    let edit = egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(text)
                    .code_editor()
                    .desired_rows(8)
                    .desired_width(f32::INFINITY),
            )
        })
        .inner;
    // Off by default: leaving bars out changes the music, so the person asks for it.
    let skip = ui.checkbox(
        &mut options.skip_odd_bars,
        big(&tr!("menu-paste-tab-skip-odd-bars")),
    );
    // A refusal is about the text and the choice as they were, so it goes once either changes.
    if edit.changed() || skip.changed() {
        *refusal = None;
    }
    if let Some(message) = refusal.as_deref() {
        ui.label(big(message).color(ui.visuals().warn_fg_color));
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                !text.trim().is_empty(),
                widgets::button(icons::MUSIC_NOTES_PLUS, &tr!("menu-paste-tab-add")),
            )
            .clicked()
        {
            events.push(MenuEvent::Project(ProjectEvent::PasteTab(
                text.clone(),
                *options,
            )));
        }
        if ui.add(word_button(&tr!("menu-cancel"))).clicked() {
            done = true;
        }
    });
    done
}

/// The Add grooves from a link dialog: a box for the link, and Add. While the download runs, the
/// box is disabled and its progress shows, with Stop. Why the last try failed shows below the
/// box. Add sends the link and leaves the dialog open: the app closes it once the grooves are in,
/// or shows why not. Returns true when cancelled.
fn add_link(
    ui: &mut egui::Ui,
    text: &mut String,
    error: &mut Option<String>,
    focus: &mut bool,
    downloading: Option<link::Progress>,
    events: &mut Vec<MenuEvent>,
) -> bool {
    let mut done = false;
    ui.label(tr!("grooves-link-hint"));
    let edit = ui.add_enabled(
        downloading.is_none(),
        egui::TextEdit::singleline(text)
            .hint_text("https://")
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Heading),
    );
    // Focus the box once, when the dialog opens from the menu. Asking every frame would also
    // interrupt the input method each frame, which on Android closes and reopens the on-screen
    // keyboard over and over.
    if *focus {
        edit.request_focus();
        *focus = false;
    }
    // An error is about the link as it was, so it goes once the link changes.
    if edit.changed() {
        *error = None;
    }
    let link = dm_groove::link_address(text);
    // Enter adds, as it saves in the Save as dialog, and for the same reason.
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    match downloading {
        Some((so_far, total)) => {
            ui.label(big(&link::progress_text(so_far)));
            ui.horizontal_wrapped(|ui| {
                match total.filter(|&t| t > 0) {
                    Some(total) => {
                        let fraction = (so_far as f64 / total as f64).min(1.0) as f32;
                        ui.add(
                            egui::ProgressBar::new(fraction)
                                .show_percentage()
                                .desired_width(200.0),
                        );
                    }
                    None => {
                        ui.spinner();
                    }
                }
                if ui
                    .add(widgets::button(icons::STOP, &tr!("grooves-stop")))
                    .clicked()
                {
                    events.push(MenuEvent::Grooves(GrooveEvent::StopDownload));
                }
            });
        }
        None => {
            if let Some(message) = error.as_deref() {
                ui.label(big(message).color(ui.visuals().warn_fg_color));
            }
            ui.horizontal_wrapped(|ui| {
                let add = ui
                    .add_enabled(
                        link.is_some(),
                        widgets::button(icons::PLUS, &tr!("grooves-link-add")),
                    )
                    .clicked();
                if (add || enter)
                    && let Some(link) = link
                {
                    *error = None;
                    events.push(MenuEvent::Grooves(GrooveEvent::Download(link)));
                }
                if ui.add(word_button(&tr!("menu-cancel"))).clicked() {
                    done = true;
                }
            });
        }
    }
    done
}

/// The Export dialog: MIDI and WAV, the progress of a WAV export with Stop, and where exports
/// go.
fn export(ui: &mut egui::Ui, p: &ProjectsView, events: &mut Vec<MenuEvent>) {
    match p.exporting {
        Some(fraction) => {
            ui.label(big(&tr!("menu-recording")));
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .show_percentage()
                        .desired_width(200.0),
                );
                if ui
                    .add(widgets::button(icons::STOP, &tr!("menu-stop")))
                    .clicked()
                {
                    events.push(MenuEvent::Project(ProjectEvent::CancelExport));
                }
            });
        }
        None => {
            // Each kind: its button, then what it is for on a line of its own.
            if ui
                .add(widgets::button(icons::MUSIC_NOTES, &tr!("menu-midi")))
                .clicked()
            {
                events.push(MenuEvent::Project(ProjectEvent::ExportMidi));
            }
            ui.label(tr!("menu-midi-about"));
            ui.add_space(4.0);
            if ui
                .add_enabled(
                    p.can_render,
                    widgets::button(icons::FILE_AUDIO, &tr!("menu-wav")),
                )
                .clicked()
            {
                events.push(MenuEvent::Project(ProjectEvent::ExportWav));
            }
            ui.label(if p.can_render {
                tr!("menu-wav-about")
            } else {
                tr!("menu-wav-about-no-bank")
            });
        }
    }
    if let Some(message) = p.export_message {
        ui.add_space(8.0);
        ui.label(big(message).color(ui.visuals().warn_fg_color));
    }
    if let Some(place) = p.export_place {
        ui.add_space(8.0);
        ui.label(tr!("menu-export-place", place = place));
    }
}

/// The Language dialog: the system's language, then each language by its own name, which is
/// never translated, so people can find theirs whatever the screen shows. Returns true when one
/// was chosen.
fn language(ui: &mut egui::Ui, view: &MenuView, events: &mut Vec<MenuEvent>) -> bool {
    let mut done = false;
    let system = tr!(
        "menu-language-system",
        language = view.system_language.native_name()
    );
    let choices = std::iter::once((None, system)).chain(
        Language::ALL
            .into_iter()
            .map(|l| (Some(l), l.native_name().to_owned())),
    );
    for (choice, text) in choices {
        let selected = view.language == choice;
        if ui
            .add(crate::toggle(selected, big(&text)).min_size(Vec2::new(ITEM_WIDTH, TAP)))
            .clicked()
        {
            if !selected {
                events.push(MenuEvent::Language(choice));
            }
            done = true;
        }
    }
    done
}

/// The MIDI output dialog: "None" first, then each output the system reports. A chosen output
/// that is not there now is listed too, so the choice still shows. A tap chooses and closes;
/// nothing asks. Returns true when an entry was tapped.
fn midi_output(ui: &mut egui::Ui, midi: &MidiView, events: &mut Vec<MenuEvent>) -> bool {
    let mut done = false;
    ui.label(tr!("menu-midi-output-about"));
    let missing = midi
        .choice
        .filter(|name| !midi.outputs.iter().any(|o| o == name));
    // Names are the system's, shown as they came.
    let choices = std::iter::once((None, tr!("menu-midi-output-none")))
        .chain(
            midi.outputs
                .iter()
                .map(|name| (Some(name.as_str()), name.clone())),
        )
        .chain(missing.map(|name| {
            (
                Some(name),
                tr!("menu-midi-output-not-connected", device = name),
            )
        }));
    for (choice, text) in choices {
        let selected = midi.choice == choice;
        if ui
            .add(crate::toggle(selected, big(&text)).min_size(Vec2::new(ITEM_WIDTH, TAP)))
            .clicked()
        {
            if !selected {
                events.push(MenuEvent::MidiOutput(choice.map(str::to_owned)));
            }
            done = true;
        }
    }
    // A choice that is listed but not connected yet, or that will not open.
    if let Some(name) = midi.choice
        && missing.is_none()
        && !midi.connected
    {
        ui.label(tr!("menu-midi-output-waiting", device = name));
    }
    done
}

/// A preset's description in the current language. The id is chosen by the preset's name; a
/// preset added without a message shows its English description.
fn preset_description(preset: &dm_project::Preset) -> String {
    let id = match preset.name {
        "Rock" => "menu-preset-rock",
        "Pop" => "menu-preset-pop",
        "Hip-hop" => "menu-preset-hip-hop",
        "Funk" => "menu-preset-funk",
        "Bossa nova" => "menu-preset-bossa-nova",
        "House" => "menu-preset-house",
        "Reggae" => "menu-preset-reggae",
        _ => return preset.description.to_owned(),
    };
    i18n::tr(id)
}
