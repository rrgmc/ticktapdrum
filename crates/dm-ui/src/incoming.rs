//! Files a host hands to the app from outside, such as a groove pack shared to the app on
//! Android. Each one goes to the groove import, as a file chosen in the menu does.
//!
//! The host sends a file's name and bytes through [`IncomingFiles`], from any thread. The
//! interface takes them on its next frame. See "Groove import" in `docs/DESIGN.md`.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

/// Where a host hands in files to import. Cheap to clone, and usable from any thread, such as
/// the one a JNI callback arrives on. Get it from [`crate::DrumApp::incoming_files`].
#[derive(Clone)]
pub struct IncomingFiles {
    sender: Sender<(String, Vec<u8>)>,
    /// The interface's context once it has drawn a frame, so a file wakes it at once.
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl IncomingFiles {
    /// Hands in a file to import as grooves. `name` is the file's name as people know it, such
    /// as `Chilled Beat.zip`; it names the pack and appears in messages. `bytes` is the whole
    /// file.
    pub fn send(&self, name: String, bytes: Vec<u8>) {
        let _ = self.sender.send((name, bytes));
        if let Ok(ctx) = self.ctx.lock()
            && let Some(ctx) = ctx.as_ref()
        {
            ctx.request_repaint();
        }
    }
}

/// The interface's end of the files.
pub(crate) struct IncomingInbox {
    receiver: Receiver<(String, Vec<u8>)>,
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

    /// The next file that has arrived, if any, as its name and bytes.
    pub(crate) fn take(&self) -> Option<(String, Vec<u8>)> {
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
        })
        .join()
        .unwrap();
        assert_eq!(inbox.take(), Some(("One.mid".into(), vec![1])));
        assert_eq!(inbox.take(), Some(("Two.zip".into(), vec![2, 2])));
        assert_eq!(inbox.take(), None);
    }
}
