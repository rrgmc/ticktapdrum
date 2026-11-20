//! Downloads, verifies and installs the instrument bank.
//!
//! The bank is never bundled. See "How the bank is downloaded" in `docs/DESIGN.md`.
//!
//! Files in the bank folder, for a bank with id `<id>`:
//!
//! - `<id>.zip.part`: the archive while it downloads.
//! - `<id>.zip`: the archive after its size and digest matched. Deleted once the member is
//!   installed, and reused if an earlier install stopped before that.
//! - `<id>.sf2.part`: the member while it is extracted.
//! - the member's own name, such as `ColomboGMGS2_Vanilla.sf2`: the installed bank.
//! - `<id>.installed`: the marker, written only after the member's size and digest matched.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// A bank the app knows how to fetch: a zip at a pinned address holding one `.sf2`.
#[derive(Debug, Clone, Copy)]
pub struct BankInfo {
    /// Stable id, also used in project files, such as `colombogmgs2-17.11`.
    pub id: &'static str,
    /// Name shown to the user.
    pub name: &'static str,
    /// Direct download address of the archive.
    pub url: &'static str,
    /// Exact size of the archive in bytes.
    pub archive_bytes: u64,
    /// SHA-256 of the archive, lowercase hex.
    pub archive_sha256: &'static str,
    /// Name of the `.sf2` inside the archive.
    pub member: &'static str,
    /// Exact size of the `.sf2` in bytes.
    pub member_bytes: u64,
    /// SHA-256 of the `.sf2`, lowercase hex.
    pub member_sha256: &'static str,
    /// The licence line shown beside the offer to download.
    pub license: &'static str,
}

/// ColomboGMGS2 Vanilla 17.11, the bank karaokemachine recommends, from the copy on this repository's release.
pub const COLOMBO: BankInfo = BankInfo {
    id: "colombogmgs2-17.11",
    name: "ColomboGMGS2 Vanilla 17.11",
    url: "https://github.com/rrgmc/ticktapdrum/releases/download/bank-colombogmgs2-17.11/ColomboGMGS2_VANILLA.zip",
    archive_bytes: 237_967_501,
    archive_sha256: "5ed5ba3516bcc139d4c9e3854338c892510e8d6f34a40fb2a9b423692eff411a",
    member: "ColomboGMGS2_Vanilla.sf2",
    member_bytes: 274_474_922,
    member_sha256: "6a3197ad7d6d9528f204c683316aaed2b9b09db562833653bfe21a7ab1521dbb",
    license: "CC-BY-SA 4.0, claimed in the file; its credits name other banks among its sample sources",
};

/// How far an install has got, reported to the interface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    /// Bytes of the archive received so far, of `total`.
    Downloading { done: u64, total: u64 },
    /// Checking the archive's digest.
    VerifyingArchive,
    /// Bytes of the `.sf2` extracted so far, of `total`.
    Extracting { done: u64, total: u64 },
    /// Checking the `.sf2`'s digest.
    VerifyingMember,
}

