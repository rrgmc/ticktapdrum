//! Plays a kick and snare pattern to a MIDI output and prints how far each message went out from
//! its due time.
//!
//! `cargo run --release -p dm-midi --example send -- "<output name>" [bars]`
//!
//! The pattern is one bar of four beats at 120 BPM: kick on beats 1 and 3, snare on 2 and 4, and
//! a closed hi-hat on every eighth note. Messages enter the queue in small batches a little ahead
//! of their due time, as the audio thread pushes them.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dm_audio::{MIDI_QUEUE_CAPACITY, MidiEvent};
use dm_midi::{MidiOut, MidiStatus};

const KICK: u8 = 36;
const SNARE: u8 = 38;
const CLOSED_HAT: u8 = 42;

/// How long each note sounds before its note-off.
const NOTE_LENGTH: Duration = Duration::from_millis(100);

/// How far ahead of due time messages enter the queue, like an audio block of a few tens of
/// milliseconds.
const LOOKAHEAD: Duration = Duration::from_millis(30);

/// How often the feeding loop wakes, like an audio callback.
const FEED_PERIOD: Duration = Duration::from_millis(10);

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(name) = args.next() else {
        eprintln!("usage: send <output name> [bars]");
        eprintln!("outputs:");
        let (_p, c) = rtrb::RingBuffer::new(1);
        for name in MidiOut::start(c).outputs() {
            eprintln!("  {name}");
        }
        std::process::exit(2);
    };
    let bars: u32 = args.next().and_then(|b| b.parse().ok()).unwrap_or(4);

    // Errors in microseconds, positive when late.
    let errors = Arc::new(Mutex::new(Vec::<i64>::new()));
    let hook_errors = Arc::clone(&errors);
    let hook = Box::new(move |event: &MidiEvent, sent: Instant| {
        let error = if sent >= event.at {
            (sent - event.at).as_micros() as i64
        } else {
            -((event.at - sent).as_micros() as i64)
        };
        hook_errors.lock().unwrap().push(error);
    });

    let (mut producer, consumer) = rtrb::RingBuffer::new(MIDI_QUEUE_CAPACITY);
    let mut midi = MidiOut::start_with_hook(consumer, Some(hook));
    midi.select(Some(name.clone()));

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = midi.status();
        if status == MidiStatus::Connected {
            break;
        }
        if Instant::now() > deadline {
            eprintln!("could not open {name:?}: {status:?}");
            std::process::exit(1);
        }
        thread::sleep(Duration::from_millis(10));
    }
    println!("connected to {name:?}; playing {bars} bars at 120 BPM");

    let start = Instant::now() + Duration::from_millis(100);
    let mut events = pattern(start, bars);
    events.sort_by_key(|e| e.at);
    let total = events.len();
    let mut next = 0;
    while next < events.len() {
        let horizon = Instant::now() + LOOKAHEAD;
        while next < events.len() && events[next].at <= horizon {
            if producer.push(events[next]).is_err() {
                eprintln!("queue full");
            }
            next += 1;
        }
        thread::sleep(FEED_PERIOD);
    }
    thread::sleep(NOTE_LENGTH + Duration::from_millis(200));
    // Dropping sends "All notes off" and joins the thread. (On Android `MidiOut` holds nothing.)
    #[allow(clippy::drop_non_drop)]
    drop(midi);

    let errors = errors.lock().unwrap();
    report(&errors, total);
}

/// The pattern's messages for `bars` bars, starting at `start`.
fn pattern(start: Instant, bars: u32) -> Vec<MidiEvent> {
    let eighth = Duration::from_millis(250);
    let mut events = vec![MidiEvent {
        at: start,
        bytes: [0xC9, 0, 0],
        len: 2,
    }];
    for step in 0..bars * 8 {
        let at = start + eighth * step;
        let mut keys = vec![(CLOSED_HAT, 80)];
        match step % 8 {
            0 | 4 => keys.push((KICK, 110)),
            2 | 6 => keys.push((SNARE, 100)),
            _ => {}
        }
        for (key, velocity) in keys {
            events.push(MidiEvent {
                at,
                bytes: [0x99, key, velocity],
                len: 3,
            });
            events.push(MidiEvent {
                at: at + NOTE_LENGTH,
                bytes: [0x89, key, 0],
                len: 3,
            });
        }
    }
    events
}

fn report(errors: &[i64], queued: usize) {
    println!("sent {} of {queued} messages", errors.len());
    if errors.is_empty() {
        return;
    }
    let mut sorted = errors.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let mean = sorted.iter().sum::<i64>() as f64 / n as f64;
    let mean_abs = sorted.iter().map(|e| e.abs()).sum::<i64>() as f64 / n as f64;
    let percentile = |p: f64| sorted[((n - 1) as f64 * p).round() as usize];
    println!("send time minus due time, in microseconds (positive is late):");
    println!(
        "  mean {mean:.0}, mean absolute {mean_abs:.0}, min {}, median {}, p95 {}, p99 {}, max {}",
        sorted[0],
        percentile(0.5),
        percentile(0.95),
        percentile(0.99),
        sorted[n - 1]
    );
}
