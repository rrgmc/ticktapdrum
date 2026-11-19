//! The tempo pop-up: large − and + buttons around the tempo, a slider, and swing.
//!
//! The same contents show in a dialog when the strip has no room for the tempo button.

use dm_audio::{MAX_BPM, MIN_BPM};
use egui::{RichText, Vec2};

use crate::icons;
use crate::widgets::{self, TAP};

pub(crate) enum TempoEvent {
    /// The new tempo, in beats per minute.
    Tempo(f32),
    /// The swing amount, 0.0 to 0.5 of a step.
    Swing(f32),
}

/// The width of the sliders, in points. Wide enough to aim with a finger.
const SLIDER_WIDTH: f32 = 240.0;

/// Draws the tempo and swing controls. Changes come back as events.
pub(crate) fn contents(ui: &mut egui::Ui, bpm: f32, swing: f32) -> Vec<TempoEvent> {
    let mut events = Vec::new();
    // Kept narrow, so the explanation at the end wraps instead of widening the pop-up.
    ui.set_min_width(SLIDER_WIDTH + 40.0);
    ui.set_max_width(SLIDER_WIDTH + 80.0);
    ui.spacing_mut().interact_size.y = 36.0;
    ui.spacing_mut().slider_width = SLIDER_WIDTH;

    let mut value = bpm;
    ui.horizontal(|ui| {
        if ui
            .add(widgets::button(icons::MINUS, "").min_size(Vec2::new(TAP + 8.0, TAP + 8.0)))
            .on_hover_text(tr!("tempo-slower"))
            .clicked()
        {
            value -= 1.0;
        }
        ui.add_sized(
            Vec2::new(110.0, TAP + 8.0),
            egui::Label::new(RichText::new(format!("{bpm:.0}")).size(40.0).strong()),
        );
        if ui
            .add(widgets::button(icons::PLUS, "").min_size(Vec2::new(TAP + 8.0, TAP + 8.0)))
            .on_hover_text(tr!("tempo-faster"))
            .clicked()
        {
            value += 1.0;
        }
    });
    ui.label(tr!("tempo-bpm"));
    ui.add(
        egui::Slider::new(&mut value, MIN_BPM..=MAX_BPM)
            .step_by(1.0)
            .show_value(false),
    );
    let value = value.round().clamp(MIN_BPM, MAX_BPM);
    if value != bpm {
        events.push(TempoEvent::Tempo(value));
    }

    ui.add_space(8.0);
    ui.label(RichText::new(tr!("tempo-swing")).size(18.0).strong());
    // Shown as 0 to 100: 100 delays every second step by half a step.
    let mut percent = (swing * 200.0).round() as u8;
    if ui
        .add(egui::Slider::new(&mut percent, 0..=100).suffix("%"))
        .changed()
    {
        events.push(TempoEvent::Swing(f32::from(percent) / 200.0));
    }
    // A swing of about 66% puts every second step on the last note of a triplet.
    ui.label(tr!("tempo-swing-help", triplet = 66));
    events
}