/// What can go wrong installing a bank.
#[derive(Debug, thiserror::Error)]
pub enum BankError {
    /// Network failure or an unexpected HTTP status.
    #[error("download failed: {0}")]
    Download(String),
    /// A file's digest or size did not match the pinned value. The file was deleted.
    #[error("{what} did not match its pinned digest")]
    Mismatch { what: &'static str },
    /// The archive could not be read, or lacks the member.
    #[error("archive error: {0}")]
    Archive(String),
    /// A local file operation failed.
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    /// The cancel flag was set.
    #[error("cancelled")]
    Cancelled,
}

/// Sent with every request. Generic on purpose: it names the program, nothing about the user.
const USER_AGENT: &str = concat!("ticktapdrum/", env!("CARGO_PKG_VERSION"));

/// Size of each read while downloading, hashing or extracting.
const CHUNK: usize = 64 * 1024;

/// The smallest step between two progress reports, in bytes.
const MIN_REPORT_STEP: u64 = 256 * 1024;

/// The installed `.sf2`, if `dir` holds a copy that passed verification when it was installed.
/// Cheap: checks a marker written after verification and the file's size, not the full digest.
pub fn installed(dir: &Path, bank: &BankInfo) -> Option<PathBuf> {
    let files = Files::new(dir, bank);
    let marker = fs::read_to_string(&files.marker).ok()?;
    if marker.trim() != marker_text(bank).trim() {
        return None;
    }
    let len = fs::metadata(&files.member).ok()?.len();
    (len == bank.member_bytes).then_some(files.member)
}

/// Downloads the archive into `dir`, verifies it, extracts and verifies the member, and returns
/// the `.sf2` path. Blocking: run it on a worker thread. Removes partial and mismatched files.
///
/// Returns at once if the bank is already installed. Reuses a verified archive left in `dir` by
/// an install that stopped during extraction, after checking its digest again.
pub fn install(
    dir: &Path,
    bank: &BankInfo,
    progress: &dyn Fn(Progress),
    cancel: &AtomicBool,
) -> Result<PathBuf, BankError> {
    fs::create_dir_all(dir)?;
    if let Some(path) = installed(dir, bank) {
        return Ok(path);
    }
    let files = Files::new(dir, bank);

    let mut have_archive = false;
    if files.archive.exists() {
        progress(Progress::VerifyingArchive);
        match verify_file(
            &files.archive,
            bank.archive_bytes,
            bank.archive_sha256,
            cancel,
        ) {
            Ok(()) => have_archive = true,
            Err(BankError::Mismatch { .. }) => {
                log::warn!("discarding a stored archive that no longer matches its digest");
                fs::remove_file(&files.archive)?;
            }
            Err(e) => return Err(e),
        }
    }
    if !have_archive {
        download(bank, &files, progress, cancel)?;
    }

    let path = extract(bank, &files, &files.archive, progress, cancel)?;
    if let Err(e) = fs::remove_file(&files.archive) {
        log::warn!("could not delete the archive after installing: {e}");
    }
    Ok(path)
}

/// As [`install`], from an archive already on disk instead of the network. For seeding a
/// development machine; the archive is verified exactly as a download would be.
///
/// The caller's archive is never deleted, not even when it does not match.
pub fn install_from_archive(
    dir: &Path,
    bank: &BankInfo,
    archive: &Path,
    progress: &dyn Fn(Progress),
    cancel: &AtomicBool,
) -> Result<PathBuf, BankError> {
    fs::create_dir_all(dir)?;
    progress(Progress::VerifyingArchive);
    verify_file(archive, bank.archive_bytes, bank.archive_sha256, cancel)?;
    extract(bank, &Files::new(dir, bank), archive, progress, cancel)
}

/// The paths an install uses inside the bank folder.
struct Files {
    archive_part: PathBuf,
    archive: PathBuf,
    member_part: PathBuf,
    member: PathBuf,
    marker: PathBuf,
}

impl Files {
    fn new(dir: &Path, bank: &BankInfo) -> Self {
        Files {
            archive_part: dir.join(format!("{}.zip.part", bank.id)),
            archive: dir.join(format!("{}.zip", bank.id)),
            member_part: dir.join(format!("{}.sf2.part", bank.id)),
            member: dir.join(bank.member),
            marker: dir.join(format!("{}.installed", bank.id)),
        }
    }
}

/// What the marker holds: the member's digest and size, so a marker left by another version of
/// the bank does not count.
fn marker_text(bank: &BankInfo) -> String {
    format!("{}\n{}\n", bank.member_sha256, bank.member_bytes)
}

/// Deletes a file when dropped, unless [`Cleanup::keep`] was called. Covers every early return.
struct Cleanup<'a> {
    path: &'a Path,
    armed: bool,
}

impl<'a> Cleanup<'a> {
    fn new(path: &'a Path) -> Self {
        Cleanup { path, armed: true }
    }

    fn keep(mut self) {
        self.armed = false;
    }
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        if self.armed
            && let Err(e) = fs::remove_file(self.path)
            && e.kind() != io::ErrorKind::NotFound
        {
            log::warn!("could not delete {}: {e}", self.path.display());
        }
    }
}

/// Calls `progress` at most once per step of the larger of 256 KiB and 1% of the total.
struct Throttle {
    step: u64,
    next: u64,
}

impl Throttle {
    fn new(total: u64) -> Self {
        Throttle {
            step: (total / 100).max(MIN_REPORT_STEP),
            next: 0,
        }
    }

    fn due(&mut self, done: u64) -> bool {
        if done >= self.next {
            self.next = done + self.step;
            true
        } else {
            false
        }
    }
}

/// The HTTP client. It follows redirects (the default, up to 10), which GitHub release
/// addresses need.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .https_only(true)
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_per_read(Some(Duration::from_secs(60)))
        .build()
        .into()
}

