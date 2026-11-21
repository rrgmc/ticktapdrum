//! The settings of one row, in a pop-up beside its name in the grid: play, rename, mute, solo, volume,
//! drum, move up or down, fills for the pattern being edited, and remove. See "One control strip"
//! in `docs/DESIGN.md`.
//!
//! Tapping a row's name opens it. A tap outside closes it. On a short screen its contents
//! scroll. Choosing a drum shows the drum list in place of the settings, because a drop-down list
//! opened inside a pop-up closes the pop-up when an item is tapped.

use dm_project::Project;
use egui::{Id, LayerId, Popup, PopupCloseBehavior, Rect, RectAlign, RichText, Vec2};

use crate::drums::{drum_name, row_name};
use crate::grid::Grid;
use crate::icons;
use crate::sounds::{Pick, SoundEvent};
use crate::widgets::{TAP, TEXT_SIZE, button, toggle_button};

/// The gap between the pop-up and the screen's edges, and room for its frame.
const MARGIN: f32 = 8.0;
const FRAME: f32 = 16.0;
/// The widest the pop-up gets, so it covers no more of the grid than it must.
const MAX_WIDTH: f32 = 300.0;
const MIN_WIDTH: f32 = 180.0;

/// What the user did in a row's pop-up.
pub(crate) enum RowEvent {
    /// A change to the row itself: rename, mute, solo, volume, move, drum, remove, or
    /// play the drum once.
    Sound(SoundEvent),
    /// In the pattern being edited, turn on every `every`-th step of the row, counting from the
    /// first step of each bar, in all its bars.
    Fill { row: usize, every: usize },
    /// In the pattern being edited, turn off every step of the row.
    ClearRow { row: usize },
}

/// What the pop-up shows.
#[derive(Default, PartialEq)]
enum Page {
    #[default]
    Settings,
    /// The list of drums to choose from.
    Drums,
}

/// A row's name being edited. It starts as the name shown, which may be the drum's name in the
/// current language rather than the name the project stores.
struct Renaming {
    text: String,
    shown: String,
}

impl Renaming {
    /// The name to store, if the text was changed. Text left as shown changes nothing, so a
    /// translated drum name is not written into the project.
    fn changed(self) -> Option<String> {
        (self.text != self.shown).then_some(self.text)
    }
}

/// The pop-up with one row's settings. It remembers which row is open.
#[derive(Default)]
pub(crate) struct RowPopup {
    /// The open row, and where its name was when it opened.
    open: Option<(usize, Rect)>,
    page: Page,
    /// The name being edited, while renaming.
    renaming: Option<Renaming>,
    /// Set when renaming starts: select the whole name on the next frame.
    select_name: bool,
    /// Remove asks once more before it removes the row.
    /// The row whose removal is being asked about. The pop-up closes while the question shows
    /// in its own window, and this outlives it.
    asking_remove: Option<usize>,
    /// Whether the drum list shows every drum, not only the common ones.
    more_drums: bool,
    /// A name typed for another row when the pop-up moved away from it, to apply next frame.
    kept_rename: Option<RowEvent>,
}

