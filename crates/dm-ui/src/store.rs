//! Where projects live on the device: named projects in a `projects` folder, and one autosave.
//!
//! Every write goes to a temporary file first and is then renamed over the old one, so a crash or
//! a phone stopping the app mid-write never leaves a half-written project. The same reading and
//! writing serve project files anywhere else, including ones the host's picker opened.
//!
//! Errors come back as text in the interface's language, ready to show.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use dm_project::{FILE_EXTENSION, InvalidReason, LoadError, Project, from_json, to_json};

use crate::files::Target;
use crate::i18n::Language;

use crate::pictures::RowLabels;

const AUTOSAVE: &str = "autosave";
/// Holds the name of the project the autosave belongs to, if it has one.
const AUTOSAVE_NAME: &str = "autosave.name";
/// Exists while the screen is kept in landscape.
const LANDSCAPE: &str = "landscape.lock";
/// Exists while the app is in basic mode.
const BASIC: &str = "basic.mode";
/// Exists while the menu offers pasting a drum tab, an experiment that is off by default.
const PASTE_TABS: &str = "paste.tabs";
/// Holds the tag of the language the person chose. Missing while the app follows the system.
const LANGUAGE: &str = "language";
/// Holds the name of the chosen MIDI output. Missing while none is chosen.
const MIDI_OUTPUT: &str = "midi.output";
/// Holds how row names show, when not as names.
const ROW_LABELS: &str = "row.labels";
/// The longest project name, in characters.
const MAX_NAME: usize = 60;

pub(crate) struct Store {
    dir: PathBuf,
}

