//! The bar strip above the grid: one thumbnail per bar, and the bar commands.
//!
//! The thumbnails and commands open in a pop-up from the bar label in the top strip. Tapping a
//! thumbnail shows that bar in the grid and leaves the pop-up open. The thumbnails wrap onto
//! more lines. A setting can also show the thumbnails all the time, under the strip, on one line
//! that scrolls sideways.
//!
//! "Remove last bar" asks first, in a window of its own, and then deletes the bar with its
//! notes. Nothing is kept hidden for later: a bar that is gone is gone, which is easier to
//! understand than bars that disappear and come back.

use dm_project::{MAX_BARS, Pattern, STEPS_PER_BAR};
use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::{icons, widgets};

/// A thumbnail in the pop-up, as tall as a button a finger can hit.
const POPUP_THUMB: Vec2 = Vec2::new(64.0, widgets::TAP);
/// The widest the bar pop-up grows: eight thumbnails in a line, two lines for 16 bars.
const POPUP_MAX_WIDTH: f32 = 8.0 * 64.0 + 7.0 * 8.0;
/// Keeps the grid on the bar being heard while playing: Phosphor's crosshair-simple, as in
/// tracking a moving target.
const FOLLOW: &str = "\u{E1D8}";

pub(crate) enum BarEvent {
    /// Show this bar in the grid.
    Select(usize),
    AddBar,
    /// Delete the last bar and its notes. Sent once the user has said yes.
    RemoveBar,
    /// Copy the first bar shown in the grid.
    CopyBar,
    ToggleFollow,
    /// Play the pattern from the first step of this bar.
    PlayFrom(usize),
}

#[derive(Default)]
pub(crate) struct BarStrip {
    /// "Remove last bar" asks first, in a window of its own (see [`BarStrip::confirm`]). Holds
    /// the number of the bar asked about, counting from 1.
    asking_remove: Option<usize>,
    /// The first bar the grid showed last frame, so the line of thumbnails scrolls to it only
    /// when it changes and leaves a sideways scroll by hand alone otherwise.
    last_shown: Option<usize>,
}

