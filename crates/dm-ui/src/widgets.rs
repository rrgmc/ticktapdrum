//! Shared pieces of the one-strip layout: buttons with an icon, small pop-ups that open under a
//! button, and larger dialogs in the middle of the screen. See "One control strip" in
//! `docs/DESIGN.md`.
//!
//! Both kinds of pop-up close with a tap outside them. Inside one, `ui.close()` closes it too,
//! for example after a choice that needs nothing more.

use egui::{Popup, PopupCloseBehavior, RichText, Vec2};

/// The height of a button a finger can hit: 48 dp on Android, a little over 44 pt on iOS.
pub(crate) const TAP: f32 = 48.0;
/// The size of the text on buttons.
pub(crate) const TEXT_SIZE: f32 = 18.0;
/// The size of an icon on its own.
pub(crate) const ICON_SIZE: f32 = 24.0;

/// Where the app may draw: the screen less the system bars and the camera cutout. The app sets
/// it each frame with [`set_safe_area`]; pop-ups and dialogs stay inside it.
pub(crate) fn safe_area(ctx: &egui::Context) -> egui::Rect {
    ctx.data(|d| d.get_temp(safe_area_id()))
        .unwrap_or_else(|| ctx.content_rect())
}

pub(crate) fn set_safe_area(ctx: &egui::Context, rect: egui::Rect) {
    ctx.data_mut(|d| d.insert_temp(safe_area_id(), rect));
}

fn safe_area_id() -> egui::Id {
    egui::Id::unique("dm-safe-area")
}

/// An icon and a word, for a button: `label("\u{E3D0}", "Play")`. An empty word gives the icon
/// alone, at the larger icon size; an empty icon gives the word alone.
pub(crate) fn label(icon: &str, word: &str) -> RichText {
    if word.is_empty() {
        RichText::new(icon).size(ICON_SIZE)
    } else if icon.is_empty() {
        RichText::new(word).size(TEXT_SIZE)
    } else {
        RichText::new(format!("{icon} {word}")).size(TEXT_SIZE)
    }
}

/// A button with an icon and a word, at least [`TAP`] high.
pub(crate) fn button<'a>(icon: &str, word: &str) -> egui::Button<'a> {
    egui::Button::new(label(icon, word)).min_size(Vec2::new(TAP, TAP))
}

/// The same as [`button`], drawn as on or off.
pub(crate) fn toggle_button<'a>(selected: bool, icon: &str, word: &str) -> egui::Button<'a> {
    button(icon, word).selected(selected)
}

/// Shows a small pop-up under `anchor` while it is open. A tap on `anchor` opens and closes it;
/// a tap outside closes it. Returns what `add` returned, when the pop-up showed this frame.
/// Its contents scroll when they are taller than the screen below `anchor`, as on a phone in
/// landscape.
pub(crate) fn popup<R>(anchor: &egui::Response, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let below = safe_area(&anchor.ctx).bottom() - anchor.rect.bottom() - 32.0;
    let max_height = below.max(120.0);
    Popup::from_toggle_button_response(anchor)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            egui::ScrollArea::vertical()
                .id_salt("dm-popup-scroll")
                .max_height(max_height)
                .min_scrolled_height(max_height)
                .show(ui, add)
                .inner
        })
        .map(|r| r.inner)
}

/// What a dialog's caller learns each frame.
pub(crate) struct Dialog<R> {
    /// What the dialog's contents returned.
    pub(crate) inner: R,
    /// The user closed it: the close button, a tap outside, Escape, or `ui.close()` inside.
    /// The caller stops showing it.
    pub(crate) closed: bool,
}