impl Store {
    /// `data_dir` is the app's private folder.
    pub(crate) fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.to_path_buf(),
        }
    }

    fn projects_dir(&self) -> PathBuf {
        self.dir.join("projects")
    }

    fn project_path(&self, name: &str) -> PathBuf {
        self.projects_dir().join(format!("{name}.{FILE_EXTENSION}"))
    }

    /// The saved projects' names, sorted, case ignored.
    pub(crate) fn list(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.projects_dir()) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == FILE_EXTENSION))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    #[cfg(test)]
    pub(crate) fn exists(&self, name: &str) -> bool {
        self.project_path(name).exists()
    }

    pub(crate) fn save(&self, name: &str, project: &Project) -> Result<(), String> {
        fs::create_dir_all(self.projects_dir()).map_err(folder_error)?;
        write_path(&self.project_path(name), project)
    }

    pub(crate) fn load(&self, name: &str) -> Result<Project, String> {
        read_path(&self.project_path(name))
    }

    pub(crate) fn delete(&self, name: &str) -> Result<(), String> {
        fs::remove_file(self.project_path(name)).map_err(delete_error)
    }

    /// Writes the autosave and the name of the project it belongs to.
    pub(crate) fn autosave(&self, project: &Project, name: Option<&str>) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(folder_error)?;
        write_path(
            &self.dir.join(format!("{AUTOSAVE}.{FILE_EXTENSION}")),
            project,
        )?;
        let name_file = self.dir.join(AUTOSAVE_NAME);
        match name {
            Some(name) => fs::write(name_file, name).map_err(write_error),
            None => match fs::remove_file(name_file) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(delete_error(e)),
                _ => Ok(()),
            },
        }
    }

    /// Whether the screen is kept in landscape. A device setting, not part of any project.
    pub(crate) fn landscape(&self) -> bool {
        self.flag(LANDSCAPE)
    }

    pub(crate) fn set_landscape(&self, on: bool) -> Result<(), String> {
        self.set_flag(LANDSCAPE, on)
    }

    /// Whether the app is in basic mode. A device setting, not part of any project.
    pub(crate) fn basic(&self) -> bool {
        self.flag(BASIC)
    }

    pub(crate) fn set_basic(&self, on: bool) -> Result<(), String> {
        self.set_flag(BASIC, on)
    }

    /// Whether the menu offers pasting a drum tab. A device setting, not part of any project.
    pub(crate) fn paste_tabs(&self) -> bool {
        self.flag(PASTE_TABS)
    }

    pub(crate) fn set_paste_tabs(&self, on: bool) -> Result<(), String> {
        self.set_flag(PASTE_TABS, on)
    }

    /// The language the person chose, or `None` to follow the system. A device setting, not
    /// part of any project.
    pub(crate) fn language(&self) -> Option<Language> {
        let tag = fs::read_to_string(self.dir.join(LANGUAGE)).ok()?;
        Language::from_tag(tag.trim())
    }

    pub(crate) fn set_language(&self, language: Option<Language>) -> Result<(), String> {
        let path = self.dir.join(LANGUAGE);
        match language {
            Some(language) => {
                fs::create_dir_all(&self.dir).map_err(folder_error)?;
                fs::write(path, language.tag()).map_err(write_error)
            }
            None => match fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(delete_error(e)),
                _ => Ok(()),
            },
        }
    }

    /// The name of the chosen MIDI output, or `None` for none. A device setting, not part of
    /// any project. The name is kept exactly as the system reported it.
    pub(crate) fn midi_output(&self) -> Option<String> {
        let name = fs::read_to_string(self.dir.join(MIDI_OUTPUT)).ok()?;
        (!name.is_empty()).then_some(name)
    }

    pub(crate) fn set_midi_output(&self, name: Option<&str>) -> Result<(), String> {
        let path = self.dir.join(MIDI_OUTPUT);
        match name {
            Some(name) if !name.is_empty() => {
                fs::create_dir_all(&self.dir).map_err(folder_error)?;
                fs::write(path, name).map_err(write_error)
            }
            _ => match fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(delete_error(e)),
                _ => Ok(()),
            },
        }
    }

    /// How row names show in the grid. A device setting, not part of any project.
    pub(crate) fn row_labels(&self) -> RowLabels {
        fs::read_to_string(self.dir.join(ROW_LABELS))
            .map(|text| RowLabels::from_str(&text))
            .unwrap_or_default()
    }

    pub(crate) fn set_row_labels(&self, labels: RowLabels) -> Result<(), String> {
        let path = self.dir.join(ROW_LABELS);
        if labels == RowLabels::Names {
            return match fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(delete_error(e)),
                _ => Ok(()),
            };
        }
        fs::create_dir_all(&self.dir).map_err(folder_error)?;
        fs::write(path, labels.as_str()).map_err(write_error)
    }

    /// A setting kept as a file that exists while it is on.
    fn flag(&self, file: &str) -> bool {
        self.dir.join(file).exists()
    }

    fn set_flag(&self, file: &str, on: bool) -> Result<(), String> {
        let path = self.dir.join(file);
        if on {
            fs::create_dir_all(&self.dir).map_err(folder_error)?;
            fs::write(path, "").map_err(write_error)
        } else {
            match fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(delete_error(e)),
                _ => Ok(()),
            }
        }
    }

    /// The autosaved project and its name, if there is one that loads.
    pub(crate) fn restore(&self) -> Option<(Project, Option<String>)> {
        let path = self.dir.join(format!("{AUTOSAVE}.{FILE_EXTENSION}"));
        if !path.exists() {
            return None;
        }
        match read_path(&path) {
            Ok(project) => {
                let name = fs::read_to_string(self.dir.join(AUTOSAVE_NAME))
                    .ok()
                    .and_then(|n| clean_name(&n));
                Some((project, name))
            }
            Err(e) => {
                log::warn!("the autosave did not load: {e}");
                None
            }
        }
    }
}

/// Writes a project to a file anywhere: a path, through a temporary file, or a file the host's
/// picker opened.
pub(crate) fn write_file(target: &Target, project: &Project) -> Result<(), String> {
    target
        .write_all(to_json(project).as_bytes())
        .map_err(write_error)
}

/// Reads a project from a file anywhere: a path, or a file the host's picker opened.
pub(crate) fn read_file(target: &Target) -> Result<Project, String> {
    let text = target.read_to_string().map_err(read_error)?;
    from_json(&text).map_err(|e| load_error_text(&e))
}

fn write_path(path: &Path, project: &Project) -> Result<(), String> {
    write_file(&Target::Path(path.to_path_buf()), project)
}

fn read_path(path: &Path) -> Result<Project, String> {
    read_file(&Target::Path(path.to_path_buf()))
}

// The system's own description of an io error goes into these messages as it is, so it may be
// in another language than the rest, as the system chose.

fn read_error(error: io::Error) -> String {
    tr!("errors-read-file", detail = error.to_string())
}

fn write_error(error: io::Error) -> String {
    tr!("errors-write-file", detail = error.to_string())
}

