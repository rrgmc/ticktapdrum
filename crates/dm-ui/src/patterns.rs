//! Pattern tabs: one per pattern, a button to add one, and a menu for the rest. From milestone 8
//! the same choices open in a pop-up from the pattern button in the top strip.
//!
//! Everything opens with a tap; nothing depends on hover or a right click, so it works the same
//! with a finger, a mouse and an iPad pencil.

use dm_project::{MAX_PATTERNS, Project};
use egui::{RichText, Vec2};

use crate::{icons, widgets};

/// The side of a pattern tile in the pop-up, and of a song entry in the Song dialog.
pub(crate) const TILE: f32 = 64.0;
/// The widest the pattern pop-up grows: six tiles in a line.
pub(crate) const POPUP_MAX_WIDTH: f32 = 6.0 * TILE + 5.0 * 8.0;

pub(crate) enum PatternEvent {
    Select(usize),
    Add,
    Copy,
    Delete,
}

#[derive(Default)]
pub(crate) struct PatternTabs {
    /// The pattern whose deletion is being asked about. The pop-up closes while the question
    /// shows in its own window (see [`PatternTabs::confirm`]).
    asking_delete: Option<usize>,
}

impl PatternTabs {
    /// The patterns' part of the pattern pop-up: a large tile per pattern, then New, Copy and
    /// Delete. `playing` is the index of the pattern being heard, if any; its tile carries a dot.
    /// Choosing a pattern leaves the pop-up open, so it can then go into the song below; New and
    /// Copy close it, to show the new pattern.
    pub(crate) fn popup_contents(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        current: usize,
        playing: Option<usize>,
    ) -> Vec<PatternEvent> {
        let mut events = Vec::new();
        ui.horizontal_wrapped(|ui| {
            for (index, pattern) in project.patterns().iter().enumerate() {
                let response = tile(ui, index == current, &pattern.name().to_string());
                if playing == Some(index) {
                    playing_dot(ui, response.rect);
                }
                if response.clicked() && index != current {
                    events.push(PatternEvent::Select(index));
                }
            }
        });
        ui.separator();

        let room = project.patterns().len() < MAX_PATTERNS;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(room, widgets::button(icons::PLUS, &tr!("patterns-new")))
                .clicked()
            {
                events.push(PatternEvent::Add);
                ui.close();
            }
            if ui
                .add_enabled(room, widgets::button(icons::COPY, &tr!("patterns-copy")))
                .clicked()
            {
                events.push(PatternEvent::Copy);
                ui.close();
            }
            if ui
                .add_enabled(
                    project.patterns().len() > 1,
                    widgets::button(icons::TRASH, &tr!("patterns-delete")),
                )
                .clicked()
            {
                // Asked in a window of its own, so the pop-up closes.
                self.asking_delete = Some(current);
                ui.close();
            }
        });
        events
    }

    /// The "Delete pattern?" question, in a window of its own. Call it every frame; it shows only
    /// while the question is asked, and answers with [`PatternEvent::Delete`] for the pattern
    /// being edited, `current`.
    pub(crate) fn confirm(
        &mut self,
        ctx: &egui::Context,
        project: &Project,
        current: usize,
    ) -> Vec<PatternEvent> {
        let mut events = Vec::new();
        // Only for the pattern still being edited, and never the last one.
        let Some(index) = self
            .asking_delete
            .filter(|&i| i == current && project.patterns().len() > 1)
        else {
            self.asking_delete = None;
            return events;
        };
        let name = project.pattern(index).map(|p| p.name()).unwrap_or('?');
        let question = tr!("patterns-delete-question", name = name.to_string());
        match widgets::confirm(
            ctx,
            "delete-pattern",
            &question,
            &tr!("patterns-delete"),
            &tr!("patterns-keep"),
        ) {
            Some(true) => {
                events.push(PatternEvent::Delete);
                self.asking_delete = None;
            }
            Some(false) => self.asking_delete = None,
            None => {}
        }
        events
    }
}

/// A large square button with a letter: a pattern in the pop-up, or a song entry.
pub(crate) fn tile(ui: &mut egui::Ui, selected: bool, text: &str) -> egui::Response {
    ui.add(
        crate::toggle(selected, RichText::new(text).size(28.0).strong())
            .min_size(Vec2::splat(TILE)),
    )
}

/// The dot on the tile of the pattern being heard.
pub(crate) fn playing_dot(ui: &egui::Ui, rect: egui::Rect) {
    let color = ui.visuals().warn_fg_color;
    ui.painter()
        .circle_filled(rect.right_top() + Vec2::new(-10.0, 10.0), 6.0, color);
}

/// The width of a pop-up: `max`, or less on a narrow screen, so it fits a 320-point phone.
pub(crate) fn popup_width(ui: &egui::Ui, max: f32) -> f32 {
    (crate::widgets::safe_area(ui.ctx()).width() - 40.0).clamp(200.0, max)
}
