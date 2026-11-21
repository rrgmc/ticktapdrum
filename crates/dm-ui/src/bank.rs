//! Getting a bank: the offer to download it, the download, a SoundFont the person adds, loading
//! it, and starting the audio. See "Milestone 15: other SoundFonts" in `docs/DESIGN.md`.
//!
//! Downloading, copying and parsing run on worker threads. The audio engine is started on the
//! interface thread, because a cpal stream cannot be moved between threads on every platform.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};

use dm_audio::{AudioError, Engine, SoundFont};
use dm_bank::{BankError, BankInfo, COLOMBO, Progress, UserSoundfont};
use dm_project::SoundfontRef;

pub(crate) const BANK: BankInfo = COLOMBO;

/// Which SoundFont the app plays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    /// The bank the app downloads, ColomboGMGS2.
    Colombo,
    /// A SoundFont the person added, kept in the app's `soundfonts` folder.
    Added(UserSoundfont),
    /// Development shortcut: a file used where it is (`DM_SF2`).
    Path(PathBuf),
}

impl Source {
    /// How a project file records it.
    pub(crate) fn reference(&self) -> SoundfontRef {
        match self {
            Source::Colombo => SoundfontRef::Bank(BANK.id.to_owned()),
            Source::Added(added) => SoundfontRef::File {
                name: added.name.clone(),
                sha256: added.sha256.clone(),
            },
            // Not digested: the shortcut skips the work, and records only the name.
            Source::Path(path) => SoundfontRef::File {
                name: file_name(path),
                sha256: String::new(),
            },
        }
    }
}

/// How messages name a SoundFont a project recorded: the bank's own name, or the file's name
/// without `.sf2`.
pub(crate) fn display_name(reference: &SoundfontRef) -> String {
    match reference {
        SoundfontRef::Bank(id) if id == BANK.id => BANK.name.to_owned(),
        SoundfontRef::Bank(id) => id.clone(),
        SoundfontRef::File { name, .. } => stem(name),
    }
}

/// Whether two records name the same SoundFont: the same bank, or files with the same digest.
/// A record without a digest (the development shortcut) matches by name.
pub(crate) fn same_soundfont(a: &SoundfontRef, b: &SoundfontRef) -> bool {
    match (a, b) {
        (
            SoundfontRef::File {
                name: name_a,
                sha256: digest_a,
            },
            SoundfontRef::File {
                name: name_b,
                sha256: digest_b,
            },
        ) => {
            if digest_a.is_empty() || digest_b.is_empty() {
                name_a == name_b
            } else {
                digest_a == digest_b
            }
        }
        _ => a == b,
    }
}

/// A file name without its `.sf2` extension.
pub(crate) fn stem(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .map_or_else(|| name.to_owned(), |s| s.to_string_lossy().into_owned())
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

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
        source: Source,
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
            Failure::Bank(error) => bank_error(error),
            Failure::Audio(error) => audio_error(error),
            Failure::InstallerStopped => tr!("bank-error-installer-stopped"),
            Failure::LoadingStopped => tr!("bank-error-loading-stopped"),
        }
    }
}

fn bank_error(error: &BankError) -> String {
    match error {
        BankError::Download(detail) => tr!("bank-error-download", detail = detail.as_str()),
        BankError::Mismatch { what } => match *what {
            "the bank file" => tr!("bank-error-mismatch-bank-file"),
            _ => tr!("bank-error-mismatch-archive"),
        },
        BankError::Archive(detail) => tr!("bank-error-archive", detail = detail.as_str()),
        BankError::Io(error) => tr!("bank-error-file", detail = error.to_string()),
        BankError::Cancelled => tr!("bank-error-cancelled"),
    }
}

