//! Exporting the song as a MIDI file or a WAV recording.
//!
//! MIDI is written at once. A WAV file is rendered on a worker thread, with the same synthesizer
//! and bank as the live sound but no audio device, and reports its progress.
//!
//! Either goes to a [`Target`]: a path, written through a temporary file, or a file the host's
//! picker opened, written straight into it.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};

use dm_audio::{AudioError, OfflineRender, SoundFont, write_wav, write_wav_to};
use dm_project::{Change, Project};

use crate::files::Target;

/// Writes `project` as a MIDI file.
pub(crate) fn write_midi(target: &Target, project: &Project) -> io::Result<()> {
    target.write_all(&dm_project::to_midi(project))
}

/// The sample rate of exported WAV files: CD quality, which every program reads.
const WAV_RATE: u32 = 44_100;
/// How long the last hits ring out after the song ends, in seconds.
const TAIL_SECONDS: f32 = 2.0;

/// A WAV export in progress.
pub(crate) struct WavJob {
    progress: Arc<Mutex<f32>>,
    cancel: Arc<AtomicBool>,
    done: Receiver<Result<Target, String>>,
}

impl WavJob {
    /// Starts rendering `project` into `target` on a worker thread.
    pub(crate) fn start(
        soundfont: Arc<SoundFont>,
        project: &Project,
        target: Target,
        ctx: egui::Context,
    ) -> Self {
        let progress = Arc::new(Mutex::new(0.0));
        let cancel = Arc::new(AtomicBool::new(false));
        let (send, done) = channel();
        let project = project.clone();
        let (progress_w, cancel_w) = (progress.clone(), cancel.clone());
        std::thread::spawn(move || {
            let result = render(soundfont, &project, &target, &progress_w, &cancel_w, &ctx);
            let _ = send.send(result.map(|()| target));
            ctx.request_repaint();
        });
        Self {
            progress,
            cancel,
            done,
        }
    }

    pub(crate) fn progress(&self) -> f32 {
        self.progress.lock().map(|p| *p).unwrap_or(0.0)
    }

    pub(crate) fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// The result, once the worker has finished.
    pub(crate) fn poll(&self) -> Option<Result<Target, String>> {
        match self.done.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(tr!("projects-export-stopped"))),
        }
    }
}

fn render(
    soundfont: Arc<SoundFont>,
    project: &Project,
    target: &Target,
    progress: &Mutex<f32>,
    cancel: &AtomicBool,
    ctx: &egui::Context,
) -> Result<(), String> {
    // The kit plays as it does live: the nearest one when the SoundFont lacks it.
    let kits = dm_audio::drum_kits(&soundfont);
    let name = project.kit_origin().and_then(|o| o.name.as_deref());
    let kit = dm_audio::resolve_kit(&kits, project.kit(), name);
    let mut offline = OfflineRender::new(soundfont, WAV_RATE).map_err(reason)?;
    offline.set_tempo(project.tempo);
    for change in project.full_sync() {
        apply_offline(&mut offline, change);
    }
    if let Some(kit) = kit {
        offline.set_kit(kit);
    }
    let mut report = |fraction: f32| {
        if let Ok(mut p) = progress.lock() {
            *p = fraction;
        }
        ctx.request_repaint();
    };
    let samples = if project.song().is_empty() {
        offline.render_pattern(
            project.patterns()[0].slot(),
            1,
            TAIL_SECONDS,
            &mut report,
            cancel,
        )
    } else {
        offline.render_song(TAIL_SECONDS, &mut report, cancel)
    }
    .map_err(reason)?;
    match target {
        Target::Path(path) => write_wav(path, &samples, WAV_RATE),
        Target::Picked(picked) => write_wav_to(&picked.file, &samples, WAV_RATE),
    }
    .map_err(|e| e.to_string())
}

/// Why a render failed, in words for the export message. The ones people cause, by stopping
/// it or by exporting an empty song, are translated; the rest are rare and stay as the audio
/// library words them.
fn reason(error: AudioError) -> String {
    match error {
        AudioError::Cancelled => tr!("projects-export-cancelled"),
        AudioError::Nothing => tr!("projects-export-nothing"),
        other => other.to_string(),
    }
}

/// Tells the offline renderer about the project, as `apply` tells the live engine.
fn apply_offline(offline: &mut OfflineRender, change: Change) {
    match change {
        Change::Step {
            slot,
            row,
            step,
            velocity,
        } => offline.set_step(slot, row, step, velocity),
        Change::Length { slot, steps } => offline.set_length(slot, steps),
        Change::Song(order) => offline.set_song(&order),
        Change::RowKey { row, key } => offline.set_row_key(row, key),
        Change::RowGain { row, percent } => offline.set_row_gain(row, f32::from(percent) / 100.0),
        Change::ClearSlot { slot } => offline.clear_pattern(slot),
        Change::Kit(program) => offline.set_kit(program),
        Change::Swing(thousandths) => offline.set_swing(f32::from(thousandths) / 1000.0),
    }
}
