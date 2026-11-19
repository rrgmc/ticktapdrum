//! Plays a beat through the real audio engine with MIDI out on, to a MIDI output, and reports
//! what was sent and how far each message went out from its due time.
//!
//! `DM_SF2=<bank.sf2> cargo run --release -p dm-midi --example engine -- "<output name>" [seconds]`
//!
//! This is the path the app takes: the engine's audio callback queues the messages, and the
//! `dm-midi` thread sends them. It checks the two crates together without the interface.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dm_audio::{Engine, MidiEvent, load_soundfont};
use dm_midi::{MidiOut, MidiStatus};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(name) = args.next() else {
        eprintln!("usage: engine <output name> [seconds]   (needs DM_SF2)");
        std::process::exit(2);
    };
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);
    let bank = std::env::var_os("DM_SF2").expect("DM_SF2 names a SoundFont");
    let soundfont = load_soundfont(bank.as_ref()).expect("the bank loads");

    let mut engine = Engine::start(soundfont).expect("an audio output opens");
    let events = engine
        .take_midi_events()
        .expect("the engine hands out its MIDI queue");
    assert!(
        engine.take_midi_events().is_none(),
        "the queue is handed out once"
    );

    // (message, error in microseconds, positive when late)
    let sent = Arc::new(Mutex::new(Vec::<([u8; 3], i64)>::new()));
    let hook_sent = Arc::clone(&sent);
    let hook = Box::new(move |event: &MidiEvent, at: Instant| {
        let error = if at >= event.at {
            (at - event.at).as_micros() as i64
        } else {
            -((event.at - at).as_micros() as i64)
        };
        hook_sent.lock().unwrap().push((event.bytes, error));
    });
    let mut midi = MidiOut::start_with_hook(events, Some(hook));
    midi.select(Some(name.clone()));
    let deadline = Instant::now() + Duration::from_secs(3);
    while midi.status() != MidiStatus::Connected {
        if Instant::now() > deadline {
            eprintln!("could not open {name:?}: {:?}", midi.status());
            std::process::exit(1);
        }
        thread::sleep(Duration::from_millis(10));
    }
    engine.set_midi_out(true);
    println!(
        "connected to {name:?}; output latency {} frames at {} Hz",
        engine.output_latency_frames(),
        engine.sample_rate()
    );

    // Kick on 1 and 3, snare on 2 and 4, closed hat on every eighth, at 120 BPM.
    for (row, key) in [(0, 36), (1, 38), (2, 42)] {
        engine.set_row_key(row, key);
    }
    engine.set_length(0, 16);
    for step in 0..16 {
        if step % 8 == 0 {
            engine.set_step(0, 0, step, 110);
        }
        if step % 8 == 4 {
            engine.set_step(0, 1, step, 100);
        }
        if step % 2 == 0 {
            engine.set_step(0, 2, step, 80);
        }
    }
    engine.set_tempo(120.0);
    engine.set_kit(0);
    engine.play_pattern(0);
    thread::sleep(Duration::from_secs(seconds));
    engine.stop();
    // Let pending note-offs (four seconds after each hit) go out before turning MIDI out off.
    thread::sleep(Duration::from_millis(4200));
    engine.set_midi_out(false);
    thread::sleep(Duration::from_millis(200));
    drop(midi);

    let sent = sent.lock().unwrap();
    let count = |status: u8| sent.iter().filter(|(b, _)| b[0] == status).count();
    println!(
        "sent {} messages: {} note-on, {} note-off, {} program change",
        sent.len(),
        count(0x99),
        count(0x89),
        count(0xC9)
    );
    let mut errors: Vec<i64> = sent.iter().map(|(_, e)| *e).collect();
    if errors.is_empty() {
        return;
    }
    errors.sort_unstable();
    let n = errors.len();
    let percentile = |p: f64| errors[((n - 1) as f64 * p).round() as usize];
    println!(
        "send time minus due time, microseconds: min {}, median {}, p95 {}, max {}",
        errors[0],
        percentile(0.5),
        percentile(0.95),
        errors[n - 1]
    );
}
