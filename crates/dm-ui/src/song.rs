//! The song row: the patterns in play order, shown in song mode.
//!
//! Tapping an entry selects it; the selected entry can move left or right or leave the song.
//! Leaving the song never deletes the pattern.
//!
//! The song lives in the pattern pop-up, under the patterns: the switch between playing the
//! pattern and playing the song at the top ([`SongRow::mode_contents`]), the play order at the
//! end ([`SongRow::order_contents`]).

use dm_project::{Project, slot_name};
use egui::{RichText, Vec2};

use crate::patterns::tile;
use crate::strip::Mode;
use crate::{icons, widgets};

/// A grid of squares, for "Play the pattern".
const SQUARES_FOUR: &str = "\u{E464}";

pub(crate) enum SongEvent {
    /// Add the pattern being edited to the end.
    Append,
    MoveLeft(usize),
    MoveRight(usize),
    Remove(usize),
    ToggleLoop,
    /// Play the pattern being edited, or the song.
    SetMode(Mode),
}

#[derive(Default)]
pub(crate) struct SongRow {
    selected: Option<usize>,
}

impl SongRow {
    /// The switch between playing the pattern and playing the song. `mode` is what Play plays
    /// now.
    pub(crate) fn mode_contents(&mut self, ui: &mut egui::Ui, mode: Mode) -> Vec<SongEvent> {
        let mut events = Vec::new();
        // The switch. Side by side when there is room, one above the other on a phone.
        let choices = [
            (Mode::Pattern, SQUARES_FOUR, tr!("song-play-pattern")),
            (Mode::Song, icons::PLAYLIST, tr!("song-play-song")),
        ];
        // Side by side when both fit, measured, so a short language gets one line on a phone too.
        let gap = ui.spacing().item_spacing.x;
        let needed: f32 = choices
            .iter()
            .map(|(_, icon, word)| {
                let galley = egui::WidgetText::from(widgets::label(icon, word)).into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Button,
                );
                galley.size().x + 2.0 * ui.spacing().button_padding.x
            })
            .sum::<f32>()
            + gap;
        let side_by_side = needed <= ui.available_width();
        let width = if side_by_side {
            (ui.available_width() - gap) / 2.0
        } else {
            ui.available_width()
        };
        let mut switch = |ui: &mut egui::Ui| {
            for (choice, icon, word) in &choices {
                let choice = *choice;
                if ui
                    .add(
                        widgets::toggle_button(mode == choice, icon, word)
                            .min_size(Vec2::new(width, widgets::TAP)),
                    )
                    .clicked()
                    && mode != choice
                {
                    events.push(SongEvent::SetMode(choice));
                }
            }
        };
        if side_by_side {
            ui.horizontal(switch);
        } else {
            switch(ui);
        }
        events
    }

    /// The song's play order as large tiles, then the commands: move, take out, add the
    /// pattern being edited, and Repeat. `playing_entry` is the entry being heard, if the song
    /// is playing.
    pub(crate) fn order_contents(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        current: usize,
        playing_entry: Option<usize>,
    ) -> Vec<SongEvent> {
        let mut events = Vec::new();
        let song = project.song();
        if self.selected.is_some_and(|s| s >= song.len()) {
            self.selected = None;
        }
        let text = |text: String| RichText::new(text).size(widgets::TEXT_SIZE);

        ui.label(
            RichText::new(tr!("menu-song"))
                .size(widgets::TEXT_SIZE)
                .strong(),
        );
        ui.label(text(tr!("song-order")));
        ui.horizontal_wrapped(|ui| {
            if song.is_empty() {
                ui.label(text(tr!("song-empty")));
            }
            for (entry, &slot) in song.iter().enumerate() {
                let selected = self.selected == Some(entry);
                let response = tile(ui, selected, &slot_name(slot).to_string());
                if playing_entry == Some(entry) {
                    ui.painter().rect_stroke(
                        response.rect,
                        egui::CornerRadius::same(4),
                        egui::Stroke::new(3.0, ui.visuals().warn_fg_color),
                        egui::StrokeKind::Inside,
                    );
                }
                if response.clicked() {
                    self.selected = if selected { None } else { Some(entry) };
                }
            }
        });

        if let Some(entry) = self.selected {
            let name = slot_name(song[entry]).to_string();
            ui.label(text(tr!("song-take-out-keeps", name = name)));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        entry > 0,
                        widgets::button(icons::CARET_LEFT, &tr!("song-move-left")),
                    )
                    .clicked()
                {
                    events.push(SongEvent::MoveLeft(entry));
                    self.selected = Some(entry - 1);
                }
                if ui
                    .add_enabled(
                        entry + 1 < song.len(),
                        widgets::button(icons::CARET_RIGHT, &tr!("song-move-right")),
                    )
                    .clicked()
                {
                    events.push(SongEvent::MoveRight(entry));
                    self.selected = Some(entry + 1);
                }
                if ui
                    .add(widgets::button(icons::X, &tr!("song-take-out")))
                    .clicked()
                {
                    events.push(SongEvent::Remove(entry));
                    self.selected = None;
                }
                if ui
                    .add(widgets::button(icons::CHECK, &tr!("song-done")))
                    .clicked()
                {
                    self.selected = None;
                }
            });
        } else {
            let name = project.pattern(current).map(|p| p.name()).unwrap_or('?');
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(widgets::button(
                        icons::PLUS,
                        &tr!("song-add", name = name.to_string()),
                    ))
                    .clicked()
                {
                    events.push(SongEvent::Append);
                }
                if ui
                    .add(widgets::toggle_button(
                        project.song_loop,
                        icons::REPEAT,
                        &tr!("song-repeat"),
                    ))
                    .clicked()
                {
                    events.push(SongEvent::ToggleLoop);
                }
            });
            if !song.is_empty() {
                ui.label(text(tr!("song-hint")));
            }
        }
        events
    }
}
