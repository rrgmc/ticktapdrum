//! The one row of buttons at the top: Play, the pattern, the bar, the tempo, the step levels,
//! Undo, Redo and the menu. Sounds is in the menu. See "One control strip" in `docs/DESIGN.md`.
//!
//! The row never wraps and never scrolls. It measures what fits: first an icon and a word on
//! every button, then icons only. If the icons still do not fit, Redo, then the tempo, then Undo
//! leave the row, and the menu offers them instead. The step levels button always stays.

use egui::{CornerRadius, Response, RichText, Vec2};

use dm_project::StepLevels;

use crate::icons;
use crate::widgets::{self, ICON_SIZE, TAP, TEXT_SIZE};

/// The size of the keyboard mark on Play while the drums go to a MIDI output.
const MIDI_MARK_SIZE: f32 = 13.0;

/// What Play plays.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Loop the pattern being edited.
    Pattern,
    /// Play the song.
    Song,
}

/// A tap on a button that needs no pop-up. The pattern, bar, tempo and menu buttons open
/// pop-ups instead; the caller anchors those to the responses in [`StripOutput`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StripEvent {
    Play,
    Stop,
    /// Show the screen of steps before this one.
    PrevPage,
    /// Show the screen of steps after this one.
    NextPage,
    /// Move to the next choice of step levels.
    StepLevels,
    Undo,
    Redo,
}

/// Where the grid is in the pattern, for the bar button.
#[derive(Clone, Copy)]
pub(crate) struct BarView {
    /// The first bar on screen, counted from 0.
    pub(crate) first: usize,
    /// How many bars are on screen. 1 when the grid shows half a bar.
    pub(crate) shown: usize,
    /// How many bars the pattern has.
    pub(crate) total: usize,
    /// When the grid shows half a bar: which half is on screen, 0 or 1.
    pub(crate) half: Option<usize>,
    /// The half of the shown bar the playhead is in, when it is in that bar.
    pub(crate) playing_half: Option<usize>,
    /// Whether there is a screen of steps before this one.
    pub(crate) can_prev: bool,
    /// Whether there is a screen of steps after this one.
    pub(crate) can_next: bool,
}

/// What the strip shows.
#[derive(Clone, Copy)]
pub(crate) struct StripState<'a> {
    pub(crate) playing: bool,
    pub(crate) mode: Mode,
    /// The letter of the pattern on the pattern button.
    pub(crate) pattern: char,
    /// That pattern is the one being heard.
    pub(crate) pattern_heard: bool,
    pub(crate) bars: BarView,
    pub(crate) bpm: f32,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
    /// Which loudness levels a tap on a step goes through.
    pub(crate) step_levels: StepLevels,
    /// The MIDI output the drums go to now, if any. Play then carries a small keyboard mark.
    pub(crate) midi_device: Option<&'a str>,
}

/// The buttons that did not fit. The menu offers them instead.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Hidden {
    pub(crate) redo: bool,
    pub(crate) tempo: bool,
    pub(crate) undo: bool,
}

pub(crate) struct StripOutput {
    pub(crate) events: Vec<StripEvent>,
    /// The pattern button, to anchor the pattern pop-up.
    pub(crate) pattern: Response,
    /// The middle of the bar button, to anchor the bar pop-up.
    pub(crate) bar_label: Response,
    /// The tempo button, to anchor the tempo pop-up. `None` when it did not fit.
    pub(crate) tempo: Option<Response>,
    /// The menu button, to anchor the menu pop-up.
    pub(crate) menu: Response,
    pub(crate) hidden: Hidden,
}

/// Sizes for one way of drawing the row.
#[derive(Clone, Copy)]
struct Metrics {
    /// Show a word beside each icon.
    words: bool,
    /// The narrowest a button may be.
    min_width: f32,
    /// The width of each arrow on the bar button.
    arrow_width: f32,
    /// The space between buttons.
    gap: f32,
}

const WITH_WORDS: Metrics = Metrics {
    words: true,
    min_width: TAP,
    arrow_width: 40.0,
    gap: 8.0,
};