fn audio_error(error: &AudioError) -> String {
    match error {
        AudioError::SoundFont(detail) => tr!("bank-error-soundfont", detail = detail.as_str()),
        AudioError::Device(detail) => tr!("bank-error-device", detail = detail.as_str()),
        AudioError::Nothing => tr!("bank-error-nothing"),
        AudioError::Cancelled => tr!("bank-error-render-cancelled"),
    }
}

/// Why adding a SoundFont failed.
enum AddError {
    Bank(BankError),
    Audio(AudioError),
    /// The file holds no drum kits; it was deleted again. The name is the file's.
    NoKits(String),
}

impl AddError {
    fn message(&self, name: &str) -> String {
        match self {
            AddError::Bank(BankError::Cancelled) => tr!("bank-add-cancelled"),
            AddError::Bank(error) => {
                tr!("bank-add-failed", name = name, reason = bank_error(error))
            }
            AddError::Audio(error) => {
                tr!("bank-add-failed", name = name, reason = audio_error(error))
            }
            AddError::NoKits(name) => tr!("bank-add-no-kits", name = stem(name)),
        }
    }
}

/// A SoundFont being copied into the app's folder and checked, on a worker thread. The engine
/// keeps playing meanwhile.
struct AddJob {
    /// The name the file came with, for messages.
    name: String,
    copied: Arc<Mutex<u64>>,
    /// The file's size, where the host could tell.
    total: Option<u64>,
    cancel: Arc<AtomicBool>,
    done: Receiver<Result<(UserSoundfont, Arc<SoundFont>), AddError>>,
}

/// How far an added SoundFont has got: its name and the share copied, if known.
pub(crate) struct Adding<'a> {
    pub(crate) name: &'a str,
    pub(crate) fraction: Option<f32>,
}

pub(crate) struct BankState {
    data_dir: PathBuf,
    phase: Phase,
    /// A phase change requested before the first frame, when no `egui::Context` exists yet.
    pending: Option<Start>,
    /// The loaded bank, kept for rendering a WAV file away from the live engine.
    soundfont: Option<Arc<SoundFont>>,
    /// The SoundFont playing, or the one to fall back to while none is.
    source: Source,
    adding: Option<AddJob>,
    /// Messages for the app to show, such as why a SoundFont did not load.
    notes: Vec<String>,
    /// Counts the engines started, so the app knows when to give a new one the project.
    generation: u64,
}

enum Start {
    Load(Source),
    Archive(Option<PathBuf>),
}

impl BankState {
    /// `chosen` is the added SoundFont the person chose last time, by name. If it is gone, the
    /// app falls back to ColomboGMGS2 and says so.
    pub(crate) fn new(data_dir: PathBuf, chosen: Option<String>) -> Self {
        let mut notes = Vec::new();
        let added = chosen.as_deref().and_then(|name| {
            let found = dm_bank::find_soundfont(&data_dir, name);
            if found.is_none() {
                notes.push(tr!("bank-chosen-missing", name = stem(name)));
            }
            found
        });
        let pending = match added {
            Some(added) => Some(Start::Load(Source::Added(added))),
            None => dm_bank::installed(&data_dir, &BANK).map(|_| Start::Load(Source::Colombo)),
        };
        Self {
            data_dir,
            phase: Phase::Offer,
            pending,
            soundfont: None,
            source: Source::Colombo,
            adding: None,
            notes,
            generation: 0,
        }
    }

    pub(crate) fn use_file(&mut self, path: PathBuf) {
        self.pending = Some(Start::Load(Source::Path(path)));
    }

    pub(crate) fn use_local_archive(&mut self, path: PathBuf) {
        if !matches!(self.pending, Some(Start::Load(_))) {
            self.pending = Some(Start::Archive(Some(path)));
        }
    }

    /// The loaded bank, once the engine has started.
    pub(crate) fn soundfont(&self) -> Option<Arc<SoundFont>> {
        self.soundfont.clone()
    }

    /// The SoundFont playing, or about to.
    pub(crate) fn source(&self) -> &Source {
        &self.source
    }

