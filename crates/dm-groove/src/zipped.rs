//! Reads the MIDI files and styles out of a zip, and picks the General MIDI folder when a pack holds one
//! version of its grooves per drum program.

use std::io::{Cursor, Read};

use crate::groove::MAX_MIDI_BYTES;
use crate::{ImportError, MAX_ZIP_ENTRIES, MAX_ZIP_UNPACKED_BYTES};

/// One MIDI file from the zip.
#[derive(Debug)]
pub struct Entry {
    /// The full path inside the zip, with `/` between parts.
    pub path: String,
    /// The folder shown for its groove: see [`read`].
    pub folder: String,
    /// The file's name without its extension.
    pub name: String,
    /// The bytes, or `None` if the file is larger than [`MAX_MIDI_BYTES`].
    pub bytes: Option<Vec<u8>>,
}

/// The MIDI files to import, in path order, and how many were skipped as other drum programs'
/// versions.
#[derive(Debug)]
pub struct Contents {
    pub entries: Vec<Entry>,
    pub other_mappings_skipped: usize,
}

/// Reads the MIDI files in a zip.
///
/// A file counts as MIDI by its name: `.mid` or `.midi`, or one of a Yamaha style's extensions
/// (`.sty`, `.prs`, `.sst`, `.scp`, `.fps`, `.bcs`), in any case. Its content is checked later,
/// and decides whether it is read as a style. Folders,
/// macOS `__MACOSX` folders and dot-files are ignored, as are files of any other kind.
///
/// When the pack holds a folder named for General MIDI (see [`is_general_midi`]) next to other
/// folders of MIDI files, those other folders are versions for other drum programs: only the
/// General MIDI folder is read. Each groove's folder is then its path inside that folder.
/// Otherwise every MIDI file is read, and the folder is its path without the top folder they all
/// share, if any.
pub fn read(bytes: &[u8]) -> Result<Contents, ImportError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| ImportError::BrokenZip)?;
    if zip.len() > MAX_ZIP_ENTRIES {
        return Err(ImportError::TooLarge);
    }

    // The folders and file name of every MIDI file, with its index in the zip.
    let mut midi: Vec<(usize, Vec<String>)> = Vec::new();
    let mut unpacked = 0u64;
    for index in 0..zip.len() {
        let file = zip
            .by_index_raw(index)
            .map_err(|_| ImportError::BrokenZip)?;
        unpacked = unpacked.saturating_add(file.size());
        if unpacked > MAX_ZIP_UNPACKED_BYTES {
            return Err(ImportError::TooLarge);
        }
        if file.is_dir() {
            continue;
        }
        let parts: Vec<String> = file
            .name()
            .map_err(|_| ImportError::BrokenZip)?
            .split(['/', '\\'])
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        let hidden = parts.iter().any(|p| p.starts_with('.') || p == "__MACOSX");
        if !hidden && parts.last().is_some_and(|name| is_midi_name(name)) {
            midi.push((index, parts));
        }
    }
    if midi.is_empty() {
        return Err(ImportError::NoMidiFiles);
    }
    midi.sort_by(|a, b| a.1.cmp(&b.1));

    let (chosen, skipped) = choose(&midi);
    let mut entries = Vec::new();
    let mut read_total = 0u64;
    for (index, parts, folder) in chosen {
        let file = zip.by_index(index).map_err(|_| ImportError::BrokenZip)?;
        // Never read more than one byte past the limit, whatever the zip claims.
        let mut data = Vec::new();
        file.take(MAX_MIDI_BYTES as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| ImportError::BrokenZip)?;
        read_total += data.len() as u64;
        if read_total > MAX_ZIP_UNPACKED_BYTES {
            return Err(ImportError::TooLarge);
        }
        let file_name = parts.last().expect("a file has a name");
        entries.push(Entry {
            path: parts.join("/"),
            folder: folder.join("/"),
            name: stem(file_name).to_owned(),
            bytes: (data.len() <= MAX_MIDI_BYTES).then_some(data),
        });
    }
    Ok(Contents {
        entries,
        other_mappings_skipped: skipped,
    })
}

/// A file to read: its index in the zip, the parts of its path, and the parts of its folder.
type Chosen<'a> = (usize, &'a [String], &'a [String]);

/// The files to read, and the number skipped as versions for other drum programs.
fn choose(midi: &[(usize, Vec<String>)]) -> (Vec<Chosen<'_>>, usize) {
    // A General MIDI folder, as (its parent's parts, its own name), with another folder of MIDI
    // files beside it.
    let general_midi: Vec<(&[String], &String)> = midi
        .iter()
        .flat_map(|(_, parts)| {
            let folders = &parts[..parts.len() - 1];
            folders
                .iter()
                .enumerate()
                .filter(|(_, name)| is_general_midi(name))
                .map(move |(depth, name)| (&folders[..depth], name))
        })
        .filter(|&(parent, name)| {
            midi.iter().any(|(_, other)| {
                other.len() > parent.len() + 1
                    && other.starts_with(parent)
                    && other[parent.len()] != *name
            })
        })
        .collect();

    if !general_midi.is_empty() {
        let mut chosen = Vec::new();
        let mut skipped = 0;
        for (index, parts) in midi {
            let inside = general_midi.iter().find_map(|&(parent, name)| {
                (parts.len() > parent.len() + 1
                    && parts.starts_with(parent)
                    && parts[parent.len()] == *name)
                    .then_some(parent.len() + 1)
            });
            match inside {
                Some(start) => chosen.push((*index, &parts[..], &parts[start..parts.len() - 1])),
                None => skipped += 1,
            }
        }
        return (chosen, skipped);
    }

    // Leave out the top folder when every file is inside it.
    let top = &midi[0].1[0];
    let shared = midi
        .iter()
        .all(|(_, parts)| parts.len() > 1 && parts[0] == *top);
    let start = usize::from(shared);
    let chosen = midi
        .iter()
        .map(|(index, parts)| (*index, &parts[..], &parts[start..parts.len() - 1]))
        .collect();
    (chosen, 0)
}

/// Whether a folder's name says it holds the General MIDI version of a pack: "GM",
/// "General MIDI", "GM Mapping", or a name ending in one of these, such as "Beat Farm GM".
/// Case, `_` and `-` are ignored.
pub fn is_general_midi(name: &str) -> bool {
    let lower = name.to_lowercase();
    let words: Vec<&str> = lower
        .split([' ', '_', '-', '.'])
        .filter(|w| !w.is_empty())
        .collect();
    let words = match words.as_slice() {
        [rest @ .., "mapping" | "map" | "version"] => rest,
        all => all,
    };
    matches!(words, [.., "gm"] | [.., "general", "midi"])
}

/// The extensions of the files read from a zip: MIDI files and Yamaha's styles.
const EXTENSIONS: [&str; 8] = [
    ".mid", ".midi", ".sty", ".prs", ".sst", ".scp", ".fps", ".bcs",
];

fn is_midi_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    EXTENSIONS
        .iter()
        .any(|extension| lower.ends_with(extension))
}

/// A file name without its extension. A name that is all extension, such as `.mid`, stays whole.
pub fn stem(name: &str) -> &str {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}