/// Icons only, packed a little closer, so the row fits a phone 320 points wide. Buttons stay at
/// least 44 points wide, the size iOS asks for.
const ICONS_ONLY: Metrics = Metrics {
    words: false,
    min_width: 44.0,
    arrow_width: 34.0,
    gap: 4.0,
};

/// The texts on the buttons, for one way of drawing the row.
struct Texts {
    play: RichText,
    play_word: String,
    /// The play button shows a shorter word than `play_word`.
    play_short: bool,
    pattern: RichText,
    bar: RichText,
    tempo: RichText,
    levels: RichText,
    undo: RichText,
    redo: RichText,
    menu: RichText,
}

/// `short_play` shows the plain play word in song mode, which is shorter than "Play song".
fn texts(ui: &egui::Ui, state: &StripState, words: bool, short_play: bool) -> Texts {
    let word = |w: String| if words { w } else { String::new() };
    let play_word = match (state.playing, state.mode) {
        (true, _) => tr!("strip-stop"),
        (false, Mode::Song) => tr!("strip-play-song"),
        (false, Mode::Pattern) => tr!("strip-play"),
    };
    let play_short = short_play && !state.playing && state.mode == Mode::Song;
    let play_shown = if play_short {
        tr!("strip-play")
    } else {
        play_word.clone()
    };
    let play_icon = if state.playing {
        icons::STOP
    } else {
        icons::PLAY
    };
    let pattern = if words {
        format!("{} {}", state.pattern, icons::CARET_DOWN)
    } else {
        format!("{}{}", state.pattern, icons::CARET_DOWN)
    };
    let bar_size = if words { TEXT_SIZE } else { ICON_SIZE - 4.0 };
    let levels_word = match state.step_levels {
        StepLevels::OnOff => tr!("strip-levels-on-off"),
        StepLevels::Soft => tr!("strip-levels-soft"),
        StepLevels::LoudAndSoft => tr!("strip-levels-loud"),
    };
    // The icon is painted (see `paint_levels`); the spaces leave room for it.
    let levels = if words {
        RichText::new(format!("{}{levels_word}", levels_room(ui))).size(TEXT_SIZE)
    } else {
        RichText::new(LEVELS_ROOM_ICON_ONLY).size(ICON_SIZE)
    };
    Texts {
        play: widgets::label(play_icon, &word(play_shown)),
        play_word,
        play_short,
        pattern: RichText::new(pattern).size(ICON_SIZE - 2.0),
        bar: RichText::new(bar_text(&state.bars, words)).size(bar_size),
        tempo: widgets::label(icons::METRONOME, &format!("{:.0}", state.bpm)),
        levels,
        undo: widgets::label(icons::UNDO, &word(tr!("strip-undo"))),
        redo: widgets::label(icons::REDO, &word(tr!("strip-redo"))),
        menu: widgets::label(icons::LIST, &word(tr!("strip-menu"))),
    }
}

/// "Bar 1 of 4", "Bars 1-2 of 4", or with icons only "1/4" and "1-2/4".
fn bar_text(bars: &BarView, words: bool) -> String {
    let first = bars.first + 1;
    let last = bars.first + bars.shown.max(1);
    match (words, last > first) {
        (true, false) => tr!("strip-bar", first = first, total = bars.total),
        (true, true) => tr!("strip-bars", first = first, last = last, total = bars.total),
        (false, false) => format!("{first}/{}", bars.total),
        (false, true) => format!("{first}-{last}/{}", bars.total),
    }
}

/// How wide a button with this text is drawn.
fn width(ui: &egui::Ui, text: &RichText, min: f32) -> f32 {
    let galley = egui::WidgetText::from(text.clone()).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    // One point spare, so rounding never pushes the last button past the edge.
    (galley.size().x + 2.0 * ui.spacing().button_padding.x + 1.0).max(min)
}

