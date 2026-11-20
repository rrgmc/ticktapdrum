//! The Android audio output: an AAudio stream opened directly, not through cpal.
//!
//! cpal fixes AAudio's performance mode at compile time. Asking it for low-latency mode broke a
//! real phone: the device granted the mode (its MMAP path), then failed to route the stream, so
//! every start failed with "Disconnected". This module asks for low latency and falls back:
//!
//! 1. low-latency mode, exclusive sharing;
//! 2. low-latency mode, shared;
//! 3. the default performance mode, shared.
//!
//! An attempt fails if the stream does not open, does not start, reports an error or a
//! disconnect before it is trusted, or (except for the last attempt) makes no data callbacks
//! within [`START_TIMEOUT`]. A failed stream is closed before the next attempt.
//!
//! Every attempt asks for 32-bit float, stereo, and leaves the sample rate to the device, since
//! asking for another rate can cost the low-latency path. When low-latency mode is granted, the
//! buffer is set to [`LOW_LATENCY_BURSTS`] bursts, the usual low-latency practice.
//!
//! The data callback runs the same [`Renderer`] as the cpal callback, and computes the output
//! latency from the stream's timestamps the same way cpal does. A stream that fails later (a
//! route change, headphones plugged) is logged and stays silent, as on the cpal path; it is not
//! reopened.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use ndk::audio::{
    AudioCallbackResult, AudioDirection, AudioError as AaudioError, AudioFormat,
    AudioPerformanceMode, AudioSharingMode, AudioStream, AudioStreamBuilder, AudioStreamState,
    Clockid,
};

use crate::{AudioError, OpenOutput, Renderer, SoundFont, latency_frames};

/// How long a started stream has to make [`TRUSTED_CALLBACKS`] data callbacks.
const START_TIMEOUT: Duration = Duration::from_secs(1);

/// How often the start check looks at the stream.
const START_POLL: Duration = Duration::from_millis(5);

/// The data callbacks a started stream must make, with no error, before it is trusted.
const TRUSTED_CALLBACKS: u32 = 3;

/// The buffer size, in bursts, set when low-latency mode is granted.
const LOW_LATENCY_BURSTS: i32 = 2;

/// The channels asked for: stereo.
const CHANNELS: i32 = 2;

/// One way of opening the stream.
struct Attempt {
    name: &'static str,
    performance: AudioPerformanceMode,
    sharing: AudioSharingMode,
}

/// The ways of opening the stream, in the order they are tried.
const ATTEMPTS: [Attempt; 3] = [
    Attempt {
        name: "low latency, exclusive",
        performance: AudioPerformanceMode::LowLatency,
        sharing: AudioSharingMode::Exclusive,
    },
    Attempt {
        name: "low latency, shared",
        performance: AudioPerformanceMode::LowLatency,
        sharing: AudioSharingMode::Shared,
    },
    Attempt {
        name: "default mode, shared",
        performance: AudioPerformanceMode::None,
        sharing: AudioSharingMode::Shared,
    },
];

/// A running AAudio output stream. Dropping it stops and closes the stream, which ends the data
/// callback before the renderer it owns is dropped.
pub(crate) struct Stream {
    inner: AudioStream,
}

// SAFETY: an AAudio stream handle may be used from any thread. The engine only drops it, so no
// two threads call into the stream at the same time. The callbacks the stream owns are `Send`.
unsafe impl Send for Stream {}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Err(e) = self.inner.request_stop() {
            log::debug!("audio output: stopping the AAudio stream: {e}");
        }
        // `inner` is dropped next, which closes the stream.
    }
}

/// What the callbacks report to the start check.
#[derive(Default)]
struct Probe {
    /// Data callbacks made, wrapping. Only the start check reads it.
    callbacks: AtomicU32,
    /// The last error the error callback reported, or 0.
    error: AtomicI32,
}

/// What the data callback needs once the stream is open: the renderer and the stream format.
struct Handoff {
    renderer: Renderer,
    channels: usize,
    sample_rate: f64,
}

