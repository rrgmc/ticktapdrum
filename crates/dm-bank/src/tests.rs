use std::cell::RefCell;
use std::sync::atomic::AtomicU32;
use std::time::Instant;

use zip::write::SimpleFileOptions;

use super::*;

const MEMBER: &str = "Test_Bank.sf2";

/// A fresh, empty folder under the system temp folder, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("dm-bank-test-{}-{name}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Pseudo-random bytes, so deflate cannot shrink the member to nothing.
fn member_bytes(len: usize) -> Vec<u8> {
    let mut x: u32 = 0x1234_5678;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

/// Writes a zip holding one member into `dir` and returns the matching bank description.
fn synthetic_bank(dir: &Path, id: &str, member_len: usize) -> (BankInfo, PathBuf) {
    let member = member_bytes(member_len);
    let archive = dir.join(format!("{id}-source.zip"));
    let mut writer = zip::ZipWriter::new(File::create(&archive).unwrap());
    writer
        .start_file(
            MEMBER,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(&member).unwrap();
    writer.finish().unwrap();
    let archive_data = fs::read(&archive).unwrap();

    let bank = BankInfo {
        id: leak(id.to_owned()),
        name: "Test bank",
        url: "https://example.invalid/test.zip",
        archive_bytes: archive_data.len() as u64,
        archive_sha256: leak(sha256_hex(&archive_data)),
        member: MEMBER,
        member_bytes: member.len() as u64,
        member_sha256: leak(sha256_hex(&member)),
        license: "test",
    };
    (bank, archive)
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn no_progress(_: Progress) {}

#[test]
fn install_from_archive_succeeds() {
    let src = TempDir::new("src-ok");
    let dst = TempDir::new("dst-ok");
    let (bank, archive) = synthetic_bank(&src.0, "test-ok", 1 << 20);
    let seen = RefCell::new(Vec::new());
    let cancel = AtomicBool::new(false);

    let path = install_from_archive(
        &dst.0,
        &bank,
        &archive,
        &|p| seen.borrow_mut().push(p),
        &cancel,
    )
    .unwrap();

    assert_eq!(path, dst.0.join(MEMBER));
    assert_eq!(fs::read(&path).unwrap(), member_bytes(1 << 20));
    assert_eq!(
        names_in(&dst.0),
        vec![MEMBER.to_owned(), "test-ok.installed".to_owned()]
    );
    assert!(archive.exists(), "the caller's archive must stay");
    let seen = seen.into_inner();
    assert_eq!(seen.first(), Some(&Progress::VerifyingArchive));
    assert_eq!(seen.last(), Some(&Progress::VerifyingMember));
    assert!(seen.contains(&Progress::Extracting {
        done: 1 << 20,
        total: 1 << 20
    }));
}

#[test]
fn wrong_archive_digest_is_a_mismatch() {
    let src = TempDir::new("src-badzip");
    let dst = TempDir::new("dst-badzip");
    let (mut bank, archive) = synthetic_bank(&src.0, "test-badzip", 4096);
    bank.archive_sha256 = leak("0".repeat(64));

    let err = install_from_archive(
        &dst.0,
        &bank,
        &archive,
        &no_progress,
        &AtomicBool::new(false),
    )
    .unwrap_err();

    assert!(
        matches!(
            err,
            BankError::Mismatch {
                what: "the archive"
            }
        ),
        "{err:?}"
    );
    assert!(names_in(&dst.0).is_empty());
    assert!(
        archive.exists(),
        "the caller's archive must stay even when it does not match"
    );
}

#[test]
fn wrong_member_digest_is_a_mismatch() {
    let src = TempDir::new("src-badsf2");
    let dst = TempDir::new("dst-badsf2");
    let (mut bank, archive) = synthetic_bank(&src.0, "test-badsf2", 4096);
    bank.member_sha256 = leak("0".repeat(64));

    let err = install_from_archive(
        &dst.0,
        &bank,
        &archive,
        &no_progress,
        &AtomicBool::new(false),
    )
    .unwrap_err();

    assert!(
        matches!(
            err,
            BankError::Mismatch {
                what: "the bank file"
            }
        ),
        "{err:?}"
    );
    assert!(names_in(&dst.0).is_empty());
    assert_eq!(installed(&dst.0, &bank), None);
}

#[test]
fn missing_member_is_an_archive_error() {
    let src = TempDir::new("src-nomember");
    let dst = TempDir::new("dst-nomember");
    let (mut bank, archive) = synthetic_bank(&src.0, "test-nomember", 4096);
    bank.member = "Other.sf2";

    let err = install_from_archive(
        &dst.0,
        &bank,
        &archive,
        &no_progress,
        &AtomicBool::new(false),
    )
    .unwrap_err();

    assert!(matches!(err, BankError::Archive(_)), "{err:?}");
    assert!(names_in(&dst.0).is_empty());
}

#[test]
fn installed_follows_marker_and_size() {
    let src = TempDir::new("src-installed");
    let dst = TempDir::new("dst-installed");
    let (bank, archive) = synthetic_bank(&src.0, "test-installed", 8192);
    let cancel = AtomicBool::new(false);

    assert_eq!(installed(&dst.0, &bank), None);
    let path = install_from_archive(&dst.0, &bank, &archive, &no_progress, &cancel).unwrap();
    assert_eq!(installed(&dst.0, &bank), Some(path.clone()));

    // A marker for another version of the bank does not count.
    let mut other = bank;
    other.member_sha256 = leak("1".repeat(64));
    assert_eq!(installed(&dst.0, &other), None);

    // A truncated bank file does not count.
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(100)
        .unwrap();
    assert_eq!(installed(&dst.0, &bank), None);

    // Reinstalling repairs it; then a missing marker does not count.
    install_from_archive(&dst.0, &bank, &archive, &no_progress, &cancel).unwrap();
    assert_eq!(installed(&dst.0, &bank), Some(path));
    fs::remove_file(dst.0.join("test-installed.installed")).unwrap();
    assert_eq!(installed(&dst.0, &bank), None);
}

#[test]
fn cancel_during_extraction_leaves_nothing() {
    let src = TempDir::new("src-cancel");
    let dst = TempDir::new("dst-cancel");
    let (bank, archive) = synthetic_bank(&src.0, "test-cancel", 1 << 20);
    let cancel = AtomicBool::new(false);
    let saw_partial = std::cell::Cell::new(false);

    // Cancel once some of the member is on disk.
    let progress = |p: Progress| {
        if let Progress::Extracting { done, .. } = p
            && done > 0
        {
            saw_partial.set(dst.0.join("test-cancel.sf2.part").exists());
            cancel.store(true, Ordering::Relaxed);
        }
    };
    let err = install_from_archive(&dst.0, &bank, &archive, &progress, &cancel).unwrap_err();

    assert!(matches!(err, BankError::Cancelled), "{err:?}");
    assert!(
        saw_partial.get(),
        "the cancel came before any partial file existed"
    );
    assert!(names_in(&dst.0).is_empty());
}

#[test]
fn cancel_before_start_leaves_nothing() {
    let src = TempDir::new("src-cancel0");
    let dst = TempDir::new("dst-cancel0");
    let (bank, archive) = synthetic_bank(&src.0, "test-cancel0", 4096);

    let err = install_from_archive(
        &dst.0,
        &bank,
        &archive,
        &no_progress,
        &AtomicBool::new(true),
    )
    .unwrap_err();

    assert!(matches!(err, BankError::Cancelled), "{err:?}");
    assert!(names_in(&dst.0).is_empty());
}

#[test]
fn install_reuses_a_verified_archive() {
    let src = TempDir::new("src-reuse");
    let dst = TempDir::new("dst-reuse");
    let (bank, archive) = synthetic_bank(&src.0, "test-reuse", 4096);
    // As if an earlier install downloaded the archive and stopped before extracting.
    fs::copy(&archive, dst.0.join("test-reuse.zip")).unwrap();
    let seen = RefCell::new(Vec::new());

    // The bank's address is unreachable, so this only passes without a download.
    let path = install(
        &dst.0,
        &bank,
        &|p| seen.borrow_mut().push(p),
        &AtomicBool::new(false),
    )
    .unwrap();

    assert_eq!(installed(&dst.0, &bank), Some(path));
    assert_eq!(
        names_in(&dst.0),
        vec![MEMBER.to_owned(), "test-reuse.installed".to_owned()]
    );
    assert!(
        !seen
            .into_inner()
            .iter()
            .any(|p| matches!(p, Progress::Downloading { .. }))
    );
}

#[test]
fn install_discards_a_bad_stored_archive() {
    let dst = TempDir::new("dst-badstored");
    let src = TempDir::new("src-badstored");
    let (bank, _) = synthetic_bank(&src.0, "test-badstored", 4096);
    fs::write(dst.0.join("test-badstored.zip"), b"not the archive").unwrap();

    // The bad copy is deleted, then the download fails because the address is unreachable.
    let err = install(&dst.0, &bank, &no_progress, &AtomicBool::new(false)).unwrap_err();

    assert!(matches!(err, BankError::Download(_)), "{err:?}");
    assert!(names_in(&dst.0).is_empty());
}

/// Installs the real bank from a local copy of its archive, named by `DM_TEST_BANK_ZIP`.
/// Run with `cargo test -p dm-bank --release -- --ignored`.
#[test]
#[ignore = "needs DM_TEST_BANK_ZIP pointing at the real archive"]
fn installs_real_bank_from_local_archive() {
    let archive = PathBuf::from(
        std::env::var_os("DM_TEST_BANK_ZIP").expect("set DM_TEST_BANK_ZIP to the archive's path"),
    );
    let dst = TempDir::new("real");
    let started = Instant::now();
    let phase_start = RefCell::new((Instant::now(), None::<&'static str>));
    let progress = |p: Progress| {
        let name = match p {
            Progress::Downloading { .. } => "downloading",
            Progress::VerifyingArchive => "verifying archive",
            Progress::Extracting { .. } => "extracting",
            Progress::VerifyingMember => "verifying member",
        };
        let mut phase = phase_start.borrow_mut();
        if phase.1 != Some(name) {
            if let Some(prev) = phase.1 {
                eprintln!("{prev}: {:.2?}", phase.0.elapsed());
            }
            *phase = (Instant::now(), Some(name));
        }
    };

    let path = install_from_archive(
        &dst.0,
        &COLOMBO,
        &archive,
        &progress,
        &AtomicBool::new(false),
    )
    .unwrap();
    eprintln!("total: {:.2?}", started.elapsed());

    assert_eq!(installed(&dst.0, &COLOMBO), Some(path.clone()));
    assert_eq!(fs::metadata(&path).unwrap().len(), COLOMBO.member_bytes);
}

/// Fetches the first KiB of the real bank's address, to prove TLS and redirects work without
/// downloading the whole archive.
#[test]
#[ignore = "uses the network"]
fn reaches_real_bank_address() {
    let response = agent()
        .get(COLOMBO.url)
        .header("Range", "bytes=0-1023")
        .call()
        .unwrap();
    let status = response.status().as_u16();
    assert!(status == 206 || status == 200, "status {status}");
    let mut head = Vec::new();
    response
        .into_body()
        .into_reader()
        .take(1024)
        .read_to_end(&mut head)
        .unwrap();
    assert_eq!(head.len(), 1024);
    assert_eq!(&head[..4], b"PK\x03\x04", "not the start of a zip");
}