impl BarStrip {
    /// The contents of the bar pop-up: the thumbnails, wrapping onto more lines, then the bar
    /// commands. `shown` is the range of bars the grid shows; `playing_bar` the bar being heard,
    /// if it is in this pattern. Tapping a thumbnail shows that bar; the pop-up stays open.
    /// `with_thumbnails` is false while the line under the strip already shows them.
    /// `can_play_from` offers playing from the first bar shown, which only a pattern can do.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn popup_contents(
        &mut self,
        ui: &mut egui::Ui,
        pattern: &Pattern,
        shown: std::ops::Range<usize>,
        playing_bar: Option<usize>,
        follow: bool,
        with_thumbnails: bool,
        can_play_from: bool,
    ) -> Vec<BarEvent> {
        let mut events = Vec::new();
        ui.set_width(crate::patterns::popup_width(ui, POPUP_MAX_WIDTH));

        // The pop-up stays open, so several bars can be looked at in turn.
        if with_thumbnails {
            ui.horizontal_wrapped(|ui| {
                thumbnails(ui, pattern, &shown, playing_bar, None, &mut events);
            });
            ui.separator();
        }
        // Playing from a bar goes with picking it, so it is offered only where the thumbnails
        // are; the line under the strip has its own.
        if with_thumbnails && can_play_from {
            let bar = shown.start;
            if ui
                .add(widgets::button(
                    icons::PLAY,
                    &tr!("bars-play-from", bar = bar + 1),
                ))
                .clicked()
            {
                events.push(BarEvent::PlayFrom(bar));
                ui.close();
            }
        }

        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    pattern.bars() < MAX_BARS,
                    widgets::button(icons::PLUS, &tr!("bars-add")),
                )
                .clicked()
            {
                events.push(BarEvent::AddBar);
            }
            if ui
                .add_enabled(
                    pattern.bars() > 1,
                    widgets::button(icons::TRASH, &tr!("bars-remove-last")),
                )
                .clicked()
            {
                // Asked in a window of its own, so the pop-up closes.
                self.asking_remove = Some(pattern.bars());
                ui.close();
            }
            if ui
                .add_enabled(
                    pattern.bars() < MAX_BARS,
                    widgets::button(icons::COPY, &tr!("bars-copy")),
                )
                .clicked()
            {
                events.push(BarEvent::CopyBar);
            }
            if ui
                .add(widgets::toggle_button(follow, FOLLOW, &tr!("bars-follow")))
                .clicked()
            {
                events.push(BarEvent::ToggleFollow);
            }
        });

        events
    }

    /// The thumbnails on one line under the strip, scrolling sideways when they do not fit,
    /// after two buttons that stay put: Follow, and play from bar N, the first bar the grid
    /// shows, offered when `can_play_from`. The buttons are the size of a thumbnail and line up
    /// with them. The line scrolls to the bars the grid shows whenever those change, such as
    /// while following the music.
    pub(crate) fn line(
        &mut self,
        ui: &mut egui::Ui,
        pattern: &Pattern,
        shown: std::ops::Range<usize>,
        playing_bar: Option<usize>,
        follow: bool,
        can_play_from: bool,
    ) -> Vec<BarEvent> {
        let mut events = Vec::new();
        let moved = self.last_shown.replace(shown.start) != Some(shown.start);
        ui.horizontal_top(|ui| {
            if ui
                .add_sized(POPUP_THUMB, widgets::toggle_button(follow, FOLLOW, ""))
                .on_hover_text(tr!("bars-follow"))
                .clicked()
            {
                events.push(BarEvent::ToggleFollow);
            }
            if can_play_from {
                let bar = shown.start;
                let number = (bar + 1).to_string();
                if ui
                    .add_sized(POPUP_THUMB, widgets::button(icons::PLAY, &number))
                    .on_hover_text(tr!("bars-play-from", bar = bar + 1))
                    .clicked()
                {
                    events.push(BarEvent::PlayFrom(bar));
                }
            }
            // A line between the buttons and the bars they act on, as tall as a thumbnail. egui's
            // separator would take all the height left in the window and push the grid away.
            let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, POPUP_THUMB.y), Sense::hover());
            ui.painter().vline(
                rect.center().x,
                rect.y_range(),
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
            // A solid scroll bar takes its own room under the thumbnails; the default floating
            // one is drawn over them.
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::horizontal()
                .id_salt("bar-line")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let scroll_to = moved.then_some(shown.start);
                        thumbnails(ui, pattern, &shown, playing_bar, scroll_to, &mut events);
                    });
                });
        });
        events
    }

    /// The "Remove bar?" question, in a window of its own. Call it every frame; it shows only
    /// while the question is asked, and only while `bars`, the pattern's bar count, still matches
    /// the bar asked about.
    pub(crate) fn confirm(&mut self, ctx: &egui::Context, bars: usize) -> Vec<BarEvent> {
        let mut events = Vec::new();
        let Some(number) = self.asking_remove.filter(|&n| n == bars && n > 1) else {
            self.asking_remove = None;
            return events;
        };
        match widgets::confirm(
            ctx,
            "remove-bar",
            &tr!("bars-remove-question", number = number),
            &tr!("bars-remove"),
            &tr!("bars-keep"),
        ) {
            Some(true) => {
                events.push(BarEvent::RemoveBar);
                self.asking_remove = None;
            }
            Some(false) => self.asking_remove = None,
            None => {}
        }
        events
    }
}
/// One thumbnail per bar of `pattern`, laid out by `ui`. A tap on one sends [`BarEvent::Select`].
/// `scroll_to` names a bar to scroll into view.
fn thumbnails(
    ui: &mut egui::Ui,
    pattern: &Pattern,
    shown: &std::ops::Range<usize>,
    playing_bar: Option<usize>,
    scroll_to: Option<usize>,
    events: &mut Vec<BarEvent>,
) {
    for bar in 0..pattern.bars() {
        let response = thumbnail(
            ui,
            pattern,
            bar,
            POPUP_THUMB,
            shown.contains(&bar),
            playing_bar == Some(bar),
        );
        if scroll_to == Some(bar) {
            response.scroll_to_me(None);
        }
        if response.clicked() {
            events.push(BarEvent::Select(bar));
        }
    }
}

/// One bar drawn small, `size` points: a dot per note, its number below.
fn thumbnail(
    ui: &mut egui::Ui,
    pattern: &Pattern,
    bar: usize,
    size: Vec2,
    selected: bool,
    playing: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let visuals = ui.visuals();
    let painter = ui.painter();
    let fill = if selected {
        visuals.selection.bg_fill.gamma_multiply(0.5)
    } else {
        visuals.widgets.inactive.bg_fill
    };
    painter.rect_filled(rect, CornerRadius::same(4), fill);
    if playing {
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(2.0, visuals.warn_fg_color),
            StrokeKind::Inside,
        );
    }
    let inner = rect.shrink2(Vec2::new(4.0, 4.0));
    let rows = pattern.rows();
    let dot = Vec2::new(
        inner.width() / STEPS_PER_BAR as f32,
        (inner.height() - 10.0) / rows.max(1) as f32,
    );
    for row in 0..rows {
        for i in 0..STEPS_PER_BAR {
            if pattern.velocity(row, bar * STEPS_PER_BAR + i) > 0 {
                let min = Pos2::new(
                    inner.min.x + i as f32 * dot.x,
                    inner.min.y + row as f32 * dot.y,
                );
                painter.rect_filled(
                    Rect::from_min_size(min, dot.max(Vec2::splat(1.5))),
                    0.0,
                    visuals.strong_text_color(),
                );
            }
        }
    }
    painter.text(
        Pos2::new(rect.center().x, rect.max.y - 2.0),
        egui::Align2::CENTER_BOTTOM,
        format!("{}", bar + 1),
        egui::FontId::proportional(11.0),
        visuals.text_color(),
    );
    response
}
