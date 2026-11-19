//! Getting a bank: the offer to download it, the download, loading it, and starting the audio.
//!
//! Downloading and parsing run on worker threads. The audio engine is started on the interface
//! thread, because a cpal stream cannot be moved between threads on every platform.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};

use dm_audio::{AudioError, Engine, SoundFont};
use dm_bank::{BankError, BankInfo, COLOMBO, Progress};

const BANK: BankInfo = COLOMBO;

enum Phase {
    /// Nothing installed: show the offer.
    Offer,
    /// Downloading or installing on a worker thread.
    Installing {
        progress: Arc<Mutex<Option<Progress>>>,
        cancel: Arc<AtomicBool>,
        done: Receiver<Result<PathBuf, BankError>>,
    },
    /// Parsing the `.sf2` on a worker thread.
    Loading {
        done: Receiver<Result<Arc<SoundFont>, AudioError>>,
    },
    /// Audio running.
    Ready(Box<Engine>),
    /// Something failed; the message is shown with a way back.
    Failed(Failure),
}

/// What went wrong, kept as it came so its message is written in the language shown.
enum Failure {
    Bank(BankError),
    Audio(AudioError),
    /// The installer's thread ended without an answer.
    InstallerStopped,
    /// The loading thread ended without an answer.
    LoadingStopped,
}

impl Failure {
    /// The message shown for this failure. The detail an error carries, such as the system's
    /// own words for a network or file error, is shown as it came, inside a translated sentence.
    fn message(&self) -> String {
        match self {
            Failure::Bank(BankError::Download(detail)) => {
                tr!("bank-error-download", detail = detail.as_str())
            }
            Failure::Bank(BankError::Mismatch { what }) => match *what {
                "the bank file" => tr!("bank-error-mismatch-bank-file"),
                _ => tr!("bank-error-mismatch-archive"),
            },
            Failure::Bank(BankError::Archive(detail)) => {
                tr!("bank-error-archive", detail = detail.as_str())
            }
            Failure::Bank(BankError::Io(error)) => {
                tr!("bank-error-file", detail = error.to_string())
            }
            Failure::Bank(BankError::Cancelled) => tr!("bank-error-cancelled"),
            Failure::Audio(AudioError::SoundFont(detail)) => {
                tr!("bank-error-soundfont", detail = detail.as_str())
            }
            Failure::Audio(AudioError::Device(detail)) => {
                tr!("bank-error-device", detail = detail.as_str())
            }
            Failure::Audio(AudioError::Nothing) => tr!("bank-error-nothing"),
            Failure::Audio(AudioError::Cancelled) => tr!("bank-error-render-cancelled"),
            Failure::InstallerStopped => tr!("bank-error-installer-stopped"),
            Failure::LoadingStopped => tr!("bank-error-loading-stopped"),
        }
    }
}

pub(crate) struct BankState {
    data_dir: PathBuf,
    phase: Phase,
    /// A phase change requested before the first frame, when no `egui::Context` exists yet.
    pending: Option<Start>,
    /// The loaded bank, kept for rendering a WAV file away from the live engine.
    soundfont: Option<Arc<SoundFont>>,
}

enum Start {
    File(PathBuf),
    Archive(Option<PathBuf>),
}

impl BankState {
    pub(crate) fn new(data_dir: PathBuf) -> Self {
        let pending = dm_bank::installed(&data_dir, &BANK).map(Start::File);
        Self {
            data_dir,
            phase: Phase::Offer,
            pending,
            soundfont: None,
        }
    }

    pub(crate) fn use_file(&mut self, path: PathBuf) {
        self.pending = Some(Start::File(path));
    }

    pub(crate) fn use_local_archive(&mut self, path: PathBuf) {
        if !matches!(self.pending, Some(Start::File(_))) {
            self.pending = Some(Start::Archive(Some(path)));
        }
    }

    /// The loaded bank, once the engine has started.
    pub(crate) fn soundfont(&self) -> Option<Arc<SoundFont>> {
        self.soundfont.clone()
    }

    pub(crate) fn engine_mut(&mut self) -> Option<&mut Engine> {
        match &mut self.phase {
            Phase::Ready(engine) => Some(engine),
            _ => None,
        }
    }

    /// Takes the running engine out for one frame. Call only after [`Self::engine_mut`] returned
    /// `Some`, and give it back with [`Self::put_engine`] in the same frame.
    pub(crate) fn take_engine(&mut self) -> Engine {
        match std::mem::replace(&mut self.phase, Phase::Offer) {
            Phase::Ready(engine) => *engine,
            _ => unreachable!("take_engine without a running engine"),
        }
    }

    pub(crate) fn put_engine(&mut self, engine: Engine) {
        self.phase = Phase::Ready(Box::new(engine));
    }

    /// Starts pending work and collects finished work. Called once per frame.
    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
        match self.pending.take() {
            Some(Start::File(path)) => self.start_loading(ctx, path),
            Some(Start::Archive(archive)) => self.start_install(ctx, archive),
            None => {}
        }