/// How wide the row is when drawn with these texts and sizes, leaving out what is hidden.
fn row_width(ui: &egui::Ui, t: &Texts, m: Metrics, hidden: Hidden) -> f32 {
    let mut widths = vec![
        width(ui, &t.play, m.min_width),
        width(ui, &t.pattern, m.min_width),
        // The bar button's three parts touch, so they count as one.
        2.0 * m.arrow_width + width(ui, &t.bar, m.min_width - 4.0),
    ];
    if !hidden.tempo {
        widths.push(width(ui, &t.tempo, m.min_width));
    }
    widths.push(width(ui, &t.levels, m.min_width));
    if !hidden.undo {
        widths.push(width(ui, &t.undo, m.min_width));
    }
    if !hidden.redo {
        widths.push(width(ui, &t.redo, m.min_width));
    }
    widths.push(width(ui, &t.menu, m.min_width));
    widths.iter().sum::<f32>() + m.gap * (widths.len() - 1) as f32
}

/// Picks the first way of drawing the row that fits `available` points.
fn plan(ui: &egui::Ui, state: &StripState, available: f32) -> (Metrics, Texts, Hidden) {
    let full = texts(ui, state, true, false);
    if row_width(ui, &full, WITH_WORDS, Hidden::default()) <= available {
        return (WITH_WORDS, full, Hidden::default());
    }
    // In song mode "Play song" can be all that keeps the words from fitting.
    let short = texts(ui, state, true, true);
    if short.play_short && row_width(ui, &short, WITH_WORDS, Hidden::default()) <= available {
        return (WITH_WORDS, short, Hidden::default());
    }
    let icons = texts(ui, state, false, false);
    let steps = [
        Hidden::default(),
        Hidden {
            redo: true,
            ..Hidden::default()
        },
        Hidden {
            redo: true,
            tempo: true,
            undo: false,
        },
    ];
    for hidden in steps {
        if row_width(ui, &icons, ICONS_ONLY, hidden) <= available {
            return (ICONS_ONLY, icons, hidden);
        }
    }
    // The fewest buttons. On a screen narrower than any phone they run past the right edge
    // rather than wrap.
    let hidden = Hidden {
        redo: true,
        tempo: true,
        undo: true,
    };
    (ICONS_ONLY, icons, hidden)
}

/// Draws the strip. Taps that need no pop-up come back as events.
pub(crate) fn ui(ui: &mut egui::Ui, state: &StripState) -> StripOutput {
    let mut events = Vec::new();
    let (m, t, hidden) = plan(ui, state, ui.available_width());
    let size = Vec2::new(m.min_width, TAP);
    // The hover text only repeats the word the button would show with room for it.
    let hover = |response: Response, word: &str| {
        if m.words {
            response
        } else {
            response.on_hover_text(word)
        }
    };

    let output = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = m.gap;

        // Play and Stop. Space does the same when a keyboard is attached and no text field has
        // focus. It is only a shortcut: the button does the same on a touch screen.
        let space =
            ui.input(|i| i.key_pressed(egui::Key::Space)) && !ui.ctx().egui_wants_keyboard_input();
        let play = ui.add(
            egui::Button::new(t.play.clone())
                .min_size(size)
                .selected(state.playing),
        );
        // A shortened word gets the full one as hover text.
        let play = if t.play_short {
            play.on_hover_text(&t.play_word)
        } else {
            hover(play, &t.play_word)
        };
        // While the drums go to a MIDI output, a small keyboard sits in Play's corner, so the
        // silence of this device is not a surprise. It takes no room in the row.
        let play = match state.midi_device {
            Some(device) => {
                // A filled badge, so it reads at a glance in light and dark themes.
                let centre = play.rect.right_top() + Vec2::new(-10.0, 10.0);
                let selection = ui.visuals().selection;
                ui.painter()
                    .circle_filled(centre, MIDI_MARK_SIZE / 2.0 + 1.5, selection.bg_fill);
                ui.painter().text(
                    centre,
                    egui::Align2::CENTER_CENTER,
                    icons::PIANO_KEYS,
                    egui::FontId::proportional(MIDI_MARK_SIZE),
                    selection.stroke.color,
                );
                play.on_hover_text(tr!("strip-midi-tip", device = device))
            }
            None => play,
        };
        if play.clicked() || space {
            events.push(if state.playing {
                StripEvent::Stop
            } else {
                StripEvent::Play
            });
        }

        // The pattern. A dot marks it when it is the one being heard.
        let pattern = {
            let pattern = ui.add(egui::Button::new(t.pattern.clone()).min_size(size));
            if state.pattern_heard {
                let color = ui.visuals().warn_fg_color;
                ui.painter().circle_filled(
                    pattern.rect.right_top() + Vec2::new(-7.0, 7.0),
                    4.0,
                    color,
                );
            }
            hover(pattern, &tr!("strip-patterns"))
        };

        let bar_label = bar_button(ui, state, &t, m, &mut events);

        let tempo = (!hidden.tempo).then(|| {
            ui.add(egui::Button::new(t.tempo.clone()).min_size(size))
                .on_hover_text(tr!("strip-tempo-tip"))
        });

        {
            let levels = ui.add(egui::Button::new(t.levels.clone()).min_size(size));
            paint_levels(ui, &levels, state.step_levels, m.words);
            let explained = match state.step_levels {
                StepLevels::OnOff => tr!("strip-levels-tip-on-off"),
                StepLevels::Soft => tr!("strip-levels-tip-soft"),
                StepLevels::LoudAndSoft => tr!("strip-levels-tip-loud"),
            };
            let levels = levels.on_hover_text(explained);
            if levels.clicked() {
                events.push(StripEvent::StepLevels);
            }
        }

        if !hidden.undo {
            let undo = ui.add_enabled(
                state.can_undo,
                egui::Button::new(t.undo.clone()).min_size(size),
            );
            if hover(undo, &tr!("strip-undo")).clicked() {
                events.push(StripEvent::Undo);
            }
        }
        if !hidden.redo {
            let redo = ui.add_enabled(
                state.can_redo,
                egui::Button::new(t.redo.clone()).min_size(size),
            );
            if hover(redo, &tr!("strip-redo")).clicked() {
                events.push(StripEvent::Redo);
            }
        }

        let menu = hover(
            ui.add(egui::Button::new(t.menu.clone()).min_size(size)),
            &tr!("strip-menu"),
        );
        (pattern, bar_label, tempo, menu)
    });
    let (pattern, bar_label, tempo, menu) = output.inner;
    StripOutput {
        events,
        pattern,
        bar_label,
        tempo,
        menu,
        hidden,
    }
}

