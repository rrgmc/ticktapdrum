//! Icons from the Phosphor font, bundled with the app (see `assets/PHOSPHOR-LICENSE.txt`).
//!
//! The font is added as a fallback to egui's proportional fonts, so an icon can sit in any text,
//! for example `format!("{PLAY} Play")`. The `egui-phosphor` crates would do the same, but as of
//! 2026-11-19 they need egui 0.36; the constants below are copied from egui-phosphor 0.14.0.

static FONT: &[u8] = include_bytes!("../assets/Phosphor-Bold.ttf");

/// Adds the icon font. Called once, before the first frame that shows an icon.
pub(crate) fn install(ctx: &egui::Context) {
    ctx.add_font(egui::FontInsert::new(
        "phosphor",
        egui::FontData::from_static(FONT),
        vec![egui::InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: egui::FontPriority::Lowest,
        }],
    ));
}

pub(crate) const ARROW_CLOCKWISE: &str = "\u{E036}";
pub(crate) const ARROW_COUNTER_CLOCKWISE: &str = "\u{E038}";
/// A curved arrow going back to the left: Undo. The circular arrows read as "refresh".
pub(crate) const UNDO: &str = "\u{E08A}";
/// The same going to the right: Redo.
pub(crate) const REDO: &str = "\u{E08C}";
pub(crate) const ARROW_DOWN: &str = "\u{E03E}";
pub(crate) const ARROW_UP: &str = "\u{E08E}";
pub(crate) const BROOM: &str = "\u{EC54}";
pub(crate) const CARET_DOWN: &str = "\u{E136}";
pub(crate) const CARET_LEFT: &str = "\u{E138}";
pub(crate) const CARET_RIGHT: &str = "\u{E13A}";
pub(crate) const CHECK: &str = "\u{E182}";
pub(crate) const COPY: &str = "\u{E1CA}";
pub(crate) const DEVICE_ROTATE: &str = "\u{EDF2}";
pub(crate) const DOTS_THREE: &str = "\u{E1FE}";
pub(crate) const EXPORT: &str = "\u{EAF0}";
pub(crate) const EYE_SLASH: &str = "\u{E224}";
pub(crate) const FILE_ARROW_UP: &str = "\u{E61E}";
pub(crate) const FILE_AUDIO: &str = "\u{EA20}";
pub(crate) const FILE_PLUS: &str = "\u{E236}";
pub(crate) const FLOPPY_DISK: &str = "\u{E248}";
pub(crate) const FOLDER: &str = "\u{E24A}";
pub(crate) const FOLDER_OPEN: &str = "\u{E256}";
pub(crate) const GLOBE: &str = "\u{E288}";
pub(crate) const HEADPHONES: &str = "\u{E2A6}";
pub(crate) const IMAGE: &str = "\u{E2CA}";
pub(crate) const LINK: &str = "\u{E2E2}";
pub(crate) const LIST: &str = "\u{E2F0}";
pub(crate) const METRONOME: &str = "\u{EC8E}";
pub(crate) const MINUS: &str = "\u{E32A}";
pub(crate) const MUSIC_NOTES: &str = "\u{E340}";
pub(crate) const MUSIC_NOTES_PLUS: &str = "\u{EB7C}";
pub(crate) const PENCIL_SIMPLE: &str = "\u{E3B4}";
pub(crate) const PLAY: &str = "\u{E3D0}";
pub(crate) const PLAYLIST: &str = "\u{E6AA}";
pub(crate) const PLUS: &str = "\u{E3D4}";
pub(crate) const REPEAT: &str = "\u{E3F6}";
pub(crate) const SLIDERS_HORIZONTAL: &str = "\u{E434}";
pub(crate) const SPARKLE: &str = "\u{E6A2}";
pub(crate) const SPEAKER_HIGH: &str = "\u{E44A}";
pub(crate) const SPEAKER_SLASH: &str = "\u{E45A}";
pub(crate) const STOP: &str = "\u{E46C}";
pub(crate) const TRASH: &str = "\u{E4A6}";
pub(crate) const WAVEFORM: &str = "\u{E802}";
pub(crate) const X: &str = "\u{E4F6}";
