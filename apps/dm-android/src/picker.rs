//! The system file picker, Android's Storage Access Framework, behind `DrumApp::with_file_picker`.
//!
//! A pick goes through the activity's Java class, `MainActivity` in `ports/android`:
//!
//! 1. The interface asks for a pick; [`pick`] calls `MainActivity.pickFile` through JNI, which
//!    starts the system picker on the UI thread.
//! 2. When the picker closes, `MainActivity.onActivityResult` calls the native method
//!    `onFilePicked` below with the chosen document's `content://` address, or null.
//! 3. On a thread of its own, [`open`] asks the `ContentResolver` for a file descriptor and the
//!    document's display name, and hands both to the interface through `PickAnswers`. Opening
//!    stays off the UI thread because a cloud document can take a while.
//!
//! See "Milestone 10: Android file picker" in `docs/DESIGN.md`.

use std::collections::HashMap;
use std::fs::File;
use std::os::fd::FromRawFd;
use std::sync::Mutex;

use android_activity::AndroidApp;
use dm_ui::{PickAnswers, PickKind, PickRequest, PickedFile};
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jlong;
use jni::{EnvUnowned, jni_sig, jni_str};

use crate::android::with_activity;

/// What the native callback needs: the activity, where answers go, and what each pick in
/// progress is for.
struct Picker {
    app: AndroidApp,
    answers: PickAnswers,
    kinds: HashMap<u64, PickKind>,
}

/// Set once the interface exists. A new start of the app replaces it.
static PICKER: Mutex<Option<Picker>> = Mutex::new(None);

fn with_picker<T>(f: impl FnOnce(&mut Picker) -> T) -> Option<T> {
    // A panic while the lock was held leaves nothing half done, so a poisoned lock is used as it
    // is.
    let mut picker = PICKER.lock().unwrap_or_else(|e| e.into_inner());
    picker.as_mut().map(f)
}

/// Makes the picker ready: answers to picks go to `answers`.
pub fn install(app: AndroidApp, answers: PickAnswers) {
    let mut picker = PICKER.lock().unwrap_or_else(|e| e.into_inner());
    *picker = Some(Picker {
        app,
        answers,
        kinds: HashMap::new(),
    });
}

/// Shows the system picker for `request`. The callback given to `DrumApp::with_file_picker`.
pub fn pick(request: PickRequest) {
    let Some((app, answers)) = with_picker(|p| {
        p.kinds.insert(request.id, request.kind);
        (p.app.clone(), p.answers.clone())
    }) else {
        log::warn!("a file pick was asked for before the picker was ready");
        return;
    };
    log::info!(
        "file pick {}: {:?}, {}",
        request.id,
        request.kind,
        request.mime
    );
    let result = with_activity(&app, |env, activity| {
        let mime = env.new_string(request.mime)?;
        let name = env.new_string(&request.name)?;
        env.call_method(
            activity,
            jni_str!("pickFile"),
            jni_sig!("(JZLjava/lang/String;Ljava/lang/String;)V"),
            &[
                JValue::Long(request.id as jlong),
                JValue::Bool(request.kind.creates()),
                JValue::Object(&mime),
                JValue::Object(&name),
            ],
        )?;
        Ok(())
    });
    if let Err(e) = result {
        log::warn!("could not show the file picker: {e}");
        with_picker(|p| p.kinds.remove(&request.id));
        answers.answer(request.id, None);
    }
}

/// `MainActivity.onFilePicked(long id, String uri)`, which Java calls on the UI thread when the
/// picker closes. `uri` is null when the person backed out.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_rrgmc_ticktapdrum_MainActivity_onFilePicked<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jlong,
    uri: JString<'local>,
) {
    // with_env catches a panic, so none crosses into Java.
    env.with_env(|env| -> jni::errors::Result<()> {
        let id = id as u64;
        let uri = if uri.is_null() {
            None
        } else {
            match uri.try_to_string(env) {
                Ok(uri) => Some(uri),
                Err(e) => {
                    log::warn!("could not read the picked file's address: {e}");
                    None
                }
            }
        };
        picked(id, uri);
        Ok(())
    })
    .resolve::<jni::errors::LogErrorAndDefault>()
}

/// Answers pick `id`: nothing when `uri` is `None`, otherwise the opened document.
fn picked(id: u64, uri: Option<String>) {
    let Some((app, answers, kind)) =
        with_picker(|p| (p.app.clone(), p.answers.clone(), p.kinds.remove(&id)))
    else {
        log::warn!("file pick {id} ended before the picker was ready");
        return;
    };
    let (Some(uri), Some(kind)) = (uri, kind) else {
        log::info!("file pick {id}: nothing chosen");
        answers.answer(id, None);
        return;
    };
    let fallback = answers.clone();
    let spawned = std::thread::Builder::new()
        .name("file pick".into())
        .spawn(move || {
            let file = match open(&app, kind, &uri) {
                Ok(file) => {
                    log::info!("file pick {id}: opened {}", file.name);
                    Some(file)
                }
                Err(e) => {
                    log::warn!("file pick {id}: could not open {uri}: {e}");
                    None
                }
            };
            answers.answer(id, file);
        });
    if let Err(e) = spawned {
        log::warn!("file pick {id}: could not start a thread to open the file: {e}");
        fallback.answer(id, None);
    }
}