/// Shows a dialog in the middle of the screen over a dimmed grid. Call it every frame while the
/// dialog is open. It has a title and a close button at the top, and its contents scroll when
/// they are taller than the screen. It leaves a margin of the grid visible around it, and stays
/// clear of the system bars.
pub(crate) fn dialog<R>(
    ctx: &egui::Context,
    id_salt: &str,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Dialog<R> {
    show_dialog(ctx, id_salt, title, false, add)
}

/// The same as [`dialog`], but at the top of the screen: for a dialog with a text box, so a
/// phone's on-screen keyboard, which rises from the bottom, leaves its buttons visible.
pub(crate) fn dialog_at_top<R>(
    ctx: &egui::Context,
    id_salt: &str,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Dialog<R> {
    show_dialog(ctx, id_salt, title, true, add)
}

fn show_dialog<R>(
    ctx: &egui::Context,
    id_salt: &str,
    title: &str,
    top: bool,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Dialog<R> {
    let screen = safe_area(ctx);
    let margin = 16.0;
    let width = (screen.width() - 2.0 * margin).clamp(200.0, 560.0);
    let max_height = (screen.height() - 2.0 * margin).max(120.0);
    let mut close = false;
    let id = egui::Id::unique(("dm-dialog", id_salt));
    // Centred in the safe area rather than on the whole screen, so the system bars cover none
    // of it.
    let whole = ctx.content_rect();
    let area = if top {
        egui::Modal::default_area(id).anchor(
            egui::Align2::CENTER_TOP,
            Vec2::new(
                screen.center().x - whole.center().x,
                screen.top() - whole.top() + margin,
            ),
        )
    } else {
        egui::Modal::default_area(id).anchor(
            egui::Align2::CENTER_CENTER,
            screen.center() - whole.center(),
        )
    };
    let response = egui::Modal::new(id).area(area).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(22.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(button(crate::icons::X, "")).clicked() {
                    close = true;
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt(("dm-dialog-scroll", id_salt))
            .max_height(max_height - TAP - 40.0)
            .min_scrolled_height(max_height - TAP - 40.0)
            .show(ui, add)
            .inner
    });
    Dialog {
        closed: close || response.should_close(),
        inner: response.inner,
    }
}

/// The colour of a drum family: on lit steps, on the stripe beside a row's name and in the drum
/// picker. Kicks red, snares orange, hi-hats yellow, toms green, cymbals blue, the rest purple.
/// A light theme gets darker shades, so they stand out on its pale cells.
pub(crate) fn family_colour(family: dm_project::DrumFamily, dark: bool) -> egui::Color32 {
    use dm_project::DrumFamily;
    let (r, g, b) = match family {
        DrumFamily::Kick => (226, 84, 72),
        DrumFamily::Snare => (232, 142, 44),
        DrumFamily::HiHat => (218, 190, 46),
        DrumFamily::Tom => (76, 176, 94),
        DrumFamily::Cymbal => (66, 146, 226),
        DrumFamily::Percussion => (166, 108, 216),
    };
    let colour = egui::Color32::from_rgb(r, g, b);
    if dark {
        colour
    } else {
        colour.linear_multiply(0.8)
    }
}

/// Asks a yes-or-no question in a small window over everything else: the question, then a
/// button for each answer. Call it every frame while the question is open. Returns `Some(true)`
/// for `yes`; `Some(false)` for `no`, a tap outside the window, or Back; `None` while it waits.
pub(crate) fn confirm(
    ctx: &egui::Context,
    id_salt: &str,
    question: &str,
    yes: &str,
    no: &str,
) -> Option<bool> {
    let screen = safe_area(ctx);
    let whole = ctx.content_rect();
    let width = (screen.width() - 32.0).clamp(200.0, 360.0);
    let id = egui::Id::unique(("dm-confirm", id_salt));
    let area = egui::Modal::default_area(id).anchor(
        egui::Align2::CENTER_CENTER,
        screen.center() - whole.center(),
    );
    let mut answer = None;
    let response = egui::Modal::new(id).area(area).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 12.0);
        ui.label(RichText::new(question).size(TEXT_SIZE + 2.0));
        ui.horizontal_wrapped(|ui| {
            if ui.add(button("", yes)).clicked() {
                answer = Some(true);
            }
            if ui.add(button("", no)).clicked() {
                answer = Some(false);
            }
        });
    });
    if answer.is_none() && response.should_close() {
        answer = Some(false);
    }
    answer
}