impl RowPopup {
    /// Opens the pop-up for `row`, beside its name at `rect`. If another row's pop-up is open, it
    /// moves to this row; a name being typed there is kept, as when the pop-up closes.
    pub(crate) fn open(&mut self, row: usize, rect: Rect) {
        if self.row() != Some(row) {
            let kept = match (self.row(), self.renaming.take().and_then(Renaming::changed)) {
                (Some(old), Some(name)) => Some(RowEvent::Sound(SoundEvent::Rename(old, name))),
                _ => None,
            };
            *self = Self::default();
            self.kept_rename = kept;
        }
        self.open = Some((row, rect));
    }

    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }

    /// The open row, if any.
    pub(crate) fn row(&self) -> Option<usize> {
        self.open.map(|(row, _)| row)
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Shows the pop-up while it is open, beside the row's name as the grid last drew it. Call it
    /// after [`Grid::ui`] each frame. A row that moves takes the pop-up with it.
    pub(crate) fn show(
        &mut self,
        ctx: &egui::Context,
        project: &Project,
        grid: &Grid,
    ) -> Vec<RowEvent> {
        let mut events: Vec<RowEvent> = self.kept_rename.take().into_iter().collect();
        if let Some(row) = self.asking_remove.filter(|&r| r < project.rows().len()) {
            let info = &project.rows()[row];
            let name = row_name(&info.name, info.key);
            let question = if project.row_has_notes(row) {
                tr!("rows-remove-question-notes", name = name)
            } else {
                tr!("rows-remove-question", name = name)
            };
            match crate::widgets::confirm(
                ctx,
                "remove-row",
                &question,
                &tr!("rows-remove"),
                &tr!("rows-keep"),
            ) {
                Some(true) => {
                    events.push(RowEvent::Sound(SoundEvent::Remove(row)));
                    self.asking_remove = None;
                }
                Some(false) => self.asking_remove = None,
                None => {}
            }
        } else {
            self.asking_remove = None;
        }
        let Some((row, rect)) = self.open else {
            return events;
        };
        if row >= project.rows().len() {
            self.close();
            return events;
        }
        let anchor = grid.label_rect(row).unwrap_or(rect);
        self.open = Some((row, anchor));

        let screen = crate::widgets::safe_area(ctx);
        let width = (screen.right() - anchor.right() - MARGIN - FRAME).clamp(MIN_WIDTH, MAX_WIDTH);
        // The pop-up starts level with the row's name and runs down, or, for a row low on the
        // screen, ends level with it and runs up: whichever leaves more room. Its contents
        // scroll within that room, so it never runs under the system bars.
        let below = screen.bottom() - anchor.top();
        let above = anchor.bottom() - screen.top();
        let (align, room) = if below >= above {
            (RectAlign::RIGHT_START, below)
        } else {
            (RectAlign::RIGHT_END, above)
        };
        let max_height = (room - MARGIN - FRAME).max(120.0);
        let mut open = true;
        // One id per row, so a tap on another row's name opens a new pop-up rather than
        // counting as a tap outside the old one.
        Popup::new(
            Id::unique(("dm-row-popup", row)),
            ctx.clone(),
            anchor,
            LayerId::background(),
        )
        .open_bool(&mut open)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .align(align)
        .gap(4.0)
        .width(width)
        .show(|ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            let page = if self.page == Page::Drums {
                "drums"
            } else {
                "settings"
            };
            egui::ScrollArea::vertical()
                .id_salt(("dm-row-popup-scroll", page))
                .max_height(max_height)
                .min_scrolled_height(max_height)
                .show(ui, |ui| match self.page {
                    Page::Settings => self.settings(ui, project, row, &mut events),
                    Page::Drums => self.drums(ui, project, row, grid.row_labels(), &mut events),
                });
        });
        // A tap outside or Escape closed it.
        if !open && self.is_open() {
            // A name being typed is kept, as if Done were tapped.
            if let Some(name) = self.renaming.take().and_then(Renaming::changed) {
                events.push(RowEvent::Sound(SoundEvent::Rename(row, name)));
            }
            self.close();
        }
        events
    }

    fn settings(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        row: usize,
        events: &mut Vec<RowEvent>,
    ) {
        let info = &project.rows()[row];
        let rows = project.rows().len();
        let sound = |events: &mut Vec<RowEvent>, event| events.push(RowEvent::Sound(event));

        // The name, and renaming it.
        match &mut self.renaming {
            Some(renaming) => {
                let mut output = egui::TextEdit::singleline(&mut renaming.text)
                    .desired_width(ui.available_width())
                    .font(egui::FontId::proportional(20.0))
                    .show(ui);
                let edit = output.response.clone();
                if self.select_name {
                    // The whole name starts selected, so typing replaces it, as in most rename
                    // boxes.
                    output
                        .state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::select_all(&output.galley)));
                    output.state.store(ui.ctx(), edit.id);
                    self.select_name = false;
                    // Focus once, when the box opens. Asking every frame also interrupts the
                    // input method each frame, and on Android that closes and reopens the
                    // on-screen keyboard over and over.
                    edit.request_focus();
                }
                let done = ui.add(button(icons::CHECK, &tr!("rows-done"))).clicked()
                    // Enter finishes too, with or without focus: on Android the keyboard's Enter
                    // comes as a press and a release in one frame, and the box reports losing
                    // focus only on a later one.
                    || ui.input(|i| i.key_pressed(egui::Key::Enter));
                if done && let Some(name) = self.renaming.take().and_then(Renaming::changed) {
                    sound(events, SoundEvent::Rename(row, name));
                }
            }
            None => {
                ui.horizontal(|ui| {
                    // Plays the drum again, as often as it is tapped, at the row's volume.
                    if ui.add(button(icons::PLAY, "")).clicked() {
                        sound(events, SoundEvent::Preview(info.key, info.volume));
                    }
                    let shown = row_name(&info.name, info.key);
                    if ui.add(button(icons::PENCIL_SIMPLE, "")).clicked() {
                        self.renaming = Some(Renaming {
                            text: shown.clone(),
                            shown: shown.clone(),
                        });
                        self.select_name = true;
                    }
                    ui.add(egui::Label::new(RichText::new(shown).size(20.0).strong()).wrap());
                });
            }
        }

        // Mute and solo.
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(toggle_button(
                    info.mute,
                    icons::SPEAKER_SLASH,
                    &tr!("rows-mute"),
                ))
                .clicked()
            {
                sound(events, SoundEvent::Mute(row));
            }
            if ui
                .add(toggle_button(
                    info.solo,
                    icons::HEADPHONES,
                    &tr!("rows-solo"),
                ))
                .clicked()
            {
                sound(events, SoundEvent::Solo(row));
            }
        });

        // The volume, on a line of its own sized to the pop-up. Letting go plays the drum.
        ui.label(
            RichText::new(format!("{} {}", icons::SPEAKER_HIGH, tr!("rows-volume")))
                .size(TEXT_SIZE),
        );
        ui.scope(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 64.0).max(80.0);
            ui.spacing_mut().interact_size.y = 40.0;
            let mut volume = info.volume;
            let slider = ui.add(egui::Slider::new(&mut volume, 0..=100).suffix("%"));
            if slider.changed() {
                sound(events, SoundEvent::Volume(row, volume));
            }
            if slider.drag_stopped() {
                sound(events, SoundEvent::Preview(info.key, volume));
            }
        });

        // The drum: opens the list in place of the settings.
        if ui
            .add(
                egui::Button::new(
                    RichText::new(format!(
                        "{} {} {}",
                        icons::WAVEFORM,
                        drum_name(info.key),
                        icons::CARET_RIGHT
                    ))
                    .size(TEXT_SIZE),
                )
                .wrap()
                .min_size(Vec2::new(ui.available_width(), TAP)),
            )
            .clicked()
        {
            self.page = Page::Drums;
            self.more_drums = false;
        }

        // Moving the row.
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(row > 0, button(icons::ARROW_UP, &tr!("rows-up")))
                .clicked()
            {
                sound(events, SoundEvent::MoveUp(row));
                self.follow(row - 1);
            }
            if ui
                .add_enabled(row + 1 < rows, button(icons::ARROW_DOWN, &tr!("rows-down")))
                .clicked()
            {
                sound(events, SoundEvent::MoveDown(row));
                self.follow(row + 1);
            }
        });

        // Fills for the pattern being edited.
        ui.separator();
        ui.label(
            RichText::new(tr!("rows-in-this-pattern"))
                .size(TEXT_SIZE)
                .strong(),
        );
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(button(icons::METRONOME, &tr!("rows-every-beat")))
                .clicked()
            {
                events.push(RowEvent::Fill { row, every: 4 });
            }
            if ui
                .add(button(icons::MUSIC_NOTES, &tr!("rows-every-two-steps")))
                .clicked()
            {
                events.push(RowEvent::Fill { row, every: 2 });
            }
            if ui.add(button(icons::BROOM, &tr!("rows-clear"))).clicked() {
                events.push(RowEvent::ClearRow { row });
            }
        });

        // Removing the row, from every pattern. It always asks first, in a window of its own;
        // the pop-up closes, so a tap on the window is not taken as a tap outside it.
        ui.separator();
        if ui
            .add_enabled(rows > 1, button(icons::TRASH, &tr!("rows-remove-row")))
            .clicked()
        {
            self.close();
            self.asking_remove = Some(row);
        }
    }

    /// The drum list. Choosing one plays it and goes back to the settings.
    fn drums(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        row: usize,
        labels: crate::pictures::RowLabels,
        events: &mut Vec<RowEvent>,
    ) {
        let info = &project.rows()[row];
        if ui
            .add(button(icons::CARET_LEFT, &tr!("rows-back")))
            .clicked()
        {
            self.page = Page::Settings;
            return;
        }
        match crate::sounds::drum_picker(ui, Some(info.key), &[], &mut self.more_drums, labels) {
            Some(Pick::Chosen(key)) => {
                if info.key != key {
                    events.push(RowEvent::Sound(SoundEvent::SetDrum(row, key)));
                }
                events.push(RowEvent::Sound(SoundEvent::Preview(key, info.volume)));
                self.page = Page::Settings;
            }
            Some(Pick::Heard(key)) => {
                events.push(RowEvent::Sound(SoundEvent::Preview(key, info.volume)));
            }
            None => {}
        }
    }

    /// Keeps the pop-up on a row that moved to `row`. The grid draws it there next frame.
    fn follow(&mut self, row: usize) {
        if let Some((_, rect)) = self.open {
            self.open = Some((row, rect));
        }
    }
}
