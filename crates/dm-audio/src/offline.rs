//! Rendering the song or a pattern to memory without an audio device, and writing the result as
//! a WAV file.
//!
//! [`OfflineRender`] owns the same [`Renderer`] the audio callback uses, with the same sequencer
//! and synthesizer, so an export sounds like playback. It applies every setting at once instead
//! of through the command queue, and renders as fast as the processor allows.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{AudioError, Command, DRUM_CHANNEL, MAX_PATTERNS, NO_NOTE_OFF, Renderer, SoundFont};

/// How much audio, in seconds, is rendered between two calls of the progress callback.
const PROGRESS_SECONDS: f64 = 0.1;

/// The sequencer and synthesizer of the engine without the audio device, for rendering the song
/// to memory, for example to export it as a WAV file.
///
/// The setters take the same values as the [`Engine`](crate::Engine) setters and apply them at
/// once. Settings persist from one render to the next.
pub struct OfflineRender {
    renderer: Renderer,
    sample_rate: u32,
    /// The kit program, sent again after the synthesizer is reset at the start of each render.
    kit: u8,
}

impl OfflineRender {
    /// Builds a synthesizer at `sample_rate` on `soundfont`. Every row, pattern, tempo and swing
    /// setting starts as in a new engine.
    pub fn new(soundfont: Arc<SoundFont>, sample_rate: u32) -> Result<Self, AudioError> {
        let (_commands, renderer) = Renderer::new(&soundfont, sample_rate)?;
        Ok(OfflineRender {
            renderer,
            sample_rate,
            kit: 0,
        })
    }