fn delete_error(error: io::Error) -> String {
    tr!("errors-delete-file", detail = error.to_string())
}

fn folder_error(error: io::Error) -> String {
    tr!("errors-create-folder", detail = error.to_string())
}

/// Why a project file did not load, in the interface's language. The parser's own description,
/// where there is one, stays in English.
fn load_error_text(error: &LoadError) -> String {
    match error {
        LoadError::NotJson(detail) => tr!("errors-not-project", detail = detail.as_str()),
        LoadError::NewerFormat(format) => tr!("errors-newer-format", format = *format),
        LoadError::Invalid(reason) => match reason {
            InvalidReason::NotAnObject => tr!("errors-not-an-object"),
            InvalidReason::FormatMissing => tr!("errors-format-missing"),
            InvalidReason::FormatZero => tr!("errors-format-zero"),
            InvalidReason::FormatNotWholeNumber => tr!("errors-format-not-whole"),
            InvalidReason::Layout(detail) => tr!("errors-damaged", detail = detail.as_str()),
            InvalidReason::NoRows => tr!("errors-no-rows"),
            InvalidReason::TooManyRows { count, max } => {
                tr!("errors-too-many-rows", count = *count, max = *max)
            }
            InvalidReason::NoPatterns => tr!("errors-no-patterns"),
            InvalidReason::TooManyPatterns { count, max } => {
                tr!("errors-too-many-patterns", count = *count, max = *max)
            }
            InvalidReason::SongTooLong { count, max } => {
                tr!("errors-song-too-long", count = *count, max = *max)
            }
            InvalidReason::DuplicateRowId(id) => tr!("errors-duplicate-row-id", id = *id),
            InvalidReason::BadPatternName(name) => {
                tr!("errors-bad-pattern-name", name = name.as_str())
            }
            InvalidReason::DuplicatePattern(name) => {
                tr!("errors-duplicate-pattern", name = name.as_str())
            }
            InvalidReason::SongPatternMissing(name) => {
                tr!("errors-song-pattern-missing", name = name.as_str())
            }
        },
    }
}

