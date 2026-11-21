//! The step grid: row names on the left, a range of one pattern's steps on the right.
//!
//! A step reacts when the finger or mouse button goes down, not when it lifts, so the sound is
//! immediate. Dragging paints: the first step touched decides whether the drag turns steps on or
//! off. The grid is never inside a scroll area, because a touch drag there would scroll instead.
//!
//! The grid draws from the project and reports what the user did; the caller changes the project.
//!
//! Under the project's rows come the drums on offer (see [`Offered`]): played on the MIDI
//! keyboard, with no row yet. They are drawn faded, with a plus sign, and their steps cannot be
//! tapped. A click on the name adds the drum as a row.

use crate::offered::Offered;
use crate::pictures::RowLabels;
use dm_project::{
    ACCENT_VELOCITY, DEFAULT_VELOCITY, Pattern, Project, SOFT_VELOCITY, STEPS_PER_BAR, StepLevels,
    next_level,
};
use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// The least width of the column of row names, with names showing.
const LABEL_WIDTH: f32 = 96.0;
/// The size of a name with names showing.
const NAME_SIZE: f32 = 16.0;
/// The most of the grid's width the column of names takes, so a phone keeps room for the steps.
const MAX_LABEL_SHARE: f32 = 0.25;
/// The same with pictures alone, which need less room and leave more for the steps.
const PICTURE_LABEL_WIDTH: f32 = 60.0;
/// The same with a picture and its name beside it, on a short row: wide enough for long names
/// such as Portuguese "Chimbal fechado". It is also the most the column takes with the name
/// under the picture.
const PICTURE_NAME_LABEL_WIDTH: f32 = 112.0;
/// The least the column takes with the name under the picture, so the picture keeps its size.
const PICTURE_NAME_MIN_WIDTH: f32 = 64.0;
/// The row height from which the name goes under its picture.
const NAME_UNDER_ROW: f32 = 44.0;
/// The row height from which a name under its picture may take two lines.
const TWO_LINE_ROW: f32 = 64.0;
/// Room beside a name for the mute or solo badge.
const BADGE_ROOM: f32 = 14.0;
/// The width of the area at the right of a drum on offer that takes it away.
const REMOVE_WIDTH: f32 = 24.0;
/// The size of a name beside or under its picture.
const PICTURE_NAME_SIZE: f32 = 12.0;
/// How long a row's name stays lit after its key is played on the MIDI keyboard, in seconds.
const FLASH_SECONDS: f64 = 0.3;
/// How faded a drum on offer is drawn.
const OFFERED_FADE: f32 = 0.45;
/// The size of the keyboard key's name shown in a row's name box.
const KEY_NAME_SIZE: f32 = 10.0;
/// Room kept beside a name for the keyboard key's name, while it shows.
const KEY_ROOM: f32 = 30.0;

/// The name of the keyboard key that sends `key`, as Yamaha keyboards name keys: middle C (60)
/// is C3, so the kick (36) is C1.
pub(crate) fn key_name(key: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = i32::from(key / 12) - 2;
    format!("{}{octave}", NAMES[usize::from(key % 12)])
}

/// How strong the highlight behind a drum on offer is.
const OFFERED_BAND: f32 = 0.22;

