//! The Sound bank dialog, from the menu's Settings: which SoundFont plays. ColomboGMGS2 and each
//! SoundFont the person added, with a way to add another and to delete one. See "Milestone 15:
//! other SoundFonts" in `docs/DESIGN.md`.

use dm_bank::UserSoundfont;
use egui::Vec2;

use crate::bank::{BANK, mib, stem};
use crate::{icons, widgets};

/// What the dialog shows.
pub(crate) struct BankView<'a> {
    /// The added SoundFont playing, by its file name; `None` while ColomboGMGS2 plays.
    pub(crate) playing: Option<&'a str>,
    pub(crate) colombo_installed: bool,
    pub(crate) added: &'a [UserSoundfont],
    /// The SoundFont being added and the share copied, if one is.
    pub(crate) adding: Option<(&'a str, Option<f32>)>,
    /// Whether this platform can pick a file to add.
    pub(crate) can_add: bool,
}

impl BankView<'_> {
    /// How the menu item names the SoundFont playing.
    pub(crate) fn playing_name(&self) -> String {
        self.playing.map_or_else(|| BANK.name.to_owned(), stem)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BankEvent {
    /// The dialog has just opened: read the list of added SoundFonts again.
    List,
    /// Play this added SoundFont, by file name, or ColomboGMGS2 if `None`.
    Use(Option<String>),
    /// Pick a file to add.
    Add,
    CancelAdd,
    /// Delete this added SoundFont, by file name. Asked first.
    Delete(String),
}

/// The dialog's contents. `delete` is set to the SoundFont whose Delete was tapped, for the
/// caller to ask about.
pub(crate) fn contents(
    ui: &mut egui::Ui,
    view: &BankView,
    delete: &mut Option<String>,
) -> Vec<BankEvent> {
    let mut events = Vec::new();
    ui.label(tr!("soundbank-intro"));
    ui.add_space(8.0);

    let colombo_detail = if view.colombo_installed {
        tr!("soundbank-size", size = mib(BANK.member_bytes))
    } else {
        tr!("soundbank-not-downloaded")
    };
    if choice(ui, BANK.name, &colombo_detail, view.playing.is_none()).clicked() {
        events.push(BankEvent::Use(None));
    }
    for soundfont in view.added {
        let chosen = view.playing == Some(soundfont.name.as_str());
        let detail = tr!("soundbank-size", size = mib(soundfont.bytes));
        ui.horizontal(|ui| {
            let width = ui.available_width() - 48.0;
            if choice_sized(ui, &stem(&soundfont.name), &detail, chosen, width).clicked() {
                events.push(BankEvent::Use(Some(soundfont.name.clone())));
            }
            if ui
                .add(egui::Button::new(icons::TRASH).min_size(Vec2::splat(40.0)))
                .on_hover_text(tr!("soundbank-delete"))
                .clicked()
            {
                *delete = Some(soundfont.name.clone());
            }
        });
    }
    ui.add_space(8.0);
    if let Some((name, fraction)) = view.adding {
        ui.label(tr!("soundbank-adding", name = stem(name)));
        crate::bank::progress_bar(ui, fraction);
        if ui.button(tr!("bank-cancel")).clicked() {
            events.push(BankEvent::CancelAdd);
        }
    } else if view.can_add {
        if ui
            .add(widgets::button(icons::PLUS, &tr!("soundbank-add")))
            .clicked()
        {
            events.push(BankEvent::Add);
        }
        ui.label(
            egui::RichText::new(tr!("soundbank-add-hint"))
                .small()
                .weak(),
        );
    } else {
        ui.label(
            egui::RichText::new(tr!("soundbank-add-share"))
                .small()
                .weak(),
        );
    }
    events
}

/// One SoundFont as a line: its name, and below it its size or state. Marked when it plays.
fn choice(ui: &mut egui::Ui, name: &str, detail: &str, chosen: bool) -> egui::Response {
    let width = ui.available_width();
    choice_sized(ui, name, detail, chosen, width)
}

fn choice_sized(
    ui: &mut egui::Ui,
    name: &str,
    detail: &str,
    chosen: bool,
    width: f32,
) -> egui::Response {
    let mut text = egui::text::LayoutJob::default();
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::TextStyle::Small.resolve(ui.style());
    let strong = ui.visuals().strong_text_color();
    let weak = ui.visuals().weak_text_color();
    text.append(name, 0.0, egui::TextFormat::simple(body, strong));
    text.append(
        &format!("\n{detail}"),
        0.0,
        egui::TextFormat::simple(small, weak),
    );
    ui.add(
        egui::Button::selectable(chosen, text)
            .min_size(Vec2::new(width, 44.0))
            .wrap(),
    )
}
