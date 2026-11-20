//! Files and text a host hands to the app from outside, such as a groove pack or a link shared to
//! the app on Android. A file goes to the groove import, as a file chosen in the menu does. Text
//! is a link to download, or, with the experimental setting on, a drum tab to paste.
//!
//! The host sends them through [`IncomingFiles`], from any thread. The interface takes them on
//! its next frame. See "Groove import" in `docs/DESIGN.md`.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

/// Where a host hands in files to import. Cheap to clone, and usable from any thread, such as
/// the one a JNI callback arrives on. Get it from [`crate::DrumApp::incoming_files`].
#[derive(Clone)]
pub struct IncomingFiles {
    sender: Sender<Incoming>,
    /// The interface's context once it has drawn a frame, so a file wakes it at once.
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl IncomingFiles {
    /// Hands in a file to import as grooves. `name` is the file's name as people know it, such
    /// as `Chilled Beat.zip`; it names the pack and appears in messages. `bytes` is the whole
    /// file.
    pub fn send(&self, name: String, bytes: Vec<u8>) {
        self.deliver(Incoming::File { name, bytes });
    }

    /// Hands in text shared to the app, such as a link to a groove file.
    pub fn send_text(&self, text: String) {
        self.deliver(Incoming::Text(text));
    }

    fn deliver(&self, incoming: Incoming) {
        let _ = self.sender.send(incoming);
        if let Ok(ctx) = self.ctx.lock()
            && let Some(ctx) = ctx.as_ref()
        {
            ctx.request_repaint();
        }
    }
}

/// Something a host handed in.
#[derive(Debug, PartialEq)]
pub(crate) enum Incoming {
    /// A file's name and its bytes.
    File { name: String, bytes: Vec<u8> },
    /// Shared text.
    Text(String),
}

/// The interface's end of the files.
pub(crate) struct IncomingInbox {
    receiver: Receiver<Incoming>,
    files: IncomingFiles,
}

impl IncomingInbox {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = channel();
        Self {
            receiver,
            files: IncomingFiles {
                sender,
                ctx: Arc::new(Mutex::new(None)),
            },
        }
    }

    /// A handle for the host.
    pub(crate) fn files(&self) -> IncomingFiles {
        self.files.clone()
    }

    /// Remembers the context, so files wake the interface. Call it each frame.
    pub(crate) fn set_context(&self, ctx: &egui::Context) {
        if let Ok(mut slot) = self.files.ctx.lock()
            && slot.is_none()
        {
            *slot = Some(ctx.clone());
        }
    }

    /// The next file or text that has arrived, if any.
    pub(crate) fn take(&self) -> Option<Incoming> {
        self.receiver.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_arrive_in_order_from_any_thread() {
        let inbox = IncomingInbox::new();
        let files = inbox.files();
        std::thread::spawn(move || {
            files.send("One.mid".into(), vec![1]);
            files.send("Two.zip".into(), vec![2, 2]);
            files.send_text("https://example.com/a.mid".into());
        })
        .join()
        .unwrap();
        let file = |name: &str, bytes: Vec<u8>| Incoming::File {
            name: name.into(),
            bytes,
        };
        assert_eq!(inbox.take(), Some(file("One.mid", vec![1])));
        assert_eq!(inbox.take(), Some(file("Two.zip", vec![2, 2])));
        assert_eq!(
            inbox.take(),
            Some(Incoming::Text("https://example.com/a.mid".into()))
        );
        assert_eq!(inbox.take(), None);
    }
}