/// The width of the column of row names for this way of labelling `project`'s rows and the
/// drums on offer, `offered`, in a grid `width` points wide with rows `row_height` points tall
/// (see [`row_height`]).
///
/// The column is as wide as the longest name on one line needs, with room for a badge, within
/// limits: with names alone, from 96 points up to a quarter of the grid, so a wide window shows
/// whole names and a phone keeps its steps; with the name under the picture, from 64 to 112
/// points. With the name beside the picture it is 112 points, and pictures alone take 60.
pub(crate) fn label_width(
    ctx: &egui::Context,
    project: &Project,
    offered: &[u8],
    labels: RowLabels,
    width: f32,
    row_height: f32,
    show_keys: bool,
) -> f32 {
    let key_room = if show_keys { KEY_ROOM } else { 0.0 };
    let widest = |size: f32| {
        let font = FontId::proportional(size);
        project
            .rows()
            .iter()
            .map(|info| crate::drums::row_name(&info.name, info.key))
            .chain(offered.iter().map(|&key| crate::drums::drum_name(key)))
            .map(|name| {
                ctx.fonts_mut(|f| f.layout_no_wrap(name, font.clone(), Color32::WHITE))
                    .size()
                    .x
            })
            .fold(0.0, f32::max)
    };
    match labels {
        RowLabels::Pictures => PICTURE_LABEL_WIDTH,
        RowLabels::PicturesAndNames if row_height >= NAME_UNDER_ROW => {
            (widest(PICTURE_NAME_SIZE) + 6.0 + BADGE_ROOM + 4.0)
                .clamp(PICTURE_NAME_MIN_WIDTH, PICTURE_NAME_LABEL_WIDTH)
        }
        RowLabels::PicturesAndNames => PICTURE_NAME_LABEL_WIDTH,
        RowLabels::Names => (widest(NAME_SIZE) + 18.0 + BADGE_ROOM + key_room + 4.0)
            .clamp(LABEL_WIDTH, (width * MAX_LABEL_SHARE).max(LABEL_WIDTH)),
    }
}

/// How tall each of `rows` rows is in a grid `height` points tall, header included.
pub(crate) fn row_height(height: f32, rows: usize) -> f32 {
    ((height - HEADER_HEIGHT) / rows.max(1) as f32).clamp(8.0, MAX_ROW_HEIGHT)
}
const HEADER_HEIGHT: f32 = 26.0;
const GAP: f32 = 3.0;
/// Rows always fit the height there is, because the grid never scrolls. On a short landscape
/// phone in song mode that makes them shorter than a fingertip; scrolling rows is a later step.
const MAX_ROW_HEIGHT: f32 = 72.0;

/// What the user did in the grid this frame.
pub(crate) enum GridEvent {
    /// Give a step this velocity; 0 turns it off.
    Set {
        row: usize,
        step: usize,
        velocity: u8,
    },
    /// Play a row's drum once.
    Preview { row: usize },
    /// Open the settings of a row beside its name. Comes with `Preview` when a name is tapped.
    /// `rect` is the name's place on the screen, for the pop-up to sit beside.
    OpenRow { row: usize, rect: Rect },
    /// Add the drum on offer with this key as a row.
    AddOffered { key: u8 },
    /// Take the drum on offer with this key away, with the notes recorded on it.
    RemoveOffered { key: u8 },
}

/// Which steps the grid shows.
#[derive(Clone, Copy)]
pub(crate) struct View {
    pub(crate) first_step: usize,
    pub(crate) visible: usize,
}

/// State that lives across frames: the value a drag paints, and which press it belongs to.
#[derive(Default)]
pub(crate) struct Grid {
    paint: Option<u8>,
    /// Counts presses on the grid, so one drag's edits undo together.
    gesture: u64,
    /// Which loudness levels a tap on a step goes through.
    step_levels: StepLevels,
    /// The level the last left click put on a step. A right click puts this level on a step, or
    /// clears a step that already has it. `None` means the normal level.
    last_level: Option<u8>,
    /// How the row names show: names, pictures and names, or pictures.
    row_labels: RowLabels,
    /// While set, the steps ignore presses. A pop-up is open, and a tap outside it only closes it.
    blocked: bool,
    /// While set, the row names ignore presses too. Clear while only a row's pop-up is open, so
    /// a tap on another row's name moves the pop-up there at once.
    names_blocked: bool,
    /// The row whose settings are open, drawn with its name lit.
    highlight: Option<usize>,
    /// Where each row's name was drawn on the last frame.
    label_rects: Vec<Rect>,
    /// The keys played on the MIDI keyboard lately, each with when (egui's clock, in seconds).
    /// Their rows' names light up for [`FLASH_SECONDS`].
    flashes: Vec<(u8, f64)>,
    /// Whether each name box shows the keyboard key that plays its drum.
    show_keys: bool,
}

struct Layout {
    labels: Rect,
    cells: Rect,
    view: View,
    cell: Vec2,
}

