//! A file picker the host provides, such as Android's Storage Access Framework, for a platform
//! whose picker answers later instead of at once.
//!
//! The interface asks with a [`PickRequest`] through the host's callback (see
//! [`crate::DrumApp::with_file_picker`]). The host shows the system picker and, when it closes,
//! hands back the opened file, or nothing if the person backed out, through [`PickAnswers`]. The
//! interface collects the answer on a later frame. See "Milestone 10: Android file picker" in
//! `docs/DESIGN.md`.

use std::fs::File;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

/// What a pick is for. It decides whether the picker opens or creates a file, and how the host
/// opens it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    /// Open a project file to read (Android: `ACTION_OPEN_DOCUMENT`, opened with mode `"r"`).
    OpenProject,
    /// Save the project to a new file (Android: `ACTION_CREATE_DOCUMENT`, mode `"rwt"`).
    SaveProject,
    /// Export a MIDI file (as `SaveProject`).
    ExportMidi,
    /// Export a WAV recording (as `SaveProject`).
    ExportWav,
    /// Export a keyboard style file (as `SaveProject`).
    ExportStyle,
    /// Open a MIDI file or a zip of them to import as grooves (as `OpenProject`).
    OpenGrooves,
}

impl PickKind {
    /// Whether the picker creates a file to write, rather than opening one to read.
    pub fn creates(self) -> bool {
        !matches!(self, PickKind::OpenProject | PickKind::OpenGrooves)
    }
}

/// A request for the host to show its file picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickRequest {
    /// Identifies the answer. The host passes it back unchanged.
    pub id: u64,
    pub kind: PickKind,
    /// The MIME type to offer or to give the new file, such as `audio/midi`. `*/*` to open any.
    pub mime: &'static str,
    /// The suggested file name, with its extension, for a file to create. Empty to open.
    pub name: String,
}

/// A file the person picked, opened by the host: for reading when its kind does not
/// [`create`](PickKind::creates), and empty and open for writing otherwise.
#[derive(Debug)]
pub struct PickedFile {
    pub file: File,
    /// The name to show for it, as the picker reports it, such as `Beat.drums`.
    pub name: String,
}

/// Where the host hands back the answers to its picks. Cheap to clone, and usable from any
/// thread, such as the one a JNI callback arrives on.
#[derive(Clone)]
pub struct PickAnswers {
    sender: Sender<(u64, Option<PickedFile>)>,
    /// The interface's context once it has drawn a frame, so an answer wakes it at once.
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl PickAnswers {
    /// Hands back the answer to the request with this `id`: the opened file, or `None` when the
    /// person backed out or the file would not open.
    pub fn answer(&self, id: u64, file: Option<PickedFile>) {
        let _ = self.sender.send((id, file));
        if let Ok(ctx) = self.ctx.lock()
            && let Some(ctx) = ctx.as_ref()
        {
            ctx.request_repaint();
        }
    }
}

/// The interface's end of the answers.
pub(crate) struct PickInbox {
    receiver: Receiver<(u64, Option<PickedFile>)>,
    answers: PickAnswers,
}

impl PickInbox {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = channel();
        Self {
            receiver,
            answers: PickAnswers {
                sender,
                ctx: Arc::new(Mutex::new(None)),
            },
        }
    }

    /// A handle for the host.
    pub(crate) fn answers(&self) -> PickAnswers {
        self.answers.clone()
    }

    /// Remembers the context, so answers wake the interface. Call it each frame.
    pub(crate) fn set_context(&self, ctx: &egui::Context) {
        if let Ok(mut slot) = self.answers.ctx.lock()
            && slot.is_none()
        {
            *slot = Some(ctx.clone());
        }
    }

    /// The next answer that has arrived, if any.
    pub(crate) fn take(&self) -> Option<(u64, Option<PickedFile>)> {
        self.receiver.try_recv().ok()
    }
}