/// Downloads the archive to its `.part` name while hashing it, checks it, and renames it to its
/// final name.
fn download(
    bank: &BankInfo,
    files: &Files,
    progress: &dyn Fn(Progress),
    cancel: &AtomicBool,
) -> Result<(), BankError> {
    let total = bank.archive_bytes;
    progress(Progress::Downloading { done: 0, total });
    check_cancel(cancel)?;

    let response = agent()
        .get(bank.url)
        .call()
        .map_err(|e| BankError::Download(e.to_string()))?;
    if let Some(len) = response.body().content_length()
        && len != total
    {
        return Err(BankError::Download(format!(
            "the server offers {len} bytes, expected {total}"
        )));
    }
    let mut body = response.into_body().into_reader();

    let mut out = File::create(&files.archive_part)?;
    let guard = Cleanup::new(&files.archive_part);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    let mut throttle = Throttle::new(total);
    throttle.due(0);
    loop {
        check_cancel(cancel)?;
        let n = match body.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(BankError::Download(e.to_string())),
        };
        done += n as u64;
        if done > total {
            return Err(BankError::Mismatch {
                what: "the archive",
            });
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        if throttle.due(done) {
            progress(Progress::Downloading { done, total });
        }
    }
    progress(Progress::Downloading { done, total });
    out.sync_all()?;
    drop(out);

    progress(Progress::VerifyingArchive);
    if done != total || hex::encode(hasher.finalize()) != bank.archive_sha256 {
        return Err(BankError::Mismatch {
            what: "the archive",
        });
    }
    fs::rename(&files.archive_part, &files.archive)?;
    guard.keep();
    Ok(())
}

/// Checks a file's size and digest without changing it.
fn verify_file(
    path: &Path,
    bytes: u64,
    sha256: &str,
    cancel: &AtomicBool,
) -> Result<(), BankError> {
    let mismatch = BankError::Mismatch {
        what: "the archive",
    };
    if fs::metadata(path)?.len() != bytes {
        return Err(mismatch);
    }
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        check_cancel(cancel)?;
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        };
        hasher.update(&buf[..n]);
    }
    if hex::encode(hasher.finalize()) != sha256 {
        return Err(mismatch);
    }
    Ok(())
}

/// Extracts the member from a verified archive to its `.part` name while hashing it, checks it,
/// renames it to its final name and writes the marker.
fn extract(
    bank: &BankInfo,
    files: &Files,
    archive: &Path,
    progress: &dyn Fn(Progress),
    cancel: &AtomicBool,
) -> Result<PathBuf, BankError> {
    let total = bank.member_bytes;
    progress(Progress::Extracting { done: 0, total });
    check_cancel(cancel)?;

    let mut zip = zip::ZipArchive::new(File::open(archive)?)
        .map_err(|e| BankError::Archive(e.to_string()))?;
    let entry = zip
        .by_name(bank.member)
        .map_err(|e| BankError::Archive(format!("{}: {e}", bank.member)))?;
    // Never read past one byte more than expected, so a damaged archive cannot fill the disk.
    let mut entry = entry.take(total + 1);

    let mut out = File::create(&files.member_part)?;
    let guard = Cleanup::new(&files.member_part);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    let mut throttle = Throttle::new(total);
    throttle.due(0);
    loop {
        check_cancel(cancel)?;
        let n = match entry.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(BankError::Archive(e.to_string())),
        };
        done += n as u64;
        if done > total {
            return Err(BankError::Mismatch {
                what: "the bank file",
            });
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])?;
        if throttle.due(done) {
            progress(Progress::Extracting { done, total });
        }
    }
    progress(Progress::Extracting { done, total });
    out.sync_all()?;
    drop(out);

    progress(Progress::VerifyingMember);
    if done != total || hex::encode(hasher.finalize()) != bank.member_sha256 {
        return Err(BankError::Mismatch {
            what: "the bank file",
        });
    }

    // Remove the old marker first, so no marker ever names a file that is being replaced.
    remove_if_present(&files.marker)?;
    fs::rename(&files.member_part, &files.member)?;
    guard.keep();

    let marker_part = files.marker.with_extension("installed.part");
    let marker_guard = Cleanup::new(&marker_part);
    {
        let mut f = File::create(&marker_part)?;
        f.write_all(marker_text(bank).as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&marker_part, &files.marker)?;
    marker_guard.keep();
    Ok(files.member.clone())
}

fn check_cancel(cancel: &AtomicBool) -> Result<(), BankError> {
    if cancel.load(Ordering::Relaxed) {
        Err(BankError::Cancelled)
    } else {
        Ok(())
    }
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
