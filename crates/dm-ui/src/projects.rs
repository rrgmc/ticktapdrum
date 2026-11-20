//! What the menu's project items do, and what they need to know. The menu and its dialogs are
//! in `menu.rs`.
//!
//! Projects are kept in the app's own folder on every platform, so this works the same on a
//! phone. On a desktop, and on a phone whose host has a file picker, two more items open and save
//! `.drums` files anywhere.

pub(crate) enum ProjectEvent {
    /// Save under the current name.
    Save,
    /// Save under this new name.
    SaveAs(String),
    New,
    Open(String),
    Delete(String),
    /// Choose a file to open, where `ProjectsView::file_items` is set.
    OpenFile,
    /// Choose where to save a file, where `ProjectsView::file_items` is set.
    SaveFile,
    /// Replace the project with a preset beat.
    Preset(&'static str),
    ExportMidi,
    ExportWav,
    /// Save a keyboard style file of this kind, with these sections.
    ExportStyle {
        kind: dm_project::KeyboardKind,
        sections: dm_project::StyleSections,
    },
    /// Send a keyboard style of this kind, with these sections, to the keyboard on the MIDI
    /// output.
    SendStyle {
        kind: dm_project::KeyboardKind,
        sections: dm_project::StyleSections,
    },
    CancelExport,
    /// Keep the screen in landscape, or let it turn freely.
    Landscape(bool),
    /// Add the pasted drum tab as a new pattern, with what the person chose for reading it.
    PasteTab(String, dm_tab::Options),
    /// Start a new project from this groove that comes with the app, as a ready-made style.
    Magenta(usize),
    /// Show the bar thumbnails under the strip all the time, or only in the bar pop-up.
    BarLine(bool),
    /// Show row names as names, pictures and names, or pictures.
    RowLabels(crate::pictures::RowLabels),
}

/// What the screen needs to know to draw itself.
pub(crate) struct ProjectsView<'a> {
    pub(crate) name: Option<&'a str>,
    pub(crate) unsaved: bool,
    pub(crate) saved: &'a [String],
    /// Whether the Open dialog offers opening and saving project files anywhere: on a desktop,
    /// with its dialogs, and where the host has a file picker.
    pub(crate) file_items: bool,
    /// A line about the last save or open: what happened, or what went wrong.
    pub(crate) message: Option<&'a str>,
    /// What the last export reported: where the file went, or what went wrong.
    pub(crate) export_message: Option<&'a str>,
    /// How far a WAV export has got, while one runs.
    pub(crate) exporting: Option<f32>,
    /// Whether the bank is loaded, which a WAV export needs.
    pub(crate) can_render: bool,
    /// Where exports go on a phone with neither a save dialog nor a file picker, said the way
    /// people find it. `None` where each export asks.
    pub(crate) export_place: Option<&'a str>,
    /// Whether the screen is kept in landscape. `None` where the host cannot lock it.
    pub(crate) landscape: Option<bool>,
    /// How row names show in the grid.
    pub(crate) row_labels: crate::pictures::RowLabels,
    /// Whether the bar thumbnails show under the strip.
    pub(crate) bar_line: bool,
}