/// Opens the output, trying each of [`ATTEMPTS`] in turn, and returns the first that runs.
pub(crate) fn open_output(soundfont: &Arc<SoundFont>) -> Result<OpenOutput, AudioError> {
    let mut last_error = None;
    for (index, attempt) in ATTEMPTS.iter().enumerate() {
        let last = index + 1 == ATTEMPTS.len();
        match try_open(soundfont, attempt, last) {
            Ok(output) => return Ok(output),
            Err(e) => {
                if last {
                    log::warn!("audio output: AAudio {} failed: {e}", attempt.name);
                } else {
                    log::warn!(
                        "audio output: AAudio {} failed: {e}; trying the next mode",
                        attempt.name
                    );
                }
                last_error = Some(e);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| AudioError::Device("no AAudio mode to try".into())))
}

/// Opens and starts one stream as `attempt` asks, and waits until it is trusted.
fn try_open(
    soundfont: &Arc<SoundFont>,
    attempt: &Attempt,
    last: bool,
) -> Result<OpenOutput, AudioError> {
    log::info!(
        "audio output: trying AAudio {}: performance {:?}, sharing {:?}",
        attempt.name,
        attempt.performance,
        attempt.sharing
    );
    let probe = Arc::new(Probe::default());
    let (mut handoff, receiver) = rtrb::RingBuffer::<Handoff>::new(1);

    let builder = AudioStreamBuilder::new()
        .map_err(|e| device("create the stream builder", e))?
        .direction(AudioDirection::Output)
        .performance_mode(attempt.performance)
        .sharing_mode(attempt.sharing)
        .format(AudioFormat::PCM_Float)
        .channel_count(CHANNELS)
        .data_callback(data_callback(receiver, Arc::clone(&probe)))
        .error_callback(error_callback(Arc::clone(&probe)));
    let stream = Stream {
        inner: builder
            .open_stream()
            .map_err(|e| device("open the stream", e))?,
    };
    let inner = &stream.inner;

    let format = inner.format();
    if format != AudioFormat::PCM_Float {
        return Err(AudioError::Device(format!(
            "the stream opened with format {format:?}, not float"
        )));
    }
    let sample_rate = u32::try_from(inner.sample_rate())
        .ok()
        .filter(|rate| *rate > 0)
        .ok_or_else(|| AudioError::Device(format!("sample rate {}", inner.sample_rate())))?;
    let channels = u16::try_from(inner.channel_count())
        .ok()
        .filter(|channels| *channels > 0)
        .ok_or_else(|| AudioError::Device(format!("channel count {}", inner.channel_count())))?;
    let performance = inner.performance_mode();
    let burst = inner.frames_per_burst();
    if performance == AudioPerformanceMode::LowLatency
        && burst > 0
        && let Err(e) = inner.set_buffer_size_in_frames(burst.saturating_mul(LOW_LATENCY_BURSTS))
        // AAudio returns the new size, a positive number, on success; the ndk crate reports any
        // non-zero result as an error. Only a negative one is a real error.
        && i32::from(e) < 0
    {
        log::warn!("audio output: could not set the buffer to {LOW_LATENCY_BURSTS} bursts: {e}");
    }
    log::info!(
        "audio output: AAudio {} opened: performance {:?}, sharing {:?}, {} Hz, {} channels, \
         burst {} frames, buffer {} of {} frames",
        attempt.name,
        performance,
        inner.sharing_mode(),
        sample_rate,
        channels,
        burst,
        inner.buffer_size_in_frames(),
        inner.buffer_capacity_in_frames()
    );

    let (commands, mut renderer) = Renderer::new(soundfont, sample_rate)?;
    let status = Arc::clone(&renderer.status);
    let midi_events = renderer.take_midi_events();
    let live_hits = renderer.take_live_hits();
    let heard_hits = renderer.take_heard_hits();
    handoff
        .push(Handoff {
            renderer,
            channels: usize::from(channels),
            sample_rate: f64::from(sample_rate),
        })
        .map_err(|_| AudioError::Device("hand the renderer to the stream".into()))?;

    inner
        .request_start()
        .map_err(|e| device("start the stream", e))?;
    wait_until_trusted(inner, &probe, last)?;

    let buffer_frames = u32::try_from(inner.buffer_size_in_frames()).ok();
    log::info!(
        "audio output: AAudio {} running: performance {:?}, sharing {:?}, {} Hz, {} channels, \
         burst {} frames, buffer {:?} frames",
        attempt.name,
        inner.performance_mode(),
        inner.sharing_mode(),
        sample_rate,
        channels,
        burst,
        buffer_frames
    );
    Ok(OpenOutput {
        stream,
        commands,
        status,
        sample_rate,
        channels,
        buffer_frames,
        midi_events,
        live_hits,
        heard_hits,
    })
}

/// Waits until a started stream has made [`TRUSTED_CALLBACKS`] data callbacks with no error.
/// Fails if it reports an error or leaves the started state first. If no callbacks come within
/// [`START_TIMEOUT`], fails unless this is the `last` attempt, which is kept with a warning.
fn wait_until_trusted(stream: &AudioStream, probe: &Probe, last: bool) -> Result<(), AudioError> {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        let error = probe.error.load(Ordering::Relaxed);
        if error != 0 {
            return Err(AudioError::Device(format!(
                "the stream reported {} after starting",
                AaudioError::from(error)
            )));
        }
        // Waiting for a change from `Unknown` with no timeout refreshes the client state from
        // the audio service and returns at once.
        match stream.wait_for_state_change(AudioStreamState::Unknown, 0) {
            Ok(AudioStreamState::Starting | AudioStreamState::Started) => {}
            Ok(state) => {
                return Err(AudioError::Device(format!(
                    "the stream went to state {state:?} after starting"
                )));
            }
            Err(e) => return Err(device("read the stream state", e)),
        }
        let callbacks = probe.callbacks.load(Ordering::Relaxed);
        if callbacks >= TRUSTED_CALLBACKS {
            return Ok(());
        }
        if Instant::now() >= deadline {
            if last {
                log::warn!(
                    "audio output: {callbacks} audio callbacks within {START_TIMEOUT:?}; \
                     keeping the stream, as no other mode is left"
                );
                return Ok(());
            }
            return Err(AudioError::Device(format!(
                "{callbacks} audio callbacks within {START_TIMEOUT:?}"
            )));
        }
        std::thread::sleep(START_POLL);
    }
}