impl Layout {
    fn cell_rect(&self, row: usize, column: usize) -> Rect {
        let min = self.cells.min + Vec2::new(column as f32 * self.cell.x, row as f32 * self.cell.y);
        Rect::from_min_size(min, self.cell).shrink(GAP / 2.0)
    }

    fn label_rect(&self, row: usize) -> Rect {
        let min = self.labels.min + Vec2::new(0.0, row as f32 * self.cell.y);
        Rect::from_min_size(min, Vec2::new(self.labels.width(), self.cell.y)).shrink(GAP / 2.0)
    }

    fn hit(&self, pos: Pos2, rows: usize) -> Option<(usize, usize)> {
        if !self.cells.contains(pos) {
            return None;
        }
        let rel = pos - self.cells.min;
        let column = (rel.x / self.cell.x) as usize;
        let row = (rel.y / self.cell.y) as usize;
        (row < rows && column < self.view.visible).then_some((row, column))
    }
}

impl Grid {
    /// The number of the current or most recent press on the grid.
    pub(crate) fn gesture(&self) -> u64 {
        self.gesture
    }

    /// Makes the steps ignore presses, and the row names too when `names` is set, until this is
    /// called again. The caller sets it every frame before [`Grid::ui`], to whether a pop-up is
    /// open, so a tap outside a pop-up closes it without also changing a step. While only a row's
    /// pop-up is open, the names stay live, so a tap on another name opens that row at once.
    pub(crate) fn set_blocked(&mut self, steps: bool, names: bool) {
        self.blocked = steps;
        self.names_blocked = names;
    }

    /// Sets which loudness levels a tap on a step goes through. The caller sets it every frame
    /// before [`Grid::ui`]: the project's choice.
    pub(crate) fn set_step_levels(&mut self, levels: StepLevels) {
        if levels != self.step_levels {
            // The level a right click repeats may not exist in the new choice.
            self.last_level = None;
        }
        self.step_levels = levels;
    }

    /// Sets how the row names show. The caller sets it every frame before [`Grid::ui`].
    pub(crate) fn set_row_labels(&mut self, labels: RowLabels) {
        self.row_labels = labels;
    }

    /// How the row names show, for the drum list in a row's pop-up.
    pub(crate) fn row_labels(&self) -> RowLabels {
        self.row_labels
    }

    /// Shows in each name box the keyboard key that plays its drum, or stops. The caller sets it
    /// every frame before [`Grid::ui`]: on while MIDI in is on and the "Keyboard notes" setting
    /// is on.
    pub(crate) fn set_show_keys(&mut self, show: bool) {
        self.show_keys = show;
    }

    /// Lights the name of the row whose settings are open, or none.
    pub(crate) fn set_highlight(&mut self, row: Option<usize>) {
        self.highlight = row;
    }

    /// Lights the name of the row, real or on offer, that plays `key`, from `now` (egui's clock)
    /// for [`FLASH_SECONDS`].
    pub(crate) fn flash(&mut self, key: u8, now: f64) {
        self.flashes.retain(|&(k, _)| k != key);
        self.flashes.push((key, now));
    }

    /// How lit the name of the row playing `key` is at `now`: 1 just after the key is played,
    /// falling to 0.
    fn flash_at(&self, key: u8, now: f64) -> f32 {
        self.flashes
            .iter()
            .find(|&&(k, _)| k == key)
            .map(|&(_, at)| (1.0 - (now - at) / FLASH_SECONDS).clamp(0.0, 1.0) as f32)
            .unwrap_or(0.0)
    }

    /// Where a row's name was drawn on the last frame, in screen points.
    pub(crate) fn label_rect(&self, row: usize) -> Option<Rect> {
        self.label_rects.get(row).copied()
    }

