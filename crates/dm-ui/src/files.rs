//! Where files to open, save and export come from: the host's file picker where there is one
//! (Android), the export folder on a phone without one (iOS), or the system's own dialogs on a
//! desktop.
//!
//! Every file action asks first and carries on when the answer comes. A desktop dialog answers at
//! once with a path. The host's picker answers on a later frame with an opened file, which the
//! interface matches to its request through [`Pending`]. Either way the action works on a
//! [`Target`], so there is one flow for every platform.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use dm_project::FILE_EXTENSION;

use crate::picker::{PickKind, PickRequest, PickedFile};

/// Whether this build has the system's open and save dialogs: desktops do, phones do not.
pub(crate) const DIALOGS: bool = cfg!(not(any(target_os = "android", target_os = "ios")));

/// A file to read or write: a path a dialog or the export folder gave, or a file the host's
/// picker opened.
#[derive(Debug)]
pub(crate) enum Target {
    Path(PathBuf),
    Picked(PickedFile),
}

impl Target {
    /// How messages name the file: the full path, or the name the picker reported.
    pub(crate) fn name(&self) -> String {
        match self {
            Target::Path(path) => path.display().to_string(),
            Target::Picked(picked) => picked.name.clone(),
        }
    }

    /// The file's name without its extension, such as `Beat` for `Beat.drums`.
    pub(crate) fn stem(&self) -> Option<String> {
        let stem = match self {
            Target::Path(path) => path.file_stem(),
            Target::Picked(picked) => Path::new(&picked.name).file_stem(),
        };
        stem.map(|s| s.to_string_lossy().into_owned())
    }

    /// The file's own name, without its folder, such as `Beat.drums`.
    pub(crate) fn file_name(&self) -> String {
        match self {
            Target::Path(path) => path
                .file_name()
                .map_or_else(|| self.name(), |n| n.to_string_lossy().into_owned()),
            Target::Picked(picked) => picked.name.clone(),
        }
    }

    /// Reads the whole file, but no more than `limit` bytes and one more, so a file that is too
    /// large shows as longer than `limit` without being read in full.
    pub(crate) fn read_limited(&self, limit: usize) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let most = limit as u64 + 1;
        match self {
            Target::Path(path) => fs::File::open(path)?.take(most).read_to_end(&mut bytes)?,
            Target::Picked(picked) => (&picked.file).take(most).read_to_end(&mut bytes)?,
        };
        Ok(bytes)
    }

    pub(crate) fn read_to_string(&self) -> io::Result<String> {
        match self {
            Target::Path(path) => fs::read_to_string(path),
            Target::Picked(picked) => {
                let mut text = String::new();
                (&picked.file).read_to_string(&mut text)?;
                Ok(text)
            }
        }
    }

    /// Writes `bytes` as the whole file. A path is written through a temporary file, so a crash
    /// never leaves half a file. A picked file is new and empty, so the bytes go straight in.
    pub(crate) fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        match self {
            Target::Path(path) => write_atomic(path, bytes),
            Target::Picked(picked) => {
                (&picked.file).write_all(bytes)?;
                picked.file.sync_all()
            }
        }
    }
}

/// Writes `bytes` to a temporary file beside `path`, then renames it over `path`.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("part");
    let result = fs::write(&temporary, bytes).and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// The file extension for each kind of pick.
fn extension(kind: PickKind) -> &'static str {
    match kind {
        PickKind::OpenProject | PickKind::SaveProject => FILE_EXTENSION,
        PickKind::ExportMidi => "mid",
        PickKind::ExportWav => "wav",
        // Every keyboard kind so far writes `.sty`; a kind with another extension will need a
        // pick kind of its own.
        PickKind::ExportStyle => dm_project::KeyboardKind::ALL[0].extension(),
        // Grooves come as `.mid`, `.midi` or `.zip`; see `dialog`.
        PickKind::OpenGrooves => "mid",
        PickKind::OpenSoundfont => "sf2",
    }
}

