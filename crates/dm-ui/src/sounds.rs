//! The Sound kits dialog, which chooses the drum kit, and the Add instrument dialog, which adds a
//! row. A row's own settings open from its name in the grid (`rows.rs`); swing is in the tempo
//! pop-up.

use crate::pictures::RowLabels;
use dm_audio::Kit;
use dm_project::{COMMON_DRUMS, GM_DRUMS, MAX_ROWS, Project, drum_family};
use egui::Vec2;

use crate::drums::drum_name;

/// The kits a beginner sees first, by General MIDI program number, when the bank has them.
const SHORT_LIST: [u8; 11] = [0, 8, 9, 16, 24, 25, 26, 30, 32, 40, 48];

pub(crate) enum SoundEvent {
    Kit(u8),
    Rename(usize, String),
    Mute(usize),
    Solo(usize),
    Volume(usize, u8),
    MoveUp(usize),
    MoveDown(usize),
    SetDrum(usize, u8),
    Remove(usize),
    Add(u8),
    /// Play a drum once, at this volume in percent.
    Preview(u8, u8),
}

/// The dialog a Sounds button or menu item opened.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SoundsDialog {
    /// Sound kits: the drum kit.
    Kits,
    /// Add instrument: a drum for a new row.
    Add,
}

#[derive(Default)]
pub(crate) struct SoundsScreen {
    /// Whether the full kit list is open.
    all_kits: bool,
    /// Whether the Add instrument list shows every drum, not only the common ones.
    more_drums: bool,
}

impl SoundsScreen {
    /// The Sound kits dialog's contents: a short list of kits, one per line, and the bank's
    /// other kits on request. The caller puts them in a `widgets::dialog`, which adds the title
    /// and the close button.
    pub(crate) fn kits_contents(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        kits: &[Kit],
    ) -> Vec<SoundEvent> {
        use crate::icons;

        let mut events = Vec::new();
        let current = project.kit();
        let short: Vec<&Kit> = SHORT_LIST
            .iter()
            .filter_map(|program| kits.iter().find(|k| k.program == *program))
            .collect();
        let in_short = |kit: &Kit| short.iter().any(|k| k.program == kit.program);
        let mut listed = short.clone();
        // A kit chosen from the others stays in sight while they are hidden.
        if !self.all_kits
            && let Some(kit) = kits.iter().find(|k| k.program == current && !in_short(k))
        {
            listed.push(kit);
        }
        for kit in listed {
            if kit_row(ui, kit, current == kit.program).clicked() {
                events.push(SoundEvent::Kit(kit.program));
            }
        }
        if kits.len() > short.len() {
            let (icon, word) = if self.all_kits {
                (icons::MINUS, tr!("sounds-fewer-kits"))
            } else {
                (icons::PLUS, tr!("sounds-more-kits"))
            };
            if ui.add(crate::widgets::button(icon, &word)).clicked() {
                self.all_kits = !self.all_kits;
            }
        }
        if self.all_kits {
            for kit in kits.iter().filter(|k| !in_short(k)) {
                if kit_row(ui, kit, current == kit.program).clicked() {
                    events.push(SoundEvent::Kit(kit.program));
                }
            }
        }
        events
    }

    /// The Add instrument dialog's contents: the drums to choose from for a new row at the
    /// bottom. Returns the events and whether a drum was chosen, which closes the dialog.
    pub(crate) fn add_contents(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        labels: RowLabels,
    ) -> (Vec<SoundEvent>, bool) {
        let mut events = Vec::new();
        if project.rows().len() >= MAX_ROWS {
            ui.label(tr!("sounds-max-rows", count = MAX_ROWS));
            return (events, false);
        }
        let added: Vec<u8> = project.rows().iter().map(|row| row.key).collect();
        match drum_picker(ui, None, &added, &mut self.more_drums, labels) {
            Some(Pick::Chosen(key)) => {
                events.push(SoundEvent::Add(key));
                events.push(SoundEvent::Preview(key, dm_project::DEFAULT_VOLUME));
                self.more_drums = false;
                (events, true)
            }
            Some(Pick::Heard(key)) => {
                events.push(SoundEvent::Preview(key, dm_project::DEFAULT_VOLUME));
                (events, false)
            }
            None => (events, false),
        }
    }
}