    /// Draws the grid in the rest of `ui` and returns what the user did.
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        offered: &Offered,
        pattern: &Pattern,
        view: View,
        playhead: Option<usize>,
    ) -> Vec<GridEvent> {
        let area = ui.available_rect_before_wrap();
        let now = ui.input(|i| i.time);
        self.flashes.retain(|&(_, at)| now - at < FLASH_SECONDS);
        if !self.flashes.is_empty() {
            ui.ctx().request_repaint();
        }
        let rows = (project.rows().len() + offered.len()).max(1);
        let label_width = label_width(
            ui.ctx(),
            project,
            &offered.keys(),
            self.row_labels,
            area.width(),
            row_height(area.height(), rows),
            self.show_keys,
        );
        let header = Rect::from_min_max(
            Pos2::new(area.min.x + label_width, area.min.y),
            Pos2::new(area.max.x, area.min.y + HEADER_HEIGHT),
        );
        let body_top = header.max.y;
        let cell = Vec2::new(
            (area.width() - label_width) / view.visible as f32,
            ((area.max.y - body_top) / rows as f32).clamp(8.0, MAX_ROW_HEIGHT),
        );
        let layout = Layout {
            labels: Rect::from_min_size(
                Pos2::new(area.min.x, body_top),
                Vec2::new(label_width, cell.y * rows as f32),
            ),
            cells: Rect::from_min_size(
                Pos2::new(area.min.x + label_width, body_top),
                Vec2::new(cell.x * view.visible as f32, cell.y * rows as f32),
            ),
            view,
            cell,
        };

        let mut events = Vec::new();
        self.labels(ui, project, &layout, now, &mut events);
        let real_rows = project.rows().len();
        self.offered_labels(ui, real_rows, offered, &layout, now, &mut events);
        self.input(
            ui,
            pattern,
            &layout,
            real_rows,
            self.step_levels,
            &mut events,
        );
        paint(ui, project, offered, pattern, &layout, header, playhead);
        ui.allocate_rect(
            Rect::from_min_max(area.min, Pos2::new(area.max.x, layout.cells.max.y)),
            Sense::hover(),
        );
        events
    }

    /// Row names. Tapping one plays its drum and opens its settings.
    fn labels(
        &mut self,
        ui: &mut egui::Ui,
        project: &Project,
        layout: &Layout,
        now: f64,
        events: &mut Vec<GridEvent>,
    ) {
        self.label_rects.clear();
        for (row, info) in project.rows().iter().enumerate() {
            let rect = layout.label_rect(row);
            self.label_rects.push(rect);
            let id = egui::Id::unique(("dm-grid-label", row));
            let response = ui.interact(rect, id, Sense::click());
            let down = !self.names_blocked && response.is_pointer_button_down_on();
            if down && ui.input(|i| i.pointer.any_pressed()) {
                events.push(GridEvent::Preview { row });
                events.push(GridEvent::OpenRow { row, rect });
            }
            let lit = self.highlight == Some(row);
            let fill = if down || lit {
                ui.visuals().widgets.active.bg_fill
            } else {
                ui.visuals().widgets.inactive.bg_fill
            };
            let family = crate::widgets::family_colour(
                dm_project::drum_family(info.key),
                ui.visuals().dark_mode,
            );
            let fill = flashed(fill, family, self.flash_at(info.key, now));
            ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
            // A row that mute or solo silences is drawn faded, and says why.
            let audible = project.audible(row);
            let fade = |c: Color32| if audible { c } else { c.gamma_multiply(0.4) };
            let colour = fade(family);
            let text_color = fade(ui.visuals().text_color());
            let badge = if info.mute {
                Some(tr!("rows-badge-mute"))
            } else if info.solo {
                Some(tr!("rows-badge-solo"))
            } else {
                None
            };
            let badge_room = if badge.is_some() { BADGE_ROOM } else { 0.0 };
            let key_room = self.draw_key(ui, rect, info.key, badge_room, text_color);
            draw_label(
                ui,
                rect,
                &crate::drums::row_name(&info.name, info.key),
                info.key,
                colour,
                text_color,
                self.row_labels,
                badge_room + key_room,
            );
            if lit {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(6),
                    Stroke::new(2.0, ui.visuals().selection.stroke.color),
                    StrokeKind::Inside,
                );
            }
            if let Some(badge) = badge {
                ui.painter().text(
                    rect.right_center() - Vec2::new(6.0, 0.0),
                    Align2::RIGHT_CENTER,
                    badge,
                    FontId::proportional(13.0),
                    ui.visuals().warn_fg_color,
                );
            }
        }
    }

    /// Draws the keyboard key that plays `key` in its name box `rect`, while keys show, and
    /// returns the room it takes beside the name. Beside a name it sits on the right, left of
    /// `right` points kept for badges. Over a picture with no name beside it, it sits in the top
    /// left corner and takes no room.
    fn draw_key(&self, ui: &egui::Ui, rect: Rect, key: u8, right: f32, text: Color32) -> f32 {
        if !self.show_keys {
            return 0.0;
        }
        let colour = text.gamma_multiply(0.75);
        let font = FontId::proportional(KEY_NAME_SIZE);
        let beside = match self.row_labels {
            RowLabels::Names => true,
            RowLabels::PicturesAndNames => rect.height() < NAME_UNDER_ROW,
            RowLabels::Pictures => false,
        };
        if beside {
            let x = rect.right() - 6.0 - right - if right > 0.0 { 2.0 } else { 0.0 };
            let used = ui
                .painter()
                .text(
                    Pos2::new(x, rect.center().y),
                    Align2::RIGHT_CENTER,
                    key_name(key),
                    font,
                    colour,
                )
                .width();
            used + 6.0
        } else {
            ui.painter().text(
                rect.left_top() + Vec2::new(4.0, 2.0),
                Align2::LEFT_TOP,
                key_name(key),
                font,
                colour,
            );
            0.0
        }
    }

    /// The names of the drums on offer, under the project's `first` rows: faded, with a plus
    /// sign and a cross. A click on the cross takes the drum away; anywhere else adds it as a row.
    fn offered_labels(
        &mut self,
        ui: &mut egui::Ui,
        first: usize,
        offered: &Offered,
        layout: &Layout,
        now: f64,
        events: &mut Vec<GridEvent>,
    ) {
        for (i, drum) in offered.rows().iter().enumerate() {
            let rect = layout.label_rect(first + i);
            // A faint highlight behind the whole row, name and steps, drawn first so both sit
            // on it. It sets the drums on offer apart from the project's rows.
            let band = Rect::from_min_max(
                Pos2::new(layout.labels.min.x, rect.min.y - GAP / 2.0),
                Pos2::new(layout.cells.max.x, rect.max.y + GAP / 2.0),
            );
            ui.painter().rect_filled(
                band,
                CornerRadius::same(6),
                ui.visuals().selection.bg_fill.gamma_multiply(OFFERED_BAND),
            );
            let width = REMOVE_WIDTH.min(rect.width() / 2.0);
            let remove_rect = Rect::from_min_max(
                egui::pos2(rect.right() - width, rect.top()),
                rect.right_bottom(),
            );
            let add_rect = Rect::from_min_max(
                rect.left_top(),
                egui::pos2(remove_rect.left(), rect.bottom()),
            );
            let id = egui::Id::unique(("dm-grid-offered", drum.key));
            let response = ui
                .interact(add_rect, id, Sense::click())
                .on_hover_text(tr!("rows-add-offered"));
            let remove = ui
                .interact(
                    remove_rect,
                    egui::Id::unique(("dm-grid-offered-remove", drum.key)),
                    Sense::click(),
                )
                .on_hover_text(tr!("rows-remove-offered"));
            if !self.names_blocked && response.clicked() {
                events.push(GridEvent::AddOffered { key: drum.key });
            }
            if !self.names_blocked && remove.clicked() {
                events.push(GridEvent::RemoveOffered { key: drum.key });
            }
            let hovered = !self.names_blocked && (response.hovered() || remove.hovered());
            let family = crate::widgets::family_colour(
                dm_project::drum_family(drum.key),
                ui.visuals().dark_mode,
            );
            let fill = if hovered {
                ui.visuals().widgets.hovered.bg_fill
            } else {
                ui.visuals().widgets.inactive.bg_fill
            }
            .gamma_multiply(OFFERED_FADE);
            let fill = flashed(fill, family, self.flash_at(drum.key, now));
            ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
            let fade = |c: Color32| {
                if hovered {
                    c
                } else {
                    c.gamma_multiply(OFFERED_FADE)
                }
            };
            let text_color = fade(ui.visuals().text_color());
            // Right of the name come the key, the plus sign and the cross.
            let key_room = self.draw_key(ui, rect, drum.key, BADGE_ROOM + width, text_color);
            draw_label(
                ui,
                rect,
                &crate::drums::drum_name(drum.key),
                drum.key,
                fade(family),
                text_color,
                self.row_labels,
                BADGE_ROOM + width + key_room,
            );
            // The cross takes the right edge; the plus sign sits left of it. Over a picture
            // alone they go in the top and bottom right corners.
            let (plus_at, plus_align, cross_at, cross_align) =
                if self.row_labels == RowLabels::Pictures {
                    (
                        rect.right_top() + Vec2::new(-3.0, 1.0),
                        Align2::RIGHT_TOP,
                        rect.right_bottom() + Vec2::new(-3.0, -1.0),
                        Align2::RIGHT_BOTTOM,
                    )
                } else {
                    (
                        egui::pos2(remove_rect.left() - 2.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        remove_rect.center(),
                        Align2::CENTER_CENTER,
                    )
                };
            ui.painter().text(
                plus_at,
                plus_align,
                crate::icons::PLUS,
                FontId::proportional(13.0),
                text_color,
            );
            let cross_color = if remove.hovered() && !self.names_blocked {
                ui.visuals().strong_text_color()
            } else {
                text_color
            };
            ui.painter().text(
                cross_at,
                cross_align,
                crate::icons::X,
                FontId::proportional(13.0),
                cross_color,
            );
        }
    }

    /// Toggling and painting steps. Reads raw pointer state so a step reacts on press.
    fn input(
        &mut self,
        ui: &mut egui::Ui,
        pattern: &Pattern,
        layout: &Layout,
        rows: usize,
        levels: StepLevels,
        events: &mut Vec<GridEvent>,
    ) {
        let id = egui::Id::unique("dm-grid-cells");
        let response = ui.interact(layout.cells, id, Sense::click_and_drag());
        let (pressed, secondary, down, pos) = ui.input(|i| {
            (
                i.pointer.any_pressed(),
                i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.any_down(),
                i.pointer.interact_pos(),
            )
        });
        if !down || self.blocked {
            // A press while blocked only closes a pop-up, and the drag it starts paints nothing.
            self.paint = None;
            return;
        }
        let Some((row, column)) = pos.and_then(|p| layout.hit(p, rows)) else {
            return;
        };
        let step = layout.view.first_step + column;
        let velocity = pattern.velocity(row, step);
        if pressed && response.is_pointer_button_down_on() {
            // The first step touched decides what the whole drag writes. A left click gives it
            // its next level and remembers that level. A right click gives it the remembered
            // level, or clears it when it already has that level.
            let value = if secondary {
                let last = self.last_level.unwrap_or(DEFAULT_VELOCITY);
                if velocity == last { 0 } else { last }
            } else {
                let next = next_level(velocity, levels);
                if next > 0 {
                    self.last_level = Some(next);
                }
                next
            };
            self.paint = Some(value);
            self.gesture += 1;
        }
        if let Some(value) = self.paint
            && velocity != value
        {
            events.push(GridEvent::Set {
                row,
                step,
                velocity: value,
            });
        }
    }
}