    /// The sample rate of the rendered audio.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn apply(&mut self, command: Option<Command>) {
        if let Some(command) = command {
            self.renderer.apply(command);
        }
    }

    /// As [`Engine::set_row_key`](crate::Engine::set_row_key).
    pub fn set_row_key(&mut self, row: usize, key: u8) {
        self.apply(Command::row_key(row, key));
    }

    /// As [`Engine::set_row_gain`](crate::Engine::set_row_gain).
    pub fn set_row_gain(&mut self, row: usize, gain: f32) {
        self.apply(Command::row_gain(row, gain));
    }

    /// As [`Engine::set_step`](crate::Engine::set_step).
    pub fn set_step(&mut self, pattern: usize, row: usize, step: usize, velocity: u8) {
        self.apply(Command::step(pattern, row, step, velocity));
    }

    /// As [`Engine::set_length`](crate::Engine::set_length).
    pub fn set_length(&mut self, pattern: usize, steps: usize) {
        self.apply(Command::length(pattern, steps));
    }

    /// As [`Engine::clear_pattern`](crate::Engine::clear_pattern).
    pub fn clear_pattern(&mut self, pattern: usize) {
        self.apply(Command::clear_pattern(pattern));
    }

    /// As [`Engine::set_song`](crate::Engine::set_song).
    pub fn set_song(&mut self, order: &[usize]) {
        for command in Command::song(order) {
            self.renderer.apply(command);
        }
    }

    /// Ignored: [`OfflineRender::render_song`] always plays the song once.
    pub fn set_song_loop(&mut self, looped: bool) {
        let _ = looped;
    }

    /// As [`Engine::set_tempo`](crate::Engine::set_tempo).
    pub fn set_tempo(&mut self, bpm: f32) {
        self.apply(Command::tempo(bpm));
    }

    /// As [`Engine::set_swing`](crate::Engine::set_swing).
    pub fn set_swing(&mut self, amount: f32) {
        self.apply(Command::swing(amount));
    }

    /// As [`Engine::set_kit`](crate::Engine::set_kit).
    pub fn set_kit(&mut self, program: u8) {
        self.kit = program.min(127);
    }

    /// Renders the song once from entry 0, without looping, then `tail_seconds` more so the last
    /// hits ring out. Returns interleaved stereo frames, left first.
    ///
    /// The length is known before rendering: the frames of all the song's steps at the current
    /// tempo, rounded up, plus the tail. `progress` is called with 0.0 first, then with the
    /// fraction done after about every 0.1 s of audio, and with 1.0 at the end. Setting `cancel`
    /// stops the render within one 64-frame block and returns [`AudioError::Cancelled`]. An empty
    /// song returns [`AudioError::Nothing`].
    pub fn render_song(
        &mut self,
        tail_seconds: f32,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<f32>, AudioError> {
        let sequencer = &self.renderer.sequencer;
        let steps: usize = sequencer.song[..sequencer.song_len]
            .iter()
            .map(|&pattern| sequencer.lengths[usize::from(pattern)])
            .sum();
        self.render(
            Command::PlaySong { from: 0 },
            steps,
            tail_seconds,
            progress,
            cancel,
        )
    }

    /// The same as [`OfflineRender::render_song`], for `pattern` played `times` times through
    /// its length. A pattern out of range or `times` of 0 returns [`AudioError::Nothing`].
    pub fn render_pattern(
        &mut self,
        pattern: usize,
        times: usize,
        tail_seconds: f32,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<f32>, AudioError> {
        if pattern >= MAX_PATTERNS {
            return Err(AudioError::Nothing);
        }
        let steps = self.renderer.sequencer.lengths[pattern] * times;
        let play = Command::PlayPattern {
            pattern: pattern as u8,
        };
        self.render(play, steps, tail_seconds, progress, cancel)
    }

    /// Starts play with `play` at frame 0 of a silent synthesizer, stops it where step `steps`
    /// would start, and renders on to the end of the tail.
    fn render(
        &mut self,
        play: Command,
        steps: usize,
        tail_seconds: f32,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<f32>, AudioError> {
        if steps == 0 {
            return Err(AudioError::Nothing);
        }
        self.reset();
        let frames_per_step = self.renderer.sequencer.clock.frames_per_step;
        let (play_end, total) =
            render_frames(steps, frames_per_step, tail_seconds, self.sample_rate);
        let mut out = vec![0.0_f32; 2 * total];
        self.renderer.sequencer.song_loop = false;
        self.renderer.apply(play);

        let block = self.renderer.left.len();
        let report_every = (PROGRESS_SECONDS * f64::from(self.sample_rate) / block as f64)
            .ceil()
            .max(1.0) as usize;
        progress(0.0);
        for (index, chunk) in out.chunks_mut(2 * block).enumerate() {
            if cancel.load(Ordering::Relaxed) {
                self.stop();
                return Err(AudioError::Cancelled);
            }
            // The step after the last is due in this block: stop before it can fire. A song that
            // does not loop stops itself here too.
            if (self.renderer.clock + block as u64) as f64 > play_end {
                self.stop();
            }
            // Each chunk is one whole synthesizer block, the last one possibly shorter.
            self.renderer.render(chunk, 2);
            if (index + 1) % report_every == 0 {
                progress(((index + 1) * block) as f32 / total as f32);
            }
        }
        self.stop();
        progress(1.0);
        Ok(out)
    }

    /// Silences the synthesizer, forgets pending note-offs, and moves the frame count back to 0,
    /// so every render starts the same way. Keeps the kit.
    fn reset(&mut self) {
        self.stop();
        let renderer = &mut self.renderer;
        renderer.synth.reset();
        renderer
            .synth
            .process_midi_message(DRUM_CHANNEL, 0xC0, i32::from(self.kit), 0);
        renderer.note_off_at.fill(NO_NOTE_OFF);
        renderer.clock = 0;
        renderer.read = renderer.left.len();
    }

    fn stop(&mut self) {
        if self.renderer.sequencer.clock.playing {
            self.renderer.apply(Command::Stop);
        }
    }
}

/// The frame at which step `steps` would start, and the whole length of a render in frames:
/// that frame rounded up, plus `tail_seconds` (at least 0) at `sample_rate`.
fn render_frames(
    steps: usize,
    frames_per_step: f64,
    tail_seconds: f32,
    sample_rate: u32,
) -> (f64, usize) {
    let play_end = steps as f64 * frames_per_step;
    let tail = if tail_seconds.is_nan() {
        0.0
    } else {
        f64::from(tail_seconds.max(0.0))
    };
    let total = play_end.ceil() as usize + (tail * f64::from(sample_rate)).round() as usize;
    (play_end, total)
}

/// Writes interleaved stereo `samples` as a 16-bit PCM WAV file, little endian. Samples are
/// clipped to -1.0..=1.0; a sample that is not a number is written as 0. An odd last sample is
/// dropped.
///
/// The file is written under a temporary name in the same folder and then renamed over `path`,
/// so a failed write never leaves a partial file at `path`.
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> io::Result<()> {
    let frames = samples.len() / 2;
    let data_bytes = u32::try_from(frames * 4)
        .ok()
        .filter(|&bytes| bytes <= u32::MAX - 36)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "too long for a WAV file"))?;
    let temp = temp_path(path)?;
    let result = write_wav_file(&temp, &samples[..2 * frames], sample_rate, data_bytes)
        .and_then(|()| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// `.<file name>.tmp` beside `path`.
fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "the WAV path has no file name")
    })?;
    let mut temp = std::ffi::OsString::from(".");
    temp.push(name);
    temp.push(".tmp");
    Ok(path.with_file_name(temp))
}