    /// Whether nothing is loading or waiting to load, so [`Self::source`] is what plays (or, on
    /// the download page, ColomboGMGS2).
    pub(crate) fn settled(&self) -> bool {
        self.pending.is_none()
            && matches!(
                self.phase,
                Phase::Ready(_) | Phase::Offer | Phase::Failed(_)
            )
    }

    /// Counts the engines started. A change means the engine is new and needs the project.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether ColomboGMGS2 is installed.
    pub(crate) fn colombo_installed(&self) -> bool {
        dm_bank::installed(&self.data_dir, &BANK).is_some()
    }

    /// The SoundFonts the person added.
    pub(crate) fn added(&self) -> Vec<UserSoundfont> {
        dm_bank::soundfonts(&self.data_dir)
    }

    /// The messages to show, oldest first. They are gone once taken.
    pub(crate) fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// How far adding a SoundFont has got, while it runs.
    pub(crate) fn adding(&self) -> Option<Adding<'_>> {
        self.adding.as_ref().map(|job| Adding {
            name: &job.name,
            fraction: job.total.filter(|&total| total > 0).map(|total| {
                let copied = job.copied.lock().map_or(0, |c| *c);
                ratio(copied, total)
            }),
        })
    }

    pub(crate) fn cancel_adding(&self) {
        if let Some(job) = &self.adding {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Plays `source` from the next frame, unless it is already playing. ColomboGMGS2 that is
    /// not installed shows the download page.
    pub(crate) fn switch(&mut self, source: Source) {
        if source == self.source && matches!(self.phase, Phase::Ready(_)) {
            return;
        }
        self.pending = Some(Start::Load(source));
    }

    /// Deletes an added SoundFont. Deleting the one playing switches to ColomboGMGS2.
    pub(crate) fn delete(&mut self, name: &str) {
        if let Err(error) = dm_bank::remove_soundfont(&self.data_dir, name) {
            self.notes.push(tr!(
                "bank-delete-failed",
                name = stem(name),
                reason = bank_error(&error)
            ));
            return;
        }
        if matches!(&self.source, Source::Added(added) if added.name == name) {
            self.switch(Source::Colombo);
        }
    }

    /// Starts copying a SoundFont into the app's folder. `total` is its size if known. Once it
    /// is copied and found to hold drum kits, it plays.
    pub(crate) fn add(
        &mut self,
        ctx: &egui::Context,
        name: String,
        total: Option<u64>,
        mut source: Box<dyn Read + Send>,
    ) {
        if self.adding.is_some() {
            self.notes.push(tr!("bank-add-busy"));
            return;
        }
        let copied = Arc::new(Mutex::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (send, done) = channel();
        let dir = self.data_dir.clone();
        let (copied_w, cancel_w, ctx_w, name_w) =
            (copied.clone(), cancel.clone(), ctx.clone(), name.clone());
        std::thread::spawn(move || {
            let report = |bytes: u64| {
                if let Ok(mut slot) = copied_w.lock() {
                    *slot = bytes;
                }
                ctx_w.request_repaint();
            };
            let result = add_and_check(&dir, &name_w, &mut *source, &report, &cancel_w);
            let _ = send.send(result);
            ctx_w.request_repaint();
        });
        self.adding = Some(AddJob {
            name,
            copied,
            total,
            cancel,
            done,
        });
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
            Some(Start::Load(source)) => self.start_loading(ctx, source),
            Some(Start::Archive(archive)) => self.start_install(ctx, archive),
            None => {}
        }
        self.poll_adding();

        let next = match &self.phase {
            Phase::Installing { done, .. } => match done.try_recv() {
                Ok(Ok(_)) => Some(Source::Colombo),
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
            Phase::Loading { source, done } => match done.try_recv() {
                Ok(Ok(soundfont)) => {
                    let source = source.clone();
                    self.start_engine(source, soundfont);
                    None
                }
                Ok(Err(error)) => self.load_failed(Failure::Audio(error)),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => self.load_failed(Failure::LoadingStopped),
            },
            _ => None,
        };
        if let Some(source) = next {
            self.start_loading(ctx, source);
        }
    }

    /// A SoundFont did not load. One the person added gives way to ColomboGMGS2, with a
    /// message; ColomboGMGS2 itself shows the failure. Returns what to load instead.
    fn load_failed(&mut self, failure: Failure) -> Option<Source> {
        let Phase::Loading { source, .. } = &self.phase else {
            return None;
        };
        match source {
            Source::Colombo => {
                self.phase = Phase::Failed(failure);
                None
            }
            Source::Added(added) => {
                self.notes.push(tr!(
                    "bank-load-failed",
                    name = stem(&added.name),
                    reason = failure.message()
                ));
                Some(Source::Colombo)
            }
            Source::Path(path) => {
                self.notes.push(tr!(
                    "bank-load-failed",
                    name = file_name(path),
                    reason = failure.message()
                ));
                Some(Source::Colombo)
            }
        }
    }

    fn poll_adding(&mut self) {
        let Some(job) = &self.adding else {
            return;
        };
        let result = match job.done.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(AddError::Bank(BankError::Download(tr!(
                "bank-error-installer-stopped"
            )))),
        };
        let job = self.adding.take().expect("checked above");
        match result {
            Ok((added, soundfont)) => {
                self.start_engine(Source::Added(added), soundfont);
            }
            Err(error) => self.notes.push(error.message(&job.name)),
        }
    }

    /// Starts the audio on a loaded SoundFont, replacing any engine running.
    fn start_engine(&mut self, source: Source, soundfont: Arc<SoundFont>) {
        // The old engine stops first, so two streams never play at once.
        self.phase = Phase::Offer;
        match Engine::start(soundfont.clone()) {
            Ok(engine) => {
                self.phase = Phase::Ready(Box::new(engine));
                self.soundfont = Some(soundfont);
                self.source = source;
                self.generation += 1;
            }
            Err(error) => {
                self.phase = Phase::Failed(Failure::Audio(error));
            }
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
        self.soundfont = None;
        self.phase = Phase::Installing {
            progress,
            cancel,
            done,
        };
    }

    /// Loads `source` on a worker thread. The engine playing stops: one bank plays at a time.
    /// ColomboGMGS2 that is not installed shows the download page instead.
    fn start_loading(&mut self, ctx: &egui::Context, source: Source) {
        let path = match &source {
            Source::Colombo => match dm_bank::installed(&self.data_dir, &BANK) {
                Some(path) => path,
                None => {
                    self.phase = Phase::Offer;
                    self.soundfont = None;
                    self.source = Source::Colombo;
                    return;
                }
            },
            Source::Added(added) => added.path.clone(),
            Source::Path(path) => path.clone(),
        };
        let (send, done) = channel();
        let ctx_w = ctx.clone();
        std::thread::spawn(move || {
            let result = dm_audio::load_soundfont(&path);
            let _ = send.send(result);
            ctx_w.request_repaint();
        });
        self.soundfont = None;
        self.phase = Phase::Loading { source, done };
    }

    /// Draws everything before the grid: the offer, progress, loading and errors, under the app's
    /// logo and name. It offers ColomboGMGS2 only: much of the app (the short kit list, the kit
    /// descriptions, the ready-made styles) is made for it. Other SoundFonts are chosen in the
    /// menu once it plays.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) {
        if matches!(self.phase, Phase::Ready(_)) {
            return;
        }
        let ctx = ui.ctx().clone();
        let mut start = false;
        // A column of at most 520 points, centred, with text wrapping inside it. A centred layout
        // alone does not wrap long labels on a narrow screen: they ran off both edges in portrait.
        // It scrolls, so a phone held sideways still reaches the button.
        let area = ui.available_rect_before_wrap();
        let width = (area.width() - 2.0 * GUTTER).clamp(0.0, 520.0);
        let builder = egui::UiBuilder::new().max_rect(area);
        ui.scope_builder(builder, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let top = (area.height() * 0.08).clamp(GUTTER, 64.0);
                    ui.add_space(top);
                    ui.vertical_centered(|ui| {
                        ui.set_max_width(width);
                        header(ui);
                        ui.add_space(24.0);
                        card(ui, |ui| {
                            start = self.phase_ui(ui);
                        });
                        ui.add_space(GUTTER);
                    });
                });
        });
        if start {
            self.notes.clear();
            self.start_install(&ctx, None);
        }
    }

    /// The card's contents for the current phase. Returns whether to start the download.
    fn phase_ui(&self, ui: &mut egui::Ui) -> bool {
        let mut start = false;
        // A SoundFont shared to the app before any played.
        if let Some(adding) = self.adding() {
            title(ui, tr!("bank-adding-title"));
            wrapped(ui, stem(adding.name));
            ui.add_space(12.0);
            progress_bar(ui, adding.fraction);
            ui.add_space(20.0);
            if big_button(ui, &tr!("bank-cancel")).clicked() {
                self.cancel_adding();
            }
            return false;
        }
        match &self.phase {
            Phase::Offer => {
                title(ui, tr!("bank-offer-title"));
                for text in &self.notes {
                    wrapped(
                        ui,
                        egui::RichText::new(text).color(ui.visuals().warn_fg_color),
                    );
                    ui.add_space(8.0);
                }
                wrapped(ui, tr!("bank-offer", bank = BANK.name));
                ui.add_space(20.0);
                if primary_button(ui, &tr!("bank-download")).clicked() {
                    start = true;
                }
                ui.add_space(16.0);
                note(
                    ui,
                    tr!(
                        "bank-offer-sizes",
                        download = mib(BANK.archive_bytes),
                        space = mib(BANK.member_bytes)
                    ),
                );
                ui.add_space(4.0);
                note(ui, license());
            }
            Phase::Installing {
                progress, cancel, ..
            } => {
                let current = progress.lock().ok().and_then(|p| *p);
                let (text, fraction) = describe(current);
                title(ui, tr!("bank-installing-title"));
                wrapped(ui, text);
                ui.add_space(12.0);
                progress_bar(ui, Some(fraction));
                ui.add_space(20.0);
                if big_button(ui, &tr!("bank-cancel")).clicked() {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
            Phase::Loading { .. } => {
                title(ui, tr!("bank-loading-title"));
                ui.add(egui::Spinner::new().size(32.0).color(ORANGE));
            }
            Phase::Failed(failure) => {
                title(ui, tr!("bank-failed-title"));
                wrapped(ui, failure.message());
                ui.add_space(20.0);
                if primary_button(ui, &tr!("bank-try-again")).clicked() {
                    start = true;
                }
            }
            Phase::Ready(_) => {}
        }
        start
    }
}

/// Copies a SoundFont in, loads it and checks that it has drum kits. One without is deleted
/// again, unless it was there before: adding a file already added returns the earlier copy.
fn add_and_check(
    dir: &Path,
    name: &str,
    source: &mut dyn Read,
    report: &dyn Fn(u64),
    cancel: &AtomicBool,
) -> Result<(UserSoundfont, Arc<SoundFont>), AddError> {
    let before = dm_bank::soundfonts(dir);
    let added =
        dm_bank::add_soundfont(dir, name, source, report, cancel).map_err(AddError::Bank)?;
    let new = !before.contains(&added);
    let discard = |added: &UserSoundfont| {
        if new {
            let _ = dm_bank::remove_soundfont(dir, &added.name);
        }
    };
    let soundfont = match dm_audio::load_soundfont(&added.path) {
        Ok(soundfont) => soundfont,
        Err(error) => {
            discard(&added);
            return Err(AddError::Audio(error));
        }
    };
    if dm_audio::drum_kits(&soundfont).is_empty() {
        discard(&added);
        return Err(AddError::NoKits(added.name));
    }
    Ok((added, soundfont))
}

/// The logo's orange, from `docs/icon/ticktapdrum.svg`.
const ORANGE: egui::Color32 = egui::Color32::from_rgb(0xFF, 0x6A, 0x3D);

/// The space kept clear at the screen's edges.
const GUTTER: f32 = 16.0;

/// The logo, the app's name and its tagline.
fn header(ui: &mut egui::Ui) {
    if let Some(texture) = crate::logo::texture(ui.ctx()) {
        let side = 96.0;
        ui.add(egui::Image::from_texture((
            texture.id(),
            egui::vec2(side, side),
        )));
        ui.add_space(12.0);
    }
    ui.label(
        egui::RichText::new(tr!("app-name"))
            .size(34.0)
            .strong()
            .color(ui.visuals().strong_text_color()),
    );
    ui.add_space(2.0);
    wrapped(
        ui,
        egui::RichText::new(tr!("app-tagline")).size(16.0).weak(),
    );
}

/// A rounded panel, slightly apart from the background, around one phase's contents.
fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let visuals = ui.visuals();
    egui::Frame::new()
        .fill(visuals.faint_bg_color)
        .stroke(visuals.widgets.noninteractive.bg_stroke)
        .corner_radius(16.0)
        .inner_margin(24.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(add);
        });
}