/// A row name's fill, lit by a flash of the row's family colour as strong as `flash`, 0 to 1.
fn flashed(fill: Color32, family: Color32, flash: f32) -> Color32 {
    if flash > 0.0 {
        fill.lerp_to_gamma(family, 0.8 * flash)
    } else {
        fill
    }
}

fn paint(
    ui: &egui::Ui,
    project: &Project,
    offered: &Offered,
    pattern: &Pattern,
    layout: &Layout,
    header: Rect,
    playhead: Option<usize>,
) {
    let painter = ui.painter();
    let visuals = ui.visuals();
    let view = layout.view;

    // The playhead: a band behind the column being heard, drawn first so steps sit on top.
    if let Some(step) = playhead
        && (view.first_step..view.first_step + view.visible).contains(&step)
    {
        let x = layout.cells.min.x + (step - view.first_step) as f32 * layout.cell.x;
        let band = Rect::from_min_size(
            Pos2::new(x, header.min.y),
            Vec2::new(layout.cell.x, layout.cells.max.y - header.min.y),
        );
        painter.rect_filled(
            band,
            CornerRadius::same(6),
            visuals.warn_fg_color.gamma_multiply(0.35),
        );
    }

    // Each row's lit steps take its drum family's colour.
    let on: Vec<Color32> = project
        .rows()
        .iter()
        .map(|row| {
            crate::widgets::family_colour(dm_project::drum_family(row.key), visuals.dark_mode)
        })
        .collect();
    let bar_line = Stroke::new(2.0, visuals.strong_text_color().gamma_multiply(0.6));

    for column in 0..view.visible {
        let step = view.first_step + column;
        let x = layout.cells.min.x + column as f32 * layout.cell.x;
        if step.is_multiple_of(4) {
            // Beat numbers; the first beat of a bar also names the bar when several show.
            let beat = (step % STEPS_PER_BAR) / 4 + 1;
            let text = if beat == 1 && view.visible > STEPS_PER_BAR {
                tr!("bars-header", number = step / STEPS_PER_BAR + 1)
            } else {
                format!("{beat}")
            };
            painter.text(
                Pos2::new(x + 6.0, header.center().y),
                Align2::LEFT_CENTER,
                text,
                FontId::proportional(15.0),
                visuals.strong_text_color(),
            );
        }
        if step.is_multiple_of(STEPS_PER_BAR) && column > 0 {
            painter.vline(x, layout.cells.y_range(), bar_line);
        }
        for (row, &colour) in on.iter().enumerate() {
            let velocity = pattern.velocity(row, step);
            draw_step(
                ui,
                layout.cell_rect(row, column),
                step,
                velocity,
                colour,
                1.0,
            );
        }
        // The drums on offer: faint, with the notes recorded on them in this pattern.
        for (i, drum) in offered.rows().iter().enumerate() {
            let colour =
                crate::widgets::family_colour(dm_project::drum_family(drum.key), visuals.dark_mode);
            let velocity = drum.velocity(pattern.slot(), step);
            let rect = layout.cell_rect(on.len() + i, column);
            draw_step(ui, rect, step, velocity, colour, OFFERED_FADE);
        }
    }
}