fn write_wav_file(
    path: &Path,
    samples: &[f32],
    sample_rate: u32,
    data_bytes: u32,
) -> io::Result<()> {
    let file = File::create(path)?;
    write_wav_data(&file, samples, sample_rate, data_bytes)?;
    file.sync_all()
}

/// Writes interleaved stereo `samples` as a 16-bit PCM WAV file into `file`, which must be
/// empty and open for writing, then flushes it to the device. For a file the caller opened
/// itself, such as one an Android file picker handed over. Samples are treated as in
/// [`write_wav`]; unlike it, a failed write may leave a partial file.
pub fn write_wav_to(file: &File, samples: &[f32], sample_rate: u32) -> io::Result<()> {
    let frames = samples.len() / 2;
    let data_bytes = u32::try_from(frames * 4)
        .ok()
        .filter(|&bytes| bytes <= u32::MAX - 36)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "too long for a WAV file"))?;
    write_wav_data(file, &samples[..2 * frames], sample_rate, data_bytes)?;
    file.sync_all()
}

fn write_wav_data(
    file: &File,
    samples: &[f32],
    sample_rate: u32,
    data_bytes: u32,
) -> io::Result<()> {
    const CHANNELS: u16 = 2;
    const BYTES_PER_SAMPLE: u16 = 2;
    let block_align = CHANNELS * BYTES_PER_SAMPLE;
    let mut w = BufWriter::new(file);
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_bytes).to_le_bytes())?;
    w.write_all(b"WAVE")?;
    w.write_all(b"fmt ")?;
    w.write_all(&16_u32.to_le_bytes())?;
    w.write_all(&1_u16.to_le_bytes())?; // PCM
    w.write_all(&CHANNELS.to_le_bytes())?;
    w.write_all(&sample_rate.to_le_bytes())?;
    w.write_all(&(sample_rate * u32::from(block_align)).to_le_bytes())?;
    w.write_all(&block_align.to_le_bytes())?;
    w.write_all(&(8 * BYTES_PER_SAMPLE).to_le_bytes())?;
    w.write_all(b"data")?;
    w.write_all(&data_bytes.to_le_bytes())?;
    for &sample in samples {
        w.write_all(&to_i16(sample).to_le_bytes())?;
    }
    w.flush()
}