/// A phase's title, at the top of the card.
fn title(ui: &mut egui::Ui, text: String) {
    ui.label(egui::RichText::new(text).size(22.0).strong());
    ui.add_space(12.0);
}

/// Small, quiet text for details: sizes and the licence.
fn note(ui: &mut egui::Ui, text: String) {
    wrapped(ui, egui::RichText::new(text).size(12.5).weak());
}

/// A progress bar in the logo's orange; one that spins when the share done is not known.
pub(crate) fn progress_bar(ui: &mut egui::Ui, fraction: Option<f32>) {
    match fraction {
        Some(fraction) => {
            ui.add(
                egui::ProgressBar::new(fraction)
                    .show_percentage()
                    .fill(ORANGE)
                    .corner_radius(8.0)
                    .desired_height(20.0),
            );
        }
        None => {
            ui.add(egui::Spinner::new().size(24.0).color(ORANGE));
        }
    }
}

/// The page's main action: a large button in the logo's orange.
fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(text)
                .size(20.0)
                .strong()
                .color(egui::Color32::WHITE),
        )
        .fill(ORANGE)
        .corner_radius(12.0)
        .min_size(egui::vec2(200.0, 52.0)),
    )
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
pub(crate) fn mib(bytes: u64) -> u64 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, sha256: &str) -> SoundfontRef {
        SoundfontRef::File {
            name: name.to_owned(),
            sha256: sha256.to_owned(),
        }
    }

    #[test]
    fn files_match_by_digest_and_by_name_without_one() {
        assert!(same_soundfont(&file("A.sf2", "11"), &file("B.sf2", "11")));
        assert!(!same_soundfont(&file("A.sf2", "11"), &file("A.sf2", "22")));
        assert!(same_soundfont(&file("A.sf2", ""), &file("A.sf2", "22")));
        assert!(!same_soundfont(
            &file("A.sf2", "11"),
            &SoundfontRef::Bank(BANK.id.to_owned())
        ));
    }

    #[test]
    fn names_shown_leave_out_the_extension() {
        assert_eq!(
            display_name(&file("GeneralUser-GS.sf2", "")),
            "GeneralUser-GS"
        );
        assert_eq!(
            display_name(&SoundfontRef::Bank(BANK.id.to_owned())),
            BANK.name
        );
    }
}