/// Opens the document at `uri` through the `ContentResolver`: to read for
/// [`PickKind::OpenProject`], otherwise to write it from empty.
fn open(app: &AndroidApp, kind: PickKind, uri: &str) -> jni::errors::Result<PickedFile> {
    // "rwt" rather than "wt": some document providers ignore the truncate in "wt" and leave the
    // end of an older, longer file behind.
    let mode = if kind.creates() { "rwt" } else { "r" };
    let file = with_activity(app, |env, activity| {
        let resolver = content_resolver(env, activity)?;
        let uri = parse_uri(env, uri)?;
        let mode = env.new_string(mode)?;
        let descriptor = env
            .call_method(
                &resolver,
                jni_str!("openFileDescriptor"),
                jni_sig!("(Landroid/net/Uri;Ljava/lang/String;)Landroid/os/ParcelFileDescriptor;"),
                &[JValue::Object(&uri), JValue::Object(&mode)],
            )?
            .l()?;
        if descriptor.is_null() {
            return Err(jni::errors::Error::NullPtr("openFileDescriptor"));
        }
        let fd = env
            .call_method(&descriptor, jni_str!("detachFd"), jni_sig!("()I"), &[])?
            .i()?;
        // SAFETY: detachFd hands the descriptor over; nothing on the Java side closes it now.
        Ok(unsafe { File::from_raw_fd(fd) })
    })?;
    let name = match display_name(app, uri) {
        Ok(Some(name)) if !name.is_empty() => name,
        Ok(_) => last_segment(uri),
        Err(e) => {
            log::warn!("could not read the display name of {uri}: {e}");
            last_segment(uri)
        }
    };
    Ok(PickedFile { file, name })
}

/// The document's name as the picker shows it: its `OpenableColumns.DISPLAY_NAME`.
fn display_name(app: &AndroidApp, uri: &str) -> jni::errors::Result<Option<String>> {
    with_activity(app, |env, activity| {
        let resolver = content_resolver(env, activity)?;
        let uri = parse_uri(env, uri)?;
        // The value of OpenableColumns.DISPLAY_NAME.
        let column = env.new_string("_display_name")?;
        let columns = env.new_object_array(1, jni_str!("java/lang/String"), &column)?;
        let none = JObject::null();
        let cursor = env
            .call_method(
                &resolver,
                jni_str!("query"),
                jni_sig!(
                    "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;"
                ),
                &[
                    JValue::Object(&uri),
                    JValue::Object(&columns),
                    JValue::Object(&none),
                    JValue::Object(&none),
                    JValue::Object(&none),
                ],
            )?
            .l()?;
        if cursor.is_null() {
            return Ok(None);
        }
        let first = env
            .call_method(&cursor, jni_str!("moveToFirst"), jni_sig!("()Z"), &[])?
            .z()?;
        let name = if first {
            let name = env
                .call_method(
                    &cursor,
                    jni_str!("getString"),
                    jni_sig!("(I)Ljava/lang/String;"),
                    &[JValue::Int(0)],
                )?
                .l()?;
            let name = env.cast_local::<JString>(name)?;
            if name.is_null() {
                None
            } else {
                Some(name.try_to_string(env)?)
            }
        } else {
            None
        };
        env.call_method(&cursor, jni_str!("close"), jni_sig!("()V"), &[])?;
        Ok(name)
    })
}

/// `activity.getContentResolver()`.
fn content_resolver<'local>(
    env: &mut jni::Env<'local>,
    activity: &JObject<'_>,
) -> jni::errors::Result<JObject<'local>> {
    env.call_method(
        activity,
        jni_str!("getContentResolver"),
        jni_sig!("()Landroid/content/ContentResolver;"),
        &[],
    )?
    .l()
}

/// `Uri.parse(uri)`.
fn parse_uri<'local>(
    env: &mut jni::Env<'local>,
    uri: &str,
) -> jni::errors::Result<JObject<'local>> {
    let uri = env.new_string(uri)?;
    env.call_static_method(
        jni_str!("android/net/Uri"),
        jni_str!("parse"),
        jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
        &[JValue::Object(&uri)],
    )?
    .l()
}

/// A name for the document when the provider gives none: the end of its address, decoded. A
/// Downloads address such as `.../document/primary%3ADownload%2FBeat.drums` gives `Beat.drums`.
fn last_segment(uri: &str) -> String {
    let bytes = uri.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                i += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                i += 1;
            }
        }
    }
    let decoded = String::from_utf8_lossy(&decoded);
    decoded
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default()
        .to_string()
}
