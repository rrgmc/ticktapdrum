//! The SoundFonts a person added, kept in `<data dir>/soundfonts/`.
//!
//! See "Milestone 15: other SoundFonts" in `docs/DESIGN.md`. Files in that folder:
//!
//! - `<name>.sf2`: an added SoundFont, such as `GeneralUser-GS.sf2`. Its file name identifies it.
//! - `<name>.sf2.sha256`: its digest and size, written when it was added, so listing never reads
//!   the SoundFont itself. A SoundFont without a readable one, or whose size differs from the
//!   recorded size, is not listed.
//! - `.adding-<n>.part`: a copy in progress. Deleted when the copy fails or is cancelled; one
//!   left by a crash is ignored by the listing.
//!
//! This module does not read what is inside a SoundFont. The caller checks it has drum kits and
//! calls [`remove_soundfont`] if it has none.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use sha2::{Digest, Sha256};

use crate::{BankError, CHUNK, Cleanup, MIN_REPORT_STEP, check_cancel, remove_if_present};

/// The folder, under the data folder, that holds the SoundFonts the person added.
pub const SOUNDFONTS_DIR: &str = "soundfonts";

/// The extension every added SoundFont has.
const EXTENSION: &str = ".sf2";

/// The extension of the file kept beside each SoundFont, after its own name.
const DIGEST_EXTENSION: &str = ".sha256";

/// The name used when cleaning leaves nothing of the name given.
const FALLBACK_STEM: &str = "SoundFont";

/// The longest stem kept, in characters. Leaves room for " (99)" and the extensions within the
/// 255-byte limit most file systems have, even with characters that take several bytes.
const MAX_STEM_CHARS: usize = 100;

/// Characters Windows does not allow in file names. Android and iOS allow a subset of these, so
/// avoiding them everywhere keeps a name valid on every platform.
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Names Windows reserves for devices, whatever the extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A SoundFont the person added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSoundfont {
    /// The file name, ending in `.sf2`, such as `GeneralUser-GS.sf2`. It identifies the
    /// SoundFont, for example in settings and project files.
    pub name: String,
    /// Where the file is.
    pub path: PathBuf,
    /// The file's size in bytes.
    pub bytes: u64,
    /// SHA-256 of the file, lowercase hex.
    pub sha256: String,
}

/// The folder that holds the added SoundFonts, for the data folder `dir`.
pub fn soundfonts_dir(dir: &Path) -> PathBuf {
    dir.join(SOUNDFONTS_DIR)
}

/// The file name [`add_soundfont`] starts from for the name a person knows a file by.
///
/// Keeps only the last part of a path, drops characters that are not allowed in file names,
/// trims spaces and dots from both ends, shortens a very long name, and gives it the `.sf2`
/// extension (in lower case) if it lacks it. A name that cleaning leaves empty becomes
/// `SoundFont.sf2`.
pub fn soundfont_file_name(name: &str) -> String {
    let last = name
        .rsplit(['/', '\\'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or("");
    let kept: String = last
        .chars()
        .filter(|c| !c.is_control() && !FORBIDDEN.contains(c))
        .collect();
    // Trim only the end before looking for the extension, so `.sf2` alone counts as one.
    let mut stem = kept.trim_end_matches(|c: char| c.is_whitespace() || c == '.');
    if stem.len() >= EXTENSION.len()
        && stem.is_char_boundary(stem.len() - EXTENSION.len())
        && stem[stem.len() - EXTENSION.len()..].eq_ignore_ascii_case(EXTENSION)
    {
        stem = trim_name(&stem[..stem.len() - EXTENSION.len()]);
    }
    let mut stem: String = stem.chars().take(MAX_STEM_CHARS).collect();
    stem = trim_name(&stem).to_owned();
    if stem.is_empty() {
        stem = FALLBACK_STEM.to_owned();
    }
    // A reserved device name is reserved with any extension, and also before the first dot.
    let before_dot = stem.split('.').next().unwrap_or("").trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(before_dot)) {
        stem.insert(before_dot.len(), '_');
    }
    format!("{stem}{EXTENSION}")
}

/// Trims spaces and dots from both ends. Windows drops trailing ones, and a leading dot hides a
/// file on Unix systems.
fn trim_name(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '.')
}

