//! The Sounds dialog: the drum kit, loud and soft steps, and adding a row. A row's own settings
//! open from its name in the grid (`rows.rs`); swing is in the tempo pop-up.

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

#[derive(Default)]
pub(crate) struct SoundsScreen {
    /// Whether the full kit list is open.
    all_kits: bool,
    /// Whether the list of drums for a new row is open.
    adding: bool,
    /// Whether that list shows every drum, not only the common ones.
    more_drums: bool,
}

impl SoundsScreen {
    /// The Sounds dialog's contents: the drum kit, loud and soft steps, and adding a row. The
    /// caller puts them in a `widgets::dialog`, which adds the title and the close button.
    /// A row's own settings are in its pop-up (`rows.rs`); swing is in the tempo pop-up.
    pub(crate) fn dialog_contents(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        kits: &[Kit],
        labels: RowLabels,
    ) -> Vec<SoundEvent> {
        use crate::icons;
        use crate::widgets::{TEXT_SIZE, toggle_button};

        let mut events = Vec::new();
        let heading = |ui: &mut egui::Ui, text: String| {
            ui.label(egui::RichText::new(text).size(TEXT_SIZE).strong());
        };

        // The kit: a short list first, one per line, and the bank's other kits on request.
        heading(ui, tr!("sounds-kit"));
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
        ui.add_space(12.0);

        // Adding a row: a list of drums opens in the dialog, which scrolls.
        let rows = project.rows().len();
        if ui
            .add_enabled(
                rows < MAX_ROWS,
                toggle_button(self.adding, icons::PLUS, &tr!("sounds-add-row")),
            )
            .clicked()
        {
            self.adding = !self.adding;
        }
        if rows >= MAX_ROWS {
            self.adding = false;
            ui.label(tr!("sounds-max-rows", count = MAX_ROWS));
        }
        if self.adding
            && let Some(key) = drum_picker(ui, None, &mut self.more_drums, labels)
        {
            events.push(SoundEvent::Add(key));
            events.push(SoundEvent::Preview(key, dm_project::DEFAULT_VOLUME));
            self.adding = false;
            self.more_drums = false;
        }
        events
    }
}

/// One kit as a line of the kit list: its name, and below it a few words on how it sounds when
/// [`kit_description`] has them. The line is as wide as the dialog and shows whether it is the
/// project's kit.
fn kit_row(ui: &mut egui::Ui, kit: &Kit, selected: bool) -> egui::Response {
    use crate::widgets::{TAP, TEXT_SIZE};

    let description = kit_description(kit);
    let height = if description.is_some() { 60.0 } else { TAP };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), egui::Sense::click());
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

/// Buttons for choosing a drum: the common ones, then "More drums" for the rest. `current` is
/// shown as chosen. `more` keeps whether the rest is open; it opens by itself when the current
/// drum is not a common one. Returns the drum tapped, if any.
/// With `labels` showing pictures, each drum shows its picture beside its name.
pub(crate) fn drum_picker(
    ui: &mut egui::Ui,
    current: Option<u8>,
    more: &mut bool,
    labels: RowLabels,
) -> Option<u8> {
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
        if response.clicked() {
            picked = Some(key);
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
