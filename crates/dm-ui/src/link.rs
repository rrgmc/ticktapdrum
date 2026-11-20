//! Adding grooves from a link: the download on a worker thread, what to do with text shared to
//! the app, and the words for both. The dialog is in `menu.rs`; the download itself is
//! `dm-groove`'s work. See "Milestone 13: add from a link" in `docs/DESIGN.md`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};

use dm_groove::LinkError;

/// A downloaded file's name and its bytes, or why the download failed.
pub(crate) type Downloaded = Result<(String, Vec<u8>), LinkError>;

/// The bytes downloaded so far, and the whole size if the server said.
pub(crate) type Progress = (u64, Option<u64>);

/// A download in progress, on a worker thread. The interface polls it each frame, as it does a
/// WAV export.
pub(crate) struct LinkJob {
    progress: Arc<Mutex<Progress>>,
    cancel: Arc<AtomicBool>,
    done: Receiver<Downloaded>,
}

impl LinkJob {
    /// Starts downloading `link` on a worker thread.
    pub(crate) fn start(link: String, ctx: egui::Context) -> Self {
        Self::start_with(link, ctx, |link, progress| {
            dm_groove::download(link, progress)
        })
    }

    /// Starts a job that runs `download` on a worker thread. Tests pass a stand-in for the
    /// network.
    fn start_with(
        link: String,
        ctx: egui::Context,
        download: impl FnOnce(&str, &mut dyn FnMut(u64, Option<u64>) -> bool) -> Downloaded
        + Send
        + 'static,
    ) -> Self {
        let progress = Arc::new(Mutex::new((0, None)));
        let cancel = Arc::new(AtomicBool::new(false));
        let (send, done) = channel();
        let (progress_w, cancel_w) = (progress.clone(), cancel.clone());
        std::thread::spawn(move || {
            let mut report = |so_far: u64, total: Option<u64>| {
                if let Ok(mut p) = progress_w.lock() {
                    *p = (so_far, total);
                }
                ctx.request_repaint();
                !cancel_w.load(Ordering::Relaxed)
            };
            let result = download(&link, &mut report);
            let _ = send.send(result);
            ctx.request_repaint();
        });
        Self {
            progress,
            cancel,
            done,
        }
    }

    /// How far the download has got.
    pub(crate) fn progress(&self) -> Progress {
        self.progress.lock().map(|p| *p).unwrap_or((0, None))
    }

    /// Asks the worker to stop. It stops at its next progress report; the job can be dropped at
    /// once, since nothing waits for its result.
    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The result, once the worker has finished. `None` while it runs.
    pub(crate) fn poll(&self) -> Option<Downloaded> {
        match self.done.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(LinkError::Failed("the download thread stopped".into())))
            }
        }
    }
}

/// Why a download failed, in words for the link dialog. A failure's detail goes to the log only.
pub(crate) fn error_text(error: &LinkError) -> String {
    match error {
        LinkError::NotALink => tr!("grooves-link-not-a-link"),
        LinkError::Failed(detail) => {
            log::warn!("download failed: {detail}");
            tr!("grooves-link-failed")
        }
        LinkError::Status(code) => tr!("grooves-link-status", code = *code),
        LinkError::TooLarge => tr!("grooves-link-too-large"),
        LinkError::WebPage => tr!("grooves-link-web-page"),
    }
}

/// The progress line: "Downloading... 120 KB", or in megabytes from 1 MB on.
pub(crate) fn progress_text(bytes: u64) -> String {
    match size(bytes) {
        Size::Kilobytes(kb) => tr!("grooves-link-downloading-kb", kb = kb),
        Size::Megabytes(mb) => tr!("grooves-link-downloading-mb", mb = mb),
    }
}

/// A size as people read it.
#[derive(Debug, PartialEq)]
enum Size {
    /// Whole kilobytes, rounded up, so a started download never shows 0 KB for long.
    Kilobytes(u64),
    /// Megabytes with one decimal.
    Megabytes(f64),
}

