//! The bar strip above the grid: one thumbnail per bar, and the bar commands.
//!
//! The thumbnails and commands open in a pop-up from the bar label in the top strip. Tapping a
//! thumbnail shows that bar in the grid and leaves the pop-up open. The thumbnails wrap onto
//! more lines. A setting can also show the thumbnails all the time, under the strip, on one line
//! that scrolls sideways.
//!
//! "Remove bar" removes the first bar the grid shows. It asks first, in a window of its own,
//! and then deletes the bar with its notes; later bars move back one. Nothing is kept hidden for later: a bar that is gone is gone, which is easier to
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
    /// Delete this bar and its notes. Sent once the user has said yes.
    RemoveBar(usize),
    /// Copy the first bar shown in the grid.
    CopyBar,
    ToggleFollow,
    /// Play the pattern from the first step of this bar.
    PlayFrom(usize),
    /// Play only this bar, over and over; or, if it is already the only bar playing, the whole
    /// pattern again.
    ToggleLoop(usize),
}

#[derive(Default)]
pub(crate) struct BarStrip {
    /// "Remove bar" asks first, in a window of its own (see [`BarStrip::confirm`]). Holds the
    /// bar asked about, counting from 0, and the pattern's bar count when it was asked.
    asking_remove: Option<(usize, usize)>,
    /// The first bar the grid showed last frame, so the line of thumbnails scrolls to it only
    /// when it changes and leaves a sideways scroll by hand alone otherwise.
    last_shown: Option<usize>,
    /// The only bar that plays, if one is chosen. The caller sets it every frame before drawing;
    /// its thumbnail carries a repeat mark.
    pub(crate) looped: Option<usize>,
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
                let looping = (self.looped, can_play_from);
                thumbnails(ui, pattern, &shown, playing_bar, looping, None, &mut events);
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
                    widgets::button(icons::TRASH, &tr!("bars-remove-bar")),
                )
                .clicked()
            {
                // Asked in a window of its own, so the pop-up closes.
                self.asking_remove = Some((shown.start, pattern.bars()));
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
                        let looping = (self.looped, can_play_from);
                        thumbnails(
                            ui,
                            pattern,
                            &shown,
                            playing_bar,
                            looping,
                            scroll_to,
                            &mut events,
                        );
                    });
                });
        });
        events
    }

    /// The "Remove bar?" question, in a window of its own. Call it every frame; it shows only
    /// while the question is asked, and only while `bars`, the pattern's bar count, is still what
    /// it was when asked.
    pub(crate) fn confirm(&mut self, ctx: &egui::Context, bars: usize) -> Vec<BarEvent> {
        let mut events = Vec::new();
        let Some((bar, _)) = self
            .asking_remove
            .filter(|&(b, n)| n == bars && n > 1 && b < n)
        else {
            self.asking_remove = None;
            return events;
        };
        match widgets::confirm(
            ctx,
            "remove-bar",
            &tr!("bars-remove-question", number = bar + 1),
            &tr!("bars-remove"),
            &tr!("bars-keep"),
        ) {
            Some(true) => {
                events.push(BarEvent::RemoveBar(bar));
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
    (looped, can_play): (Option<usize>, bool),
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
            looped == Some(bar),
        );
        if scroll_to == Some(bar) {
            response.scroll_to_me(None);
        }
        // A double click plays from the bar; a right click, or a long press, plays only it.
        // Only a pattern plays from a bar, so the song offers neither.
        if can_play && response.double_clicked() {
            events.push(BarEvent::PlayFrom(bar));
        } else if can_play && (response.secondary_clicked() || response.long_touched()) {
            events.push(BarEvent::ToggleLoop(bar));
        } else if response.clicked() {
            events.push(BarEvent::Select(bar));
        }
        let tip = if !can_play {
            None
        } else if looped == Some(bar) {
            Some(tr!("bars-thumb-looped-tip"))
        } else {
            Some(tr!("bars-thumb-tip"))
        };
        if let Some(tip) = tip {
            response.on_hover_text(tip);
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
    looped: bool,
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
    if looped {
        // The only bar that plays: a repeat mark in the corner, in the accent colour.
        painter.text(
            Pos2::new(rect.max.x - 3.0, rect.max.y - 1.0),
            egui::Align2::RIGHT_BOTTOM,
            icons::REPEAT,
            egui::FontId::proportional(13.0),
            visuals.selection.stroke.color,
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.5, visuals.selection.bg_fill),
            StrokeKind::Inside,
        );
    }
    response
}

/// How tall the bar indicator at the bottom of the screen is.
pub(crate) const INDICATOR_HEIGHT: f32 = 4.0;
/// The space between the grid and the bar indicator.
pub(crate) const INDICATOR_GAP: f32 = 6.0;

/// Paints the bar indicator in `rect`: one short line per bar of the pattern, side by side, with
/// the `marked` bars in the accent colour. The caller marks the bar being heard, or the bars the
/// grid shows while the pattern is not playing. It shows where the music is in a long pattern in
/// a few points of height.
pub(crate) fn indicator(ui: &egui::Ui, rect: Rect, total: usize, marked: std::ops::Range<usize>) {
    let total = total.max(1);
    let visuals = ui.visuals();
    // The other bars are a faint shade of the text colour, so they show against the background
    // in both themes; a button's grey is too close to it. The marked bars have the selection's
    // outline colour: dark blue on a light theme, light blue on a dark one.
    let off = visuals.text_color().gamma_multiply(0.3);
    let on = visuals.selection.stroke.color;
    let gap = (rect.width() / total as f32 * 0.2).min(3.0);
    let width = (rect.width() - gap * (total - 1) as f32) / total as f32;
    for bar in 0..total {
        let left = rect.left() + bar as f32 * (width + gap);
        let segment =
            Rect::from_min_size(Pos2::new(left, rect.top()), Vec2::new(width, rect.height()));
        let colour = if marked.contains(&bar) { on } else { off };
        ui.painter()
            .rect_filled(segment, CornerRadius::same(2), colour);
    }
}