/// The data callback. It takes the renderer from `receiver` on its first call and renders into
/// the stream's float buffer. It does not allocate, lock or block.
fn data_callback(
    mut receiver: rtrb::Consumer<Handoff>,
    probe: Arc<Probe>,
) -> ndk::audio::AudioStreamDataCallback {
    let mut active: Option<Handoff> = None;
    Box::new(move |stream, data, num_frames| {
        probe.callbacks.fetch_add(1, Ordering::Relaxed);
        if active.is_none() {
            active = receiver.pop().ok();
        }
        let frames = usize::try_from(num_frames).unwrap_or(0);
        let Some(handoff) = active.as_mut() else {
            // Not reached: the renderer is handed over before the stream starts. The stream is
            // float, as checked before starting.
            let channels = usize::try_from(stream.channel_count()).unwrap_or(0);
            // SAFETY: AAudio passes a buffer of `num_frames` frames of the stream's format.
            unsafe { std::ptr::write_bytes(data.cast::<f32>(), 0, frames * channels) };
            return AudioCallbackResult::Continue;
        };
        // SAFETY: AAudio passes a buffer of `num_frames` frames of `channels` float samples,
        // the format and channel count the stream opened with, valid for this call only.
        let out = unsafe {
            std::slice::from_raw_parts_mut(data.cast::<f32>(), frames * handoff.channels)
        };
        let latency = output_latency(stream, handoff.sample_rate, frames);
        handoff.renderer.set_latency(latency);
        handoff.renderer.render(out, handoff.channels);
        AudioCallbackResult::Continue
    })
}

/// The error callback. It records the error for the start check and logs it, as the cpal path
/// does. AAudio forbids stopping or closing the stream from here.
fn error_callback(probe: Arc<Probe>) -> ndk::audio::AudioStreamErrorCallback {
    Box::new(move |_stream, error| {
        probe.error.store(i32::from(error), Ordering::Relaxed);
        log::warn!("audio stream error: {error}");
    })
}

/// The output latency of one callback in frames: from now to the predicted presentation of the
/// callback's first frame. AAudio's timestamp gives the time a recent frame was presented; the
/// callback's first frame is `frames_written` and lies `frames_written - frame_position` frames
/// after it. This is how cpal computes its playback instant. Falls back to `frames`, the length
/// of the callback's buffer, when there is no timestamp yet or the result is implausible.
fn output_latency(stream: &AudioStream, sample_rate: f64, frames: usize) -> u32 {
    let seconds = match stream.timestamp(Clockid::Monotonic) {
        Ok(timestamp) => {
            let ahead = stream.frames_written() - timestamp.frame_position;
            let playback_nanos =
                timestamp.time_nanoseconds as f64 + ahead as f64 * 1e9 / sample_rate;
            (playback_nanos - monotonic_nanos()) / 1e9
        }
        Err(_) => 0.0,
    };
    latency_frames(seconds, sample_rate, frames)
}

/// The current `CLOCK_MONOTONIC` time in nanoseconds, the clock of AAudio's timestamps.
fn monotonic_nanos() -> f64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `now` is a valid timespec to write to. CLOCK_MONOTONIC always exists on Android.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) };
    // A double keeps well under a microsecond of precision for any plausible uptime.
    now.tv_sec as f64 * 1e9 + now.tv_nsec as f64
}

/// An `AudioError::Device` for an AAudio call that failed.
fn device(what: &str, error: AaudioError) -> AudioError {
    AudioError::Device(format!("{what}: {error} ({})", i32::from(error)))
}