        let next = match &self.phase {
            Phase::Installing { done, .. } => match done.try_recv() {
                Ok(Ok(path)) => Some(Start::File(path)),
                Ok(Err(error)) => {
                    self.phase = Phase::Failed(Failure::Bank(error));
                    None
                }
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    self.phase = Phase::Failed(Failure::InstallerStopped);
                    None
                }
            },
            Phase::Loading { done } => {
                match done.try_recv() {
                    Ok(Ok(soundfont)) => {
                        self.soundfont = Some(soundfont.clone());
                        self.phase = match Engine::start(soundfont) {
                            Ok(engine) => Phase::Ready(Box::new(engine)),
                            Err(error) => Phase::Failed(Failure::Audio(error)),
                        };
                    }
                    Ok(Err(error)) => self.phase = Phase::Failed(Failure::Audio(error)),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => {
                        self.phase = Phase::Failed(Failure::LoadingStopped);
                    }
                }
                None
            }
            _ => None,
        };
        if let Some(Start::File(path)) = next {
            self.start_loading(ctx, path);
        }
    }

    fn start_install(&mut self, ctx: &egui::Context, archive: Option<PathBuf>) {
        let progress = Arc::new(Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));
        let (send, done) = channel();
        let dir = self.data_dir.clone();
        let (progress_w, cancel_w, ctx_w) = (progress.clone(), cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let report = |p: Progress| {
                if let Ok(mut slot) = progress_w.lock() {
                    *slot = Some(p);
                }
                ctx_w.request_repaint();
            };
            let result = match archive {
                Some(archive) => {
                    dm_bank::install_from_archive(&dir, &BANK, &archive, &report, &cancel_w)
                }
                None => dm_bank::install(&dir, &BANK, &report, &cancel_w),
            };
            let _ = send.send(result);
            ctx_w.request_repaint();
        });
        self.phase = Phase::Installing {
            progress,
            cancel,
            done,
        };
    }

    fn start_loading(&mut self, ctx: &egui::Context, path: PathBuf) {
        let (send, done) = channel();
        let ctx_w = ctx.clone();
        std::thread::spawn(move || {
            let result = dm_audio::load_soundfont(&path);
            let _ = send.send(result);
            ctx_w.request_repaint();
        });
        self.phase = Phase::Loading { done };
    }

    /// Draws everything before the grid: the offer, progress, loading and errors.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let mut start = false;
        // A column of at most 520 points, centred, with text wrapping inside it. A centred layout
        // alone does not wrap long labels on a narrow screen: they ran off both edges in portrait.
        let area = ui.available_rect_before_wrap();
        let width = area.width().min(520.0);
        let top = area.min.y + area.height() * 0.12;
        let column = egui::Rect::from_min_size(
            egui::pos2(area.center().x - width / 2.0, top),
            egui::vec2(width, area.max.y - top),
        );
        let builder = egui::UiBuilder::new()
            .max_rect(column)
            .layout(egui::Layout::top_down(egui::Align::Center));
        ui.scope_builder(builder, |ui| {
            ui.set_max_width(width);
            match &self.phase {
                Phase::Offer => {
                    ui.heading(tr!("bank-offer-title"));
                    ui.add_space(12.0);
                    wrapped(ui, tr!("bank-offer", bank = BANK.name));
                    ui.add_space(8.0);
                    wrapped(
                        ui,
                        tr!(
                            "bank-offer-sizes",
                            download = mib(BANK.archive_bytes),
                            space = mib(BANK.member_bytes)
                        ),
                    );
                    wrapped(ui, license());
                    ui.add_space(16.0);
                    if big_button(ui, &tr!("bank-download")).clicked() {
                        start = true;
                    }
                }
                Phase::Installing {
                    progress, cancel, ..
                } => {
                    let current = progress.lock().ok().and_then(|p| *p);
                    let (text, fraction) = describe(current);
                    ui.heading(tr!("bank-installing-title"));
                    ui.add_space(12.0);
                    wrapped(ui, text);
                    ui.add(egui::ProgressBar::new(fraction).show_percentage());
                    ui.add_space(16.0);
                    if big_button(ui, &tr!("bank-cancel")).clicked() {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
                Phase::Loading { .. } => {
                    ui.heading(tr!("bank-loading-title"));
                    ui.add_space(12.0);
                    ui.spinner();
                }
                Phase::Failed(failure) => {
                    ui.heading(tr!("bank-failed-title"));
                    ui.add_space(12.0);
                    wrapped(ui, failure.message());
                    ui.add_space(16.0);
                    if big_button(ui, &tr!("bank-try-again")).clicked() {
                        start = true;
                    }
                }
                Phase::Ready(_) => {}
            }
        });
        if start {
            self.start_install(&ctx, None);
        }
    }
}

fn describe(progress: Option<Progress>) -> (String, f32) {
    match progress {
        None => (tr!("bank-connecting"), 0.0),
        Some(Progress::Downloading { done, total }) => (
            tr!("bank-downloading", done = mib(done), total = mib(total)),
            ratio(done, total),
        ),
        Some(Progress::VerifyingArchive) => (tr!("bank-checking-download"), 1.0),
        Some(Progress::Extracting { done, total }) => (
            tr!("bank-unpacking", done = mib(done), total = mib(total)),
            ratio(done, total),
        ),
        Some(Progress::VerifyingMember) => (tr!("bank-checking-sounds"), 1.0),
    }
}

fn ratio(done: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (done as f64 / total as f64) as f32
    }
}

/// Bytes in whole mebibytes, for the messages to write with their unit.
fn mib(bytes: u64) -> u64 {
    (bytes as f64 / (1024.0 * 1024.0)).round() as u64
}

/// The bank's licence line. The bank's own line is English; a bank this app knows has a
/// translated one.
fn license() -> String {
    match BANK.id {
        "colombogmgs2-17.11" => tr!("bank-license-colombo"),
        _ => tr!("bank-license", license = BANK.license),
    }
}

/// A label that always wraps at the column's width.
fn wrapped(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) -> egui::Response {
    ui.add(egui::Label::new(text).wrap())
}

pub(crate) fn big_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).size(20.0)).min_size(egui::vec2(160.0, 48.0)),
    )
}