/// One kit as a line of the kit list: its name, and below it a few words on how it sounds when
/// [`kit_description`] has them. The line is as wide as the dialog and shows whether it is the
/// project's kit.
fn kit_row(ui: &mut egui::Ui, kit: &Kit, selected: bool) -> egui::Response {
    use crate::widgets::{TAP, TEXT_SIZE};

    let description = kit_description(kit);
    let height = if description.is_some() { 60.0 } else { TAP };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        let painter = ui.painter();
        painter.rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
        let ink = visuals.text_color();
        let left = rect.left() + ui.spacing().button_padding.x + 4.0;
        let font = |size: f32| egui::FontId::proportional(size);
        match description {
            Some(words) => {
                painter.text(
                    egui::pos2(left, rect.top() + 8.0),
                    egui::Align2::LEFT_TOP,
                    &kit.name,
                    font(TEXT_SIZE),
                    ink,
                );
                painter.text(
                    egui::pos2(left, rect.bottom() - 8.0),
                    egui::Align2::LEFT_BOTTOM,
                    words,
                    font(14.0),
                    ink.gamma_multiply(0.7),
                );
            }
            None => {
                painter.text(
                    egui::pos2(left, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &kit.name,
                    font(TEXT_SIZE),
                    ink,
                );
            }
        }
    }
    response
}

/// A few words on how a kit of the short list sounds. The words follow the General MIDI program
/// number, so they show only when the kit's name says it is the kit they describe: another bank
/// may put something else at that number.
fn kit_description(kit: &Kit) -> Option<String> {
    let (expected, id) = match kit.program {
        0 => ("standard", "sounds-kit-standard"),
        8 => ("room", "sounds-kit-room"),
        9 => ("hip", "sounds-kit-hiphop"),
        16 => ("power", "sounds-kit-power"),
        24 => ("electr", "sounds-kit-electro"),
        25 => ("808", "sounds-kit-808"),
        26 => ("dance", "sounds-kit-dance"),
        30 => ("909", "sounds-kit-909"),
        32 => ("jazz", "sounds-kit-jazz"),
        40 => ("brush", "sounds-kit-brush"),
        48 => ("orch", "sounds-kit-orchestral"),
        _ => return None,
    };
    kit.name
        .to_lowercase()
        .contains(expected)
        .then(|| crate::i18n::tr(id))
}

/// What a drum picker's button was used for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Pick {
    /// A tap or left click chose this drum.
    Chosen(u8),
    /// A right click or long press asked to hear this drum without choosing it.
    Heard(u8),
}

/// Buttons for choosing a drum: the common ones, then "More drums" for the rest. `current` is
/// shown as chosen. The drums in `added` already have a row: they show faint and cannot be
/// chosen, but can still be heard. `more` keeps whether the rest is open; it opens by itself
/// when the current drum is not a common one. Returns what a button was used for, if anything.
/// With `labels` showing pictures, each drum shows its picture beside its name.
pub(crate) fn drum_picker(
    ui: &mut egui::Ui,
    current: Option<u8>,
    added: &[u8],
    more: &mut bool,
    labels: RowLabels,
) -> Option<Pick> {
    use crate::icons;
    use crate::widgets::toggle_button;

    if current.is_some_and(|key| !COMMON_DRUMS.contains(&key)) {
        *more = true;
    }
    let mut picked = None;
    let pictures = labels.shows_pictures();
    let mut button = |ui: &mut egui::Ui, key: u8, name: &str| {
        // The spaces leave room for the stripe or the picture.
        let room = if pictures { "         " } else { "  " };
        let taken = added.contains(&key);
        let opacity = ui.opacity();
        if taken {
            ui.multiply_opacity(0.4);
        }
        let response = ui.add(toggle_button(
            current == Some(key),
            "",
            &format!("{room}{name}"),
        ));
        let colour = crate::widgets::family_colour(drum_family(key), ui.visuals().dark_mode);
        if pictures {
            let picture = egui::Rect::from_min_size(
                response.rect.left_top() + Vec2::new(6.0, 7.0),
                Vec2::splat(response.rect.height() - 14.0),
            );
            let ink = ui.visuals().text_color();
            crate::pictures::paint(ui.painter(), picture, key, colour, ink);
        } else {
            // A stripe in the drum's family colour, as beside a row's name in the grid.
            let stripe = egui::Rect::from_min_size(
                response.rect.left_top() + Vec2::new(4.0, 8.0),
                Vec2::new(4.0, response.rect.height() - 16.0),
            );
            ui.painter()
                .rect_filled(stripe, egui::CornerRadius::same(2), colour);
        }
        ui.set_opacity(opacity);
        if response.secondary_clicked() || response.long_touched() {
            picked = Some(Pick::Heard(key));
        } else if response.clicked() && !taken {
            picked = Some(Pick::Chosen(key));
        }
        if taken {
            response.on_hover_text(tr!("sounds-already-added"));
        }
    };
    ui.horizontal_wrapped(|ui| {
        for key in COMMON_DRUMS {
            button(ui, key, &drum_name(key));
        }
    });
    if ui
        .add(toggle_button(
            *more,
            icons::DOTS_THREE,
            &tr!("sounds-more-drums"),
        ))
        .clicked()
    {
        *more = !*more;
    }
    if *more {
        ui.horizontal_wrapped(|ui| {
            for (key, _) in GM_DRUMS {
                if !COMMON_DRUMS.contains(&key) {
                    button(ui, key, &drum_name(key));
                }
            }
        });
    }
    picked
}