/// The bar button: a back arrow, the label, and a forward arrow, drawn touching so they read as
/// one button. Returns the label's response.
fn bar_button(
    ui: &mut egui::Ui,
    state: &StripState,
    t: &Texts,
    m: Metrics,
    events: &mut Vec<StripEvent>,
) -> Response {
    let bars = &state.bars;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let r = ui.visuals().widgets.inactive.corner_radius;
        let left = CornerRadius {
            nw: r.nw,
            sw: r.sw,
            ne: 0,
            se: 0,
        };
        let right = CornerRadius {
            nw: 0,
            sw: 0,
            ne: r.ne,
            se: r.se,
        };
        // The arrows wrap round: back from the first screen goes to the last, and forward from
        // the last goes to the first. An arrow about to wrap keeps its shape but turns the
        // highlight colour, so the jump is no surprise. A pattern of one bar shows no colour:
        // its two halves just take turns. With a single screen there is nowhere to go.
        let wrap_colour = ui.visuals().warn_fg_color;
        let arrow = |icon: &str, wraps: bool| {
            let mut text = RichText::new(icon).size(ICON_SIZE - 4.0);
            if wraps {
                text = text.color(wrap_colour);
            }
            egui::Button::new(text).min_size(Vec2::new(m.arrow_width, TAP))
        };
        let paging = bars.can_prev || bars.can_next;
        let colour_wraps = bars.total > 1;
        let back_hint = if bars.can_prev {
            tr!("strip-back")
        } else {
            tr!("strip-to-end")
        };
        if ui
            .add_enabled(
                paging,
                arrow(icons::CARET_LEFT, colour_wraps && !bars.can_prev).corner_radius(left),
            )
            .on_hover_text(back_hint)
            .clicked()
        {
            events.push(StripEvent::PrevPage);
        }
        let label = ui.add(
            egui::Button::new(t.bar.clone())
                .min_size(Vec2::new(m.min_width - 4.0, TAP))
                .corner_radius(CornerRadius::ZERO),
        );
        if let Some(half) = bars.half {
            half_dots(ui, &label, half, bars.playing_half);
        }
        let forward_hint = if bars.can_next {
            tr!("strip-forward")
        } else {
            tr!("strip-to-start")
        };
        if ui
            .add_enabled(
                paging,
                arrow(icons::CARET_RIGHT, colour_wraps && !bars.can_next).corner_radius(right),
            )
            .on_hover_text(forward_hint)
            .clicked()
        {
            events.push(StripEvent::NextPage);
        }
        label
    })
    .inner
}