fn size(bytes: u64) -> Size {
    const MB: u64 = 1024 * 1024;
    if bytes < MB {
        Size::Kilobytes(bytes.div_ceil(1024))
    } else {
        let tenths = (bytes * 10 + MB / 2) / MB;
        Size::Megabytes(tenths as f64 / 10.0)
    }
}

/// What to do with text shared to the app.
#[derive(Debug, PartialEq)]
pub(crate) enum SharedText {
    /// Download this link, cleaned up by [`dm_groove::link_address`].
    Download(String),
    /// Open the Paste a drum tab dialog with the text.
    PasteTab,
    /// Say that only links can be shared.
    Refuse,
}

/// Chooses what to do with shared text. A link is downloaded, even in basic mode, whose import
/// keeps the grooves without opening the list. Other text is a drum tab, outside basic mode, as
/// in the menu.
pub(crate) fn shared_text(text: &str, basic: bool) -> SharedText {
    if let Some(link) = dm_groove::link_address(text) {
        SharedText::Download(link)
    } else if !basic && !text.trim().is_empty() {
        SharedText::PasteTab
    } else {
        SharedText::Refuse
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Waits for the job's result, for at most a few seconds.
    fn wait(job: &LinkJob) -> Downloaded {
        let start = Instant::now();
        loop {
            if let Some(result) = job.poll() {
                return result;
            }
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the job never ended"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn a_link_downloads_whatever_the_settings() {
        let link = "https://example.com/Beat.mid";
        for basic in [false, true] {
            assert_eq!(
                shared_text(&format!("  {link}\n"), basic),
                SharedText::Download(link.into())
            );
        }
    }

    #[test]
    fn other_text_is_a_tab_outside_basic_mode() {
        let tab = "HH|x-x-x-x-|\nSD|----o---|";
        assert_eq!(shared_text(tab, false), SharedText::PasteTab);
        assert_eq!(shared_text(tab, true), SharedText::Refuse);
        assert_eq!(shared_text("  ", false), SharedText::Refuse);
        assert_eq!(
            shared_text("ftp://example.com/a.mid", true),
            SharedText::Refuse
        );
    }

    #[test]
    fn a_finished_job_hands_back_the_file_and_its_progress() {
        let job = LinkJob::start_with("x".into(), egui::Context::default(), |link, progress| {
            assert!(progress(3, Some(3)));
            Ok((format!("{link}.mid"), vec![1, 2, 3]))
        });
        assert_eq!(wait(&job), Ok(("x.mid".into(), vec![1, 2, 3])));
        assert_eq!(job.progress(), (3, Some(3)));
    }

    #[test]
    fn cancelling_makes_the_progress_report_say_stop() {
        let (started, wait_started) = channel();
        let job = LinkJob::start_with("x".into(), egui::Context::default(), move |_, progress| {
            assert!(progress(0, None));
            started.send(()).unwrap();
            // A download checks back now and then; this one until it is told to stop.
            let mut so_far = 0;
            loop {
                so_far += 1;
                if !progress(so_far, None) {
                    return Err(LinkError::Failed("cancelled".into()));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        wait_started.recv().unwrap();
        assert_eq!(job.poll(), None);
        job.cancel();
        assert_eq!(wait(&job), Err(LinkError::Failed("cancelled".into())));
    }

    #[test]
    fn sizes_show_in_kilobytes_then_megabytes() {
        assert_eq!(size(0), Size::Kilobytes(0));
        assert_eq!(size(1), Size::Kilobytes(1));
        assert_eq!(size(120 * 1024 - 5), Size::Kilobytes(120));
        assert_eq!(size(5 * 1024 * 1024 / 2), Size::Megabytes(2.5));
        assert_eq!(size(3 * 1024 * 1024), Size::Megabytes(3.0));
    }
}
