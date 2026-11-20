//! What other apps share with TickTapDrum, or open in it: a link, or a MIDI or zip file.
//!
//! Android delivers a share or an "Open with" as an intent to the activity. `MainActivity` in
//! `ports/android` reads it, when the app starts and when a share reaches the running app, and
//! calls the native method `onShared` below with the text or the file's address. Here:
//!
//! - text goes to the interface through `IncomingFiles::send_text`;
//! - a `content://` address is opened through the `ContentResolver`, as the picker opens a file
//!   (see `picker.rs`), and a `file://` address is opened as a path. Both are read on a thread
//!   of their own and the bytes go to `IncomingFiles::send`.
//!
//! The intent that starts the app arrives before the interface exists. What comes before then
//! waits in a queue, and [`install`] hands it on once the interface is ready.
//!
//! See "Milestone 12: share to the app on Android" in `docs/DESIGN.md`.

use std::io::Read;
use std::sync::Mutex;

use android_activity::AndroidApp;
use dm_ui::IncomingFiles;
use jni::EnvUnowned;
use jni::objects::{JClass, JString};

use crate::picker;

/// The most bytes read from a shared file. The interface refuses a file larger than 100 MB
/// (`grooves::MAX_FILE` in `dm-ui`) with a "too large" message, so one byte more is read: enough
/// for the interface to see that the file is too large and say so, without reading all of it.
const READ_LIMIT: u64 = 100 * 1024 * 1024 + 1;

/// Something shared with the app.
enum Shared {
    /// Text, such as a link.
    Text(String),
    /// A file's address: `content://...` or `file://...`.
    Uri(String),
}

/// Where shares go, and what arrived before there was anywhere for it to go.
struct Shares {
    /// Set once the interface exists. A new start of the app replaces it.
    target: Option<(AndroidApp, IncomingFiles)>,
    /// Shares that arrived before the interface existed, oldest first.
    waiting: Vec<Shared>,
}

static SHARES: Mutex<Shares> = Mutex::new(Shares {
    target: None,
    waiting: Vec::new(),
});

fn shares() -> std::sync::MutexGuard<'static, Shares> {
    // A panic while the lock was held leaves nothing half done, so a poisoned lock is used as it
    // is.
    SHARES.lock().unwrap_or_else(|e| e.into_inner())
}

/// Makes sharing ready: shares go to `incoming`. Hands on any share that arrived earlier.
pub fn install(app: AndroidApp, incoming: IncomingFiles) {
    let waiting = {
        let mut shares = shares();
        shares.target = Some((app.clone(), incoming.clone()));
        std::mem::take(&mut shares.waiting)
    };
    for shared in waiting {
        deliver(&app, &incoming, shared);
    }
}

/// `MainActivity.onShared(String text, String uri)`, which Java calls on the UI thread for a
/// share or an "Open with". One of the two is null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_rrgmc_ticktapdrum_MainActivity_onShared<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    text: JString<'local>,
    uri: JString<'local>,
) {
    // with_env catches a panic, so none crosses into Java.
    env.with_env(|env| -> jni::errors::Result<()> {
        let shared = if !uri.is_null() {
            Shared::Uri(uri.try_to_string(env)?)
        } else if !text.is_null() {
            Shared::Text(text.try_to_string(env)?)
        } else {
            return Ok(());
        };
        arrived(shared);
        Ok(())
    })
    .resolve::<jni::errors::LogErrorAndDefault>()
}

/// Hands `shared` on, or keeps it until the interface exists.
fn arrived(shared: Shared) {
    let mut shares = shares();
    let Some((app, incoming)) = shares.target.clone() else {
        log::info!("a share arrived before the interface was ready; it waits");
        shares.waiting.push(shared);
        return;
    };
    drop(shares);
    deliver(&app, &incoming, shared);
}

/// Sends text to the interface at once. Reads a file on a thread of its own, since a provider
/// can be slow, and sends its bytes.
fn deliver(app: &AndroidApp, incoming: &IncomingFiles, shared: Shared) {
    let uri = match shared {
        Shared::Text(text) => {
            log::info!("shared text: {} characters", text.chars().count());
            incoming.send_text(text);
            return;
        }
        Shared::Uri(uri) => uri,
    };
    let app = app.clone();
    let incoming = incoming.clone();
    let spawned = std::thread::Builder::new()
        .name("shared file".into())
        .spawn(move || match read(&app, &uri) {
            Ok((name, bytes)) => {
                log::info!("shared file {name}: read {} bytes", bytes.len());
                incoming.send(name, bytes);
            }
            Err(e) => log::warn!("could not read the shared file {uri}: {e}"),
        });
    if let Err(e) = spawned {
        log::warn!("could not start a thread to read a shared file: {e}");
    }
}

/// Reads the file at `uri`, up to [`READ_LIMIT`] bytes, and gives its name and bytes.
fn read(app: &AndroidApp, uri: &str) -> Result<(String, Vec<u8>), String> {
    let (file, name) = match uri.strip_prefix("file://") {
        // A file address names a path. Android lets an app send one only in rare cases, such as
        // from adb, and the app may not be allowed to read the path.
        Some(path) => {
            let path = picker::percent_decoded(path);
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            (file, picker::last_segment(&path))
        }
        None => {
            let file = picker::open_document(app, uri, "r").map_err(|e| e.to_string())?;
            (file, picker::document_name(app, uri))
        }
    };
    let mut bytes = Vec::new();
    file.take(READ_LIMIT)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok((name, bytes))
}