/// A name that is safe as a file name on every platform, or `None` if nothing is left of it.
/// Keeps letters, digits, spaces, `-` and `_`.
pub(crate) fn clean_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .take(MAX_NAME)
        .collect();
    let cleaned = cleaned.trim().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("dm-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Store::new(&dir)
    }

    // Other tests switch the language while these run, so these check only what every
    // language's text contains.

    #[test]
    fn a_file_that_does_not_load_says_why() {
        let store = temp_store("damaged");
        fs::create_dir_all(store.projects_dir()).unwrap();
        let rows: Vec<String> = (1..=33)
            .map(|id| format!(r#"{{"id": {id}, "key": 36}}"#))
            .collect();
        let text = format!(
            r#"{{"format": 1, "rows": [{}], "patterns": [{{"id": "A"}}]}}"#,
            rows.join(",")
        );
        fs::write(store.project_path("Damaged"), text).unwrap();
        let error = store.load("Damaged").unwrap_err();
        assert!(error.contains("33") && error.contains("32"), "{error}");
        assert!(!error.starts_with("errors-"), "{error}");
    }

    #[test]
    fn a_file_that_cannot_be_read_says_so() {
        let error = read_path(Path::new("no such folder/no such file.drums")).unwrap_err();
        assert!(!error.starts_with("errors-"), "{error}");
        // The system's own description is kept.
        assert!(
            error.ends_with("(os error 2)") || error.ends_with("(os error 3)"),
            "{error}"
        );
    }

    #[test]
    fn a_picked_file_is_written_and_read() {
        use crate::picker::PickedFile;
        let path = std::env::temp_dir().join(format!("dm-store-picked-{}", std::process::id()));
        let open = |write: bool| PickedFile {
            file: fs::OpenOptions::new()
                .read(true)
                .write(write)
                .create(write)
                .truncate(write)
                .open(&path)
                .unwrap(),
            name: "Beat.drums".to_owned(),
        };
        let mut p = Project::new();
        p.set_step(0, 1, 5, 90).unwrap();
        write_file(&Target::Picked(open(true)), &p).unwrap();
        assert_eq!(read_file(&Target::Picked(open(false))).unwrap(), p);
        // The same file reads as a path too.
        assert_eq!(read_path(&path).unwrap(), p);
        fs::write(&path, "not a project").unwrap();
        let error = read_file(&Target::Picked(open(false))).unwrap_err();
        assert!(!error.starts_with("errors-"), "{error}");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  My beat!? "), Some("My beat".into()));
        assert_eq!(clean_name("../../etc"), Some("etc".into()));
        assert_eq!(clean_name("?/\\:*"), None);
        assert_eq!(clean_name(&"x".repeat(100)).unwrap().len(), MAX_NAME);
    }

    #[test]
    fn projects_save_list_load_and_delete() {
        let store = temp_store("named");
        let mut p = Project::new();
        p.set_step(0, 0, 3, 100).unwrap();
        store.save("Rock", &p).unwrap();
        store.save("ambient", &Project::new()).unwrap();
        assert_eq!(store.list(), vec!["ambient".to_owned(), "Rock".to_owned()]);
        assert_eq!(store.load("Rock").unwrap(), p);
        store.delete("Rock").unwrap();
        assert!(!store.exists("Rock"));
    }

    #[test]
    fn the_autosave_keeps_its_name() {
        let store = temp_store("auto");
        assert!(store.restore().is_none());
        let mut p = Project::new();
        p.tempo = 120.0;
        store.autosave(&p, Some("Song 1")).unwrap();
        let (restored, name) = store.restore().unwrap();
        assert_eq!(restored, p);
        assert_eq!(name.as_deref(), Some("Song 1"));
        store.autosave(&p, None).unwrap();
        assert_eq!(store.restore().unwrap().1, None);
    }

    #[test]
    fn the_landscape_setting_is_kept() {
        let store = temp_store("landscape");
        assert!(!store.landscape());
        store.set_landscape(true).unwrap();
        assert!(store.landscape());
        store.set_landscape(false).unwrap();
        store.set_landscape(false).unwrap();
        assert!(!store.landscape());
    }

    #[test]
    fn the_basic_setting_is_kept_apart_from_landscape() {
        let store = temp_store("basic");
        assert!(!store.basic());
        store.set_basic(true).unwrap();
        assert!(store.basic());
        assert!(!store.landscape());
        store.set_basic(false).unwrap();
        assert!(!store.basic());
    }

    #[test]
    fn the_paste_tabs_setting_is_off_until_turned_on() {
        let store = temp_store("paste-tabs");
        assert!(!store.paste_tabs());
        store.set_paste_tabs(true).unwrap();
        assert!(store.paste_tabs());
        assert!(!store.basic() && !store.landscape());
        store.set_paste_tabs(false).unwrap();
        store.set_paste_tabs(false).unwrap();
        assert!(!store.paste_tabs());
    }

    #[test]
    fn the_language_follows_the_system_until_one_is_chosen() {
        let store = temp_store("language");
        assert_eq!(store.language(), None);
        store
            .set_language(Some(Language::BrazilianPortuguese))
            .unwrap();
        assert_eq!(store.language(), Some(Language::BrazilianPortuguese));
        store.set_language(None).unwrap();
        store.set_language(None).unwrap();
        assert_eq!(store.language(), None);
    }

    #[test]
    fn the_midi_output_is_kept_by_name() {
        let store = temp_store("midi-output");
        assert_eq!(store.midi_output(), None);
        // A name is kept exactly, spaces and all.
        let name = " Microsoft GS Wavetable Synth ";
        store.set_midi_output(Some(name)).unwrap();
        assert_eq!(store.midi_output().as_deref(), Some(name));
        assert!(!store.basic() && store.language().is_none());
        store.set_midi_output(None).unwrap();
        store.set_midi_output(None).unwrap();
        assert_eq!(store.midi_output(), None);
    }

    #[test]
    fn the_row_labels_setting_is_kept() {
        let store = temp_store("row-labels");
        assert_eq!(store.row_labels(), RowLabels::Names);
        for labels in [
            RowLabels::PicturesAndNames,
            RowLabels::Pictures,
            RowLabels::Names,
        ] {
            store.set_row_labels(labels).unwrap();
            assert_eq!(store.row_labels(), labels);
        }
        // Names leaves no file behind, and setting it twice is fine.
        store.set_row_labels(RowLabels::Names).unwrap();
        assert_eq!(store.row_labels(), RowLabels::Names);
    }
}