/// One step's cell, with its loudness, everything faded by `fade` (1 for none).
fn draw_step(ui: &egui::Ui, rect: Rect, step: usize, velocity: u8, colour: Color32, fade: f32) {
    let painter = ui.painter();
    let visuals = ui.visuals();
    let off_a = visuals.widgets.inactive.bg_fill;
    let off_b = visuals
        .faint_bg_color
        .lerp_to_gamma(visuals.widgets.inactive.bg_fill, 0.4);
    let fill = if (step / 4).is_multiple_of(2) {
        off_a
    } else {
        off_b
    };
    painter.rect_filled(rect, CornerRadius::same(4), fill.gamma_multiply(fade));
    if velocity > 0 {
        // Loudness shows as how much of the cell is filled, and how bright: a soft step fills
        // the lower half, faded; a loud one is full and carries a light top edge. Shape as well
        // as colour, so it reads without colour vision.
        let (share, brightness) = if velocity <= SOFT_VELOCITY {
            (0.5, 0.6)
        } else {
            (1.0, 1.0)
        };
        let height = rect.height() * share;
        let lit = Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - height), rect.max);
        painter.rect_filled(
            lit,
            CornerRadius::same(4),
            colour.gamma_multiply(brightness * fade),
        );
        if velocity >= ACCENT_VELOCITY {
            let edge = Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width(), 4.0_f32.min(rect.height() * 0.2)),
            );
            painter.rect_filled(
                edge,
                CornerRadius::same(2),
                visuals.strong_text_color().gamma_multiply(fade),
            );
        }
    }
    if step.is_multiple_of(4) {
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, Color32::from_gray(128).gamma_multiply(fade)),
            StrokeKind::Inside,
        );
    }
}