/// Two small dots under the bar label, one for each half of the bar. The half on screen is
/// filled and the other is a ring. The half the playhead is in is lit, as the old half switch
/// did.
fn half_dots(ui: &egui::Ui, label: &Response, shown: usize, playing: Option<usize>) {
    let text = ui.style().interact(label).fg_stroke.color;
    let lit = ui.visuals().warn_fg_color;
    let y = label.rect.bottom() - 7.0;
    for half in 0..2 {
        let center = egui::pos2(label.rect.center().x + (half as f32 - 0.5) * 12.0, y);
        let color = if playing == Some(half) { lit } else { text };
        if half == shown {
            ui.painter().circle_filled(center, 3.5, color);
        } else {
            ui.painter()
                .circle_stroke(center, 3.0, egui::Stroke::new(1.5, color));
        }
    }
}

/// Spaces that make the step levels button wide enough for its painted icon when it has no
/// word. The icon is centred on the button.
const LEVELS_ROOM_ICON_ONLY: &str = "     ";

/// How far the painted icon starts from the left edge of a button with a word.
const LEVELS_LEFT: f32 = 12.0;
/// The width of one bar of the icon, and the space between two bars.
const LEVELS_BAR: f32 = 5.0;
const LEVELS_GAP: f32 = 3.0;
/// The width of the whole icon.
const LEVELS_WIDTH: f32 = 3.0 * LEVELS_BAR + 2.0 * LEVELS_GAP;
/// The space between the icon and the word.
const LEVELS_TO_WORD: f32 = 6.0;

/// Spaces that push the word on the step levels button past its painted icon. The count
/// comes from the width of a space in the font in use, which differs between fonts and
/// platforms, so a fixed count can leave the word on top of the icon.
fn levels_room(ui: &egui::Ui) -> String {
    const PROBE: usize = 10;
    let text = |s: String| RichText::new(s).size(TEXT_SIZE);
    let spaced = width(ui, &text(format!("x{}x", " ".repeat(PROBE))), 0.0);
    let bare = width(ui, &text("xx".to_owned()), 0.0);
    let space = ((spaced - bare) / PROBE as f32).max(1.0);
    let room = LEVELS_LEFT + LEVELS_WIDTH + LEVELS_TO_WORD - ui.spacing().button_padding.x;
    " ".repeat((room / space).ceil().max(1.0) as usize)
}

/// The step levels icon: three bars rising left to right, one filled for on and off, two for
/// soft, three for loud and soft. The others are drawn as outlines, so the count reads at a
/// glance. Painted, because the icon font's signal bars leave the empty ones out.
fn paint_levels(ui: &egui::Ui, button: &Response, levels: StepLevels, words: bool) {
    let filled = match levels {
        StepLevels::OnOff => 1,
        StepLevels::Soft => 2,
        StepLevels::LoudAndSoft => 3,
    };
    let color = ui.style().interact(button).fg_stroke.color;
    let (bar, gap) = (LEVELS_BAR, LEVELS_GAP);
    let left = if words {
        button.rect.left() + LEVELS_LEFT
    } else {
        button.rect.center().x - LEVELS_WIDTH / 2.0
    };
    let bottom = button.rect.center().y + 9.0;
    for i in 0..3 {
        let height = 8.0 + 5.0 * i as f32;
        let x = left + i as f32 * (bar + gap);
        let rect =
            egui::Rect::from_min_max(egui::pos2(x, bottom - height), egui::pos2(x + bar, bottom));
        if i < filled {
            ui.painter().rect_filled(rect, CornerRadius::same(1), color);
        } else {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(1),
                egui::Stroke::new(1.5, color),
                egui::StrokeKind::Inside,
            );
        }
    }
}