/// A sample as 16-bit PCM, clipped; not a number becomes 0.
fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{energy, soundfont};

    const RATE: u32 = 48_000;
    const BLOCK: usize = 64;

    macro_rules! soundfont_or_skip {
        () => {
            match soundfont() {
                Some(font) => font,
                None => {
                    println!("DM_TEST_SF2 is not set; skipping");
                    return;
                }
            }
        };
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dm-audio-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn write_wav_writes_a_valid_header_and_every_sample() {
        let samples = [0.0, 0.5, -0.5, 1.0, 2.0, -3.0, f32::NAN, -1.0];
        let path = scratch("header.wav");
        write_wav(&path, &samples, 44_100).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 44 + 2 * samples.len());
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[4..8], &(36 + 16_u32).to_le_bytes());
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(&bytes[16..20], &16_u32.to_le_bytes());
        assert_eq!(&bytes[20..22], &1_u16.to_le_bytes(), "PCM");
        assert_eq!(&bytes[22..24], &2_u16.to_le_bytes(), "stereo");
        assert_eq!(&bytes[24..28], &44_100_u32.to_le_bytes());
        assert_eq!(&bytes[28..32], &(44_100_u32 * 4).to_le_bytes());
        assert_eq!(&bytes[32..34], &4_u16.to_le_bytes(), "block align");
        assert_eq!(&bytes[34..36], &16_u16.to_le_bytes(), "bits");
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(&bytes[40..44], &16_u32.to_le_bytes());
        let read: Vec<i16> = bytes[44..]
            .chunks(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(
            read,
            [0, 16_384, -16_384, 32_767, 32_767, -32_767, 0, -32_767]
        );
        assert!(
            !scratch(".header.wav.tmp").exists(),
            "the temporary file is gone"
        );
    }

    #[test]
    fn write_wav_round_trips_the_frame_count_and_replaces_a_file() {
        let path = scratch("frames.wav");
        fs::write(&path, b"old").unwrap();
        let samples = vec![0.25_f32; 2 * 1234 + 1];
        write_wav(&path, &samples, RATE).unwrap();
        let bytes = fs::read(&path).unwrap();
        let data = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        assert_eq!(data / 4, 1234, "the odd last sample is dropped");
        assert_eq!(bytes.len(), 44 + data);
    }

    #[test]
    fn write_wav_to_writes_the_same_bytes_into_an_open_file() {
        let samples = vec![0.1_f32, -0.2, 0.3, -0.4, 0.5];
        let by_path = scratch("by-path.wav");
        write_wav(&by_path, &samples, RATE).unwrap();
        let by_file = scratch("by-file.wav");
        let file = File::create(&by_file).unwrap();
        write_wav_to(&file, &samples, RATE).unwrap();
        drop(file);
        assert_eq!(fs::read(&by_file).unwrap(), fs::read(&by_path).unwrap());
    }

    #[test]
    fn render_frames_counts_the_steps_and_the_tail() {
        assert_eq!(render_frames(16, 6000.0, 0.0, RATE), (96_000.0, 96_000));
        assert_eq!(render_frames(16, 6000.0, 1.5, RATE), (96_000.0, 168_000));
        let (end, total) = render_frames(3, 4973.684, 0.0, 44_100);
        assert_eq!(total, end.ceil() as usize);
        assert_eq!(render_frames(1, 6000.0, -2.0, RATE).1, 6000);
        assert_eq!(render_frames(1, 6000.0, f32::NAN, RATE).1, 6000);
    }

    /// An offline renderer with a kick on row 0 and a snare on row 1, at 120 BPM.
    fn kit(font: &Arc<SoundFont>) -> OfflineRender {
        let mut offline = OfflineRender::new(Arc::clone(font), RATE).unwrap();
        offline.set_row_key(0, 36);
        offline.set_row_key(1, 38);
        offline.set_tempo(120.0);
        offline
    }

    fn no_progress() -> impl FnMut(f32) {
        |_| {}
    }

    /// The first frame of interleaved stereo `out` above a small threshold.
    fn first_sound(out: &[f32]) -> Option<usize> {
        out.chunks(2)
            .position(|f| f[0].abs() > 1e-4 || f[1].abs() > 1e-4)
    }

    #[test]
    fn a_pattern_renders_hits_where_the_steps_are() {
        let font = soundfont_or_skip!();
        let mut offline = kit(&font);
        offline.set_step(2, 0, 4, 127);
        offline.set_step(2, 1, 12, 127);
        let cancel = AtomicBool::new(false);
        let out = offline
            .render_pattern(2, 2, 0.5, &mut no_progress(), &cancel)
            .unwrap();
        assert_eq!(out.len(), 2 * (32 * 6000 + 24_000));
        let step4 = 4 * 6000;
        assert!(
            energy(&out[..2 * step4]) < 1e-6,
            "silence before the first hit"
        );
        assert_eq!(first_sound(&out), Some(step4));
        assert!(energy(&out[2 * step4..2 * (step4 + 4800)]) > 1e-3);
        let step12 = 12 * 6000;
        assert!(energy(&out[2 * step12..2 * (step12 + 4800)]) > 1e-3);
        // The second time through the pattern sounds too.
        let again = 20 * 6000;
        assert!(energy(&out[2 * again..2 * (again + 4800)]) > 1e-3);
    }

    #[test]
    fn the_song_renders_once_with_its_computed_length() {
        let font = soundfont_or_skip!();
        let mut offline = kit(&font);
        let cancel = AtomicBool::new(false);
        assert!(matches!(
            offline.render_song(1.0, &mut no_progress(), &cancel),
            Err(AudioError::Nothing)
        ));
        offline.set_length(1, 32);
        offline.set_step(0, 0, 0, 120);
        offline.set_step(1, 1, 31, 120);
        offline.set_song(&[0, 1, 0]);
        offline.set_song_loop(true);
        offline.set_tempo(133.0);
        let mut calls = Vec::new();
        let out = offline
            .render_song(2.0, &mut |f| calls.push(f), &cancel)
            .unwrap();
        let per_step: f64 = 48_000.0 * 60.0 / (133.0 * 4.0);
        let expected = (64.0 * per_step).ceil() as usize + 96_000;
        assert_eq!(out.len(), 2 * expected);
        // The song does not loop: after its last step only the tail sounds, which decays.
        let last = out.len() - 2 * 4800;
        assert!(energy(&out[last..]) < 1e-4);
        assert_eq!(calls.first(), Some(&0.0));
        assert_eq!(calls.last(), Some(&1.0));
        assert!(calls.windows(2).all(|w| w[0] <= w[1]));
        let seconds = expected as f32 / RATE as f32;
        assert!(calls.len() as f32 <= seconds / 0.1 + 3.0);
        assert!(calls.len() as f32 >= seconds / 0.1 - 1.0);
    }

    #[test]
    fn cancel_stops_the_render() {
        let font = soundfont_or_skip!();
        let mut offline = kit(&font);
        offline.set_step(0, 0, 0, 120);
        let cancel = AtomicBool::new(false);
        let mut calls = 0;
        let result = offline.render_pattern(
            0,
            100,
            0.0,
            &mut |_| {
                calls += 1;
                if calls == 3 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            &cancel,
        );
        assert!(matches!(result, Err(AudioError::Cancelled)));
        assert_eq!(calls, 3);
        // A later render starts afresh.
        cancel.store(false, Ordering::Relaxed);
        let out = offline
            .render_pattern(0, 1, 0.0, &mut no_progress(), &cancel)
            .unwrap();
        assert_eq!(first_sound(&out), Some(0));
    }

    #[test]
    fn swing_shifts_the_offline_render() {
        let font = soundfont_or_skip!();
        let mut offline = kit(&font);
        offline.set_step(0, 1, 1, 127);
        let cancel = AtomicBool::new(false);
        let render = |offline: &mut OfflineRender| {
            let out = offline
                .render_pattern(0, 1, 0.0, &mut no_progress(), &cancel)
                .unwrap();
            assert_eq!(out.len(), 2 * 16 * 6000, "swing does not change the length");
            first_sound(&out)
        };
        assert_eq!(render(&mut offline), Some(6000 / BLOCK * BLOCK));
        offline.set_swing(0.25);
        assert_eq!(render(&mut offline), Some(7500 / BLOCK * BLOCK));
        offline.set_swing(0.0);
        assert_eq!(render(&mut offline), Some(6000 / BLOCK * BLOCK));
    }

    #[test]
    fn offline_sounds_like_the_live_renderer() {
        let font = soundfont_or_skip!();
        let mut offline = kit(&font);
        offline.set_kit(8);
        offline.set_row_gain(1, 0.5);
        offline.set_swing(0.33);
        for step in [0, 3, 6, 9] {
            offline.set_step(0, 0, step, 110);
            offline.set_step(0, 1, step + 1, 100);
        }
        let cancel = AtomicBool::new(false);
        // Render twice: the second render must not carry anything over from the first.
        offline
            .render_pattern(0, 1, 0.0, &mut no_progress(), &cancel)
            .unwrap();
        let rendered = offline
            .render_pattern(0, 1, 0.0, &mut no_progress(), &cancel)
            .unwrap();

        let (mut commands, mut live) = Renderer::new(&font, RATE).unwrap();
        commands.set_kit(8);
        for command in [
            Command::row_key(0, 36),
            Command::row_key(1, 38),
            Command::row_gain(1, 0.5),
            Command::tempo(120.0),
            Command::swing(0.33),
        ] {
            commands.send(command.unwrap());
        }
        for step in [0, 3, 6, 9] {
            commands.send(Command::step(0, 0, step, 110).unwrap());
            commands.send(Command::step(0, 1, step + 1, 100).unwrap());
        }
        commands.send(Command::PlayPattern { pattern: 0 });
        let mut out = vec![0.0_f32; rendered.len()];
        live.render(&mut out, 2);
        assert!(energy(&out) > 1e-3);
        assert_eq!(out, rendered);
    }
}