/// A row's label: the name with a stripe in the family's colour, a picture with the name, or a
/// picture alone.
#[allow(clippy::too_many_arguments)] // A drawing helper; its inputs are all plain values.
fn draw_label(
    ui: &egui::Ui,
    rect: Rect,
    name: &str,
    key: u8,
    colour: Color32,
    text: Color32,
    labels: RowLabels,
    badge_room: f32,
) {
    let painter = ui.painter();
    // Long names, common in Portuguese, wrap onto a second line where there is height for it,
    // and end in "…" where they still do not fit.
    let fitted = |size: f32, width: f32, height: f32| {
        let font = FontId::proportional(size);
        let line = ui.fonts_mut(|f| f.row_height(&font));
        let mut job = egui::text::LayoutJob::simple(name.to_owned(), font, text, width.max(1.0));
        job.wrap.max_rows = (height / line).floor().clamp(1.0, 2.0) as usize;
        job.wrap.overflow_character = Some('…');
        painter.layout_job(job)
    };
    match labels {
        RowLabels::Names => {
            let inset = 6.0_f32.min(rect.height() * 0.2);
            let stripe = Rect::from_min_size(
                rect.min + Vec2::new(4.0, inset),
                Vec2::new(4.0, rect.height() - 2.0 * inset),
            );
            painter.rect_filled(stripe, CornerRadius::same(2), colour);
            let galley = fitted(
                NAME_SIZE,
                rect.width() - 18.0 - badge_room,
                rect.height() - 4.0,
            );
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(Pos2::new(rect.left() + 14.0, top), galley, text);
        }
        RowLabels::PicturesAndNames if rect.height() >= NAME_UNDER_ROW => {
            // A tall row: the picture above, the name under it on up to two lines. The name is
            // laid out first, and the picture takes the height left, so a long name is not cut.
            let line = ui.fonts_mut(|f| f.row_height(&FontId::proportional(PICTURE_NAME_SIZE)));
            let name_room = if rect.height() >= TWO_LINE_ROW {
                2.0 * line
            } else {
                line
            };
            let galley = fitted(
                PICTURE_NAME_SIZE,
                rect.width() - 6.0 - badge_room,
                name_room,
            );
            let size = (rect.height() - galley.size().y - 8.0)
                .min(rect.width() - 8.0)
                .max(8.0);
            let picture = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.min.y + 3.0 + size / 2.0),
                Vec2::splat(size),
            );
            crate::pictures::paint(painter, picture, key, colour, text);
            let at = Pos2::new(
                rect.center().x - galley.size().x / 2.0,
                rect.max.y - 3.0 - galley.size().y,
            );
            painter.galley(at, galley, text);
        }
        RowLabels::PicturesAndNames => {
            // A short row: the picture to the left, the name beside it.
            let size = (rect.height() - 4.0).max(8.0);
            let picture = Rect::from_min_size(
                Pos2::new(rect.min.x + 3.0, rect.center().y - size / 2.0),
                Vec2::splat(size),
            );
            crate::pictures::paint(painter, picture, key, colour, text);
            let width = rect.max.x - picture.max.x - 6.0 - badge_room;
            let galley = fitted(PICTURE_NAME_SIZE, width, rect.height() - 2.0);
            let top = rect.center().y - galley.size().y / 2.0;
            painter.galley(Pos2::new(picture.max.x + 4.0, top), galley, text);
        }
        RowLabels::Pictures => {
            // On a short row, as on a phone in landscape, the row's height is what limits the
            // picture, so it keeps only a thin margin there.
            let margin = if rect.height() < 40.0 { 1.0 } else { 4.0 };
            let picture = rect.shrink(margin);
            crate::pictures::paint(painter, picture, key, colour, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::key_name;

    #[test]
    fn keys_are_named_as_yamaha_names_them() {
        assert_eq!(key_name(60), "C3");
        assert_eq!(key_name(36), "C1");
        assert_eq!(key_name(38), "D1");
        assert_eq!(key_name(42), "F#1");
        assert_eq!(key_name(24), "C0");
        assert_eq!(key_name(23), "B-1");
        assert_eq!(key_name(0), "C-2");
        assert_eq!(key_name(127), "G8");
    }
}