/// What to ask the host's picker for. `name` is the project's name, which becomes the new file's
/// name.
pub(crate) fn request(id: u64, kind: PickKind, name: &str) -> PickRequest {
    let mime = match kind {
        // Files from elsewhere may be labelled anything; the format check refuses what is not a
        // project.
        PickKind::OpenProject => "*/*",
        // MIDI files and zips are labelled in many ways; the import checks what each file holds.
        PickKind::OpenGrooves => "*/*",
        // SoundFonts have no MIME type that providers agree on; loading checks the file.
        PickKind::OpenSoundfont => "*/*",
        // Not application/json: Android's Downloads provider then adds .json to the name.
        PickKind::SaveProject => "application/octet-stream",
        PickKind::ExportMidi => "audio/midi",
        PickKind::ExportWav => "audio/wav",
        // No MIME type fits a style file; as for projects, this keeps the name as given.
        PickKind::ExportStyle => "application/octet-stream",
    };
    let name = if kind.creates() {
        format!("{name}.{}", extension(kind))
    } else {
        String::new()
    };
    PickRequest {
        id,
        kind,
        mime,
        name,
    }
}

/// Where a pick goes.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Route {
    /// The host's picker, which answers later.
    Picker,
    /// Straight into this file in the export folder, without asking.
    Folder(PathBuf),
    /// The system's dialog, on a desktop.
    Dialog,
}

/// Where a pick of `kind` goes: the host's picker if there is one, else the export folder for an
/// export, else the system's dialog. `name` is the project's name.
pub(crate) fn route(
    kind: PickKind,
    has_picker: bool,
    export_dir: Option<&Path>,
    name: &str,
) -> Route {
    if has_picker {
        return Route::Picker;
    }
    match (kind, export_dir) {
        (PickKind::ExportMidi | PickKind::ExportWav | PickKind::ExportStyle, Some(dir)) => {
            Route::Folder(dir.join(format!("{name}.{}", extension(kind))))
        }
        _ => Route::Dialog,
    }
}

/// The picks sent to the host's picker and not yet answered, by id.
#[derive(Default)]
pub(crate) struct Pending {
    last: u64,
    waiting: HashMap<u64, PickKind>,
}

impl Pending {
    /// A fresh id for a pick of `kind`, remembered until its answer comes.
    pub(crate) fn ask(&mut self, kind: PickKind) -> u64 {
        self.last += 1;
        self.waiting.insert(self.last, kind);
        self.last
    }

    /// What an answer is for, and its file. `None` when the id is unknown or answered already,
    /// or when the person backed out: nothing is to be done then.
    pub(crate) fn collect(
        &mut self,
        id: u64,
        file: Option<PickedFile>,
    ) -> Option<(PickKind, PickedFile)> {
        let kind = self.waiting.remove(&id)?;
        file.map(|file| (kind, file))
    }
}

/// Shows the system's dialog for a pick of `kind`, and returns the chosen path. `name` is the
/// project's name, which the save dialog suggests.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) fn dialog(kind: PickKind, name: &str) -> Option<PathBuf> {
    let filter = match kind {
        PickKind::OpenProject | PickKind::SaveProject => tr!("projects-file-kind-project"),
        PickKind::ExportMidi => tr!("projects-file-kind-midi"),
        PickKind::ExportWav => tr!("projects-file-kind-wav"),
        PickKind::ExportStyle => tr!("projects-file-kind-style"),
        PickKind::OpenGrooves => tr!("grooves-file-kind"),
        PickKind::OpenSoundfont => tr!("soundbank-file-kind"),
    };
    let extension = extension(kind);
    let extensions: &[&str] = if kind == PickKind::OpenGrooves {
        // MIDI files, zips of them, and Yamaha keyboard styles, whose drums become grooves.
        &[
            "mid", "midi", "zip", "sty", "prs", "sst", "scp", "fps", "bcs",
        ]
    } else {
        &[extension]
    };
    let dialog = rfd::FileDialog::new().add_filter(filter, extensions);
    if kind.creates() {
        dialog
            .set_file_name(format!("{name}.{extension}"))
            .save_file()
    } else {
        dialog.pick_file()
    }
}