/// Copies a SoundFont into the soundfonts folder under the data folder `dir`, computing its
/// digest and size on the way. Blocking: run it on a worker thread.
///
/// `name` is the name the person knows the file by. It is cleaned as
/// [`soundfont_file_name`] says. If a SoundFont with that name (case ignored) and the same digest
/// is already there, nothing is kept and that one is returned. If the one there has another
/// digest, the next free name of `<stem> (2).sf2`, `<stem> (3).sf2`, and so on is used, again
/// returning an existing one with the same digest instead of a second copy.
///
/// The copy goes to a temporary file first and is renamed when complete, so a cancelled or
/// failed copy leaves nothing behind. `progress` gets the bytes copied so far, at most once per
/// 256 KiB and once at the end.
pub fn add_soundfont(
    dir: &Path,
    name: &str,
    source: &mut dyn Read,
    progress: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<UserSoundfont, BankError> {
    let folder = soundfonts_dir(dir);
    fs::create_dir_all(&folder)?;
    let file_name = soundfont_file_name(name);
    check_cancel(cancel)?;

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let part = folder.join(format!(
        ".adding-{}-{}.part",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut out = File::create(&part)?;
    let guard = Cleanup::new(&part);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    let mut next_report = 0u64;
    loop {
        if done >= next_report {
            progress(done);
            next_report = done + MIN_REPORT_STEP;
        }
        check_cancel(cancel)?;
        let n = match source.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        done += n as u64;
    }
    progress(done);
    out.sync_all()?;
    drop(out);
    let sha256 = hex::encode(hasher.finalize());

    let (stem, _) = file_name.split_at(file_name.len() - EXTENSION.len());
    let present = soundfonts(dir);
    for n in 1u32.. {
        let candidate = if n == 1 {
            file_name.clone()
        } else {
            format!("{stem} ({n}){EXTENSION}")
        };
        match present
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(&candidate))
        {
            Some(same) if same.sha256 == sha256 => {
                // The guard deletes the copy.
                return Ok(same.clone());
            }
            Some(_) => continue,
            None => {}
        }
        let path = folder.join(&candidate);
        // A file here that is not listed has no valid digest file, so it was never completely
        // added. Replace it. Remove it first, so a digest file never names an old file.
        remove_if_present(&path)?;
        write_digest(&digest_path(&path), &sha256, done)?;
        if let Err(e) = fs::rename(&part, &path) {
            let _ = remove_if_present(&digest_path(&path));
            return Err(e.into());
        }
        guard.keep();
        return Ok(UserSoundfont {
            name: candidate,
            path,
            bytes: done,
            sha256,
        });
    }
    unreachable!("every name up to u32::MAX is taken")
}

/// The added SoundFonts in the data folder `dir`, sorted by name, case ignored. Cheap: reads
/// each one's digest file and size, not the SoundFont. Leaves out files without a valid digest
/// file and copies still in progress.
pub fn soundfonts(dir: &Path) -> Vec<UserSoundfont> {
    let folder = soundfonts_dir(dir);
    let Ok(entries) = fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut list: Vec<UserSoundfont> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if !has_extension(&name) || name.starts_with('.') {
                return None;
            }
            read_entry(&folder, &name)
        })
        .collect();
    list.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    list
}

/// The added SoundFont with this file name, if it is there. An exact match wins; otherwise one
/// whose name differs only in case.
pub fn find_soundfont(dir: &Path, name: &str) -> Option<UserSoundfont> {
    let list = soundfonts(dir);
    if let Some(found) = list.iter().find(|s| s.name == name) {
        return Some(found.clone());
    }
    list.into_iter().find(|s| s.name.eq_ignore_ascii_case(name))
}

/// Deletes an added SoundFont and its digest file. Deleting one that is not there succeeds.
/// `name` must be a plain file name, as [`UserSoundfont::name`] is.
pub fn remove_soundfont(dir: &Path, name: &str) -> Result<(), BankError> {
    if name.is_empty()
        || name.contains(['/', '\\'])
        || name == "."
        || name == ".."
        || !has_extension(name)
    {
        return Err(BankError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not the name of an added SoundFont: {name:?}"),
        )));
    }
    let path = soundfonts_dir(dir).join(name);
    // The SoundFont goes first, so a digest file never outlives it by naming a file that is gone
    // halfway through.
    remove_if_present(&path)?;
    remove_if_present(&digest_path(&path))?;
    Ok(())
}

fn has_extension(name: &str) -> bool {
    name.len() > EXTENSION.len()
        && name.is_char_boundary(name.len() - EXTENSION.len())
        && name[name.len() - EXTENSION.len()..].eq_ignore_ascii_case(EXTENSION)
}

/// The digest file beside the SoundFont at `path`.
fn digest_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(DIGEST_EXTENSION);
    PathBuf::from(s)
}

/// What a digest file holds: the digest, then the size, one per line.
fn digest_text(sha256: &str, bytes: u64) -> String {
    format!("{sha256}\n{bytes}\n")
}

/// Writes a digest file through a temporary file, so it is either whole or absent.
fn write_digest(path: &Path, sha256: &str, bytes: u64) -> Result<(), BankError> {
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let guard = Cleanup::new(&part);
    {
        let mut f = File::create(&part)?;
        f.write_all(digest_text(sha256, bytes).as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&part, path)?;
    guard.keep();
    Ok(())
}

/// Reads one SoundFont's digest file and checks it against the file's size.
fn read_entry(folder: &Path, name: &str) -> Option<UserSoundfont> {
    let path = folder.join(name);
    let meta = fs::metadata(&path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let text = fs::read_to_string(digest_path(&path)).ok()?;
    let mut lines = text.lines();
    let sha256 = lines.next()?.trim();
    let bytes: u64 = lines.next()?.trim().parse().ok()?;
    let valid = sha256.len() == 64
        && sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !valid || bytes != meta.len() {
        return None;
    }
    Some(UserSoundfont {
        name: name.to_owned(),
        path,
        bytes,
        sha256: sha256.to_owned(),
    })
}