/// Phones have no system dialog: Android uses the host's picker, iOS the export folder.
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) fn dialog(_kind: PickKind, _name: &str) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("dm-files-{name}-{}", std::process::id()))
    }

    fn picked(path: &Path, name: &str) -> PickedFile {
        PickedFile {
            file: fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)
                .unwrap(),
            name: name.to_owned(),
        }
    }

    #[test]
    fn answers_are_matched_to_their_requests() {
        let mut pending = Pending::default();
        let open = pending.ask(PickKind::OpenProject);
        let midi = pending.ask(PickKind::ExportMidi);
        assert_ne!(open, midi);
        let path = temp_path("answers");
        // An id never asked for is ignored.
        assert!(pending.collect(99, Some(picked(&path, "x"))).is_none());
        // Backing out does nothing, and forgets the request.
        assert!(pending.collect(open, None).is_none());
        assert!(pending.collect(open, Some(picked(&path, "x"))).is_none());
        let (kind, file) = pending
            .collect(midi, Some(picked(&path, "Beat.mid")))
            .unwrap();
        assert_eq!(kind, PickKind::ExportMidi);
        assert_eq!(file.name, "Beat.mid");
        // A second answer to the same request is ignored.
        assert!(pending.collect(midi, Some(picked(&path, "x"))).is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn requests_name_the_new_file_and_its_type() {
        let open = request(1, PickKind::OpenProject, "Beat");
        assert_eq!((open.mime, open.name.as_str()), ("*/*", ""));
        let grooves = request(5, PickKind::OpenGrooves, "Beat");
        assert_eq!((grooves.mime, grooves.name.as_str()), ("*/*", ""));
        assert!(!PickKind::OpenGrooves.creates());
        let save = request(2, PickKind::SaveProject, "Beat");
        assert_eq!(
            (save.mime, save.name.as_str()),
            ("application/octet-stream", "Beat.drums")
        );
        let midi = request(3, PickKind::ExportMidi, "Beat");
        assert_eq!((midi.mime, midi.name.as_str()), ("audio/midi", "Beat.mid"));
        let wav = request(4, PickKind::ExportWav, "Beat");
        assert_eq!(
            (wav.id, wav.mime, wav.name.as_str()),
            (4, "audio/wav", "Beat.wav")
        );
        let style = request(6, PickKind::ExportStyle, "Beat");
        assert_eq!(
            (style.mime, style.name.as_str()),
            ("application/octet-stream", "Beat.sty")
        );
    }

    #[test]
    fn exports_prefer_the_picker_then_the_folder_then_the_dialog() {
        let dir = Path::new("exports");
        let midi = PickKind::ExportMidi;
        assert_eq!(route(midi, true, Some(dir), "Beat"), Route::Picker);
        assert_eq!(
            route(midi, false, Some(dir), "Beat"),
            Route::Folder(dir.join("Beat.mid"))
        );
        assert_eq!(
            route(PickKind::ExportWav, false, Some(dir), "Beat"),
            Route::Folder(dir.join("Beat.wav"))
        );
        assert_eq!(
            route(PickKind::ExportStyle, false, Some(dir), "Beat"),
            Route::Folder(dir.join("Beat.sty"))
        );
        assert_eq!(route(midi, false, None, "Beat"), Route::Dialog);
        // Projects never go to the export folder.
        assert_eq!(
            route(PickKind::SaveProject, false, Some(dir), "Beat"),
            Route::Dialog
        );
        assert_eq!(
            route(PickKind::OpenProject, true, None, "Beat"),
            Route::Picker
        );
        // Grooves never go to the export folder either.
        assert_eq!(
            route(PickKind::OpenGrooves, false, Some(dir), "Beat"),
            Route::Dialog
        );
    }

    #[test]
    fn reading_stops_just_past_the_limit() {
        let path = temp_path("limited");
        fs::write(&path, [7u8; 10]).unwrap();
        let target = Target::Path(path.clone());
        assert_eq!(target.read_limited(100).unwrap().len(), 10);
        assert_eq!(target.read_limited(10).unwrap().len(), 10);
        // One byte more than the limit tells that the file is too large.
        assert_eq!(target.read_limited(4).unwrap().len(), 5);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn targets_are_named_for_messages() {
        let path = temp_path("names");
        let target = Target::Picked(picked(&path, "My beat.drums"));
        assert_eq!(target.name(), "My beat.drums");
        assert_eq!(target.file_name(), "My beat.drums");
        assert_eq!(target.stem().as_deref(), Some("My beat"));
        let target = Target::Path(PathBuf::from("folder").join("Rock.drums"));
        assert_eq!(target.stem().as_deref(), Some("Rock"));
        assert_eq!(target.file_name(), "Rock.drums");
        drop(target);
        let _ = fs::remove_file(path);
    }
}
