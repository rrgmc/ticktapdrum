//! Writes a minimal drum-only Yamaha style file, to find out whether an arranger keyboard such as
//! the PSR-SX720 accepts one. See `docs/research/arranger-style-export.md`, "Suggested next steps".
//!
//! The file is a format-0 Standard MIDI File at 1920 ticks per beat, with no CASM, OTS or MDB
//! block. Bar 1 is the setup (`SFF1` or `SFF2`, then `SInt`); after it come `Main A` (two bars of
//! a rock beat) and `Fill In AA` (one bar). Every note is on channel 10, the Rhythm part, with the
//! standard kit (bank MSB 127, LSB 0, program 0).
//!
//! Usage: `cargo run -p dm-project --example yamaha_style -- <out.sty> [--sff2] [--name <name>]`

/// Ticks in one beat. SFF2 styles appear to need 1920; SFF1 allows it too.
const TICKS_PER_BEAT: u32 = 1920;
const TICKS_PER_STEP: u32 = TICKS_PER_BEAT / 4;
const STEPS_PER_BAR: u32 = 16;
const TICKS_PER_BAR: u32 = TICKS_PER_STEP * STEPS_PER_BAR;
const NOTE_TICKS: u32 = TICKS_PER_STEP / 2;
const TEMPO_BPM: u32 = 100;

/// Channel 10 (Rhythm) and channel 9 (Sub-Rhythm), counted from 0.
const RHYTHM: u8 = 9;
const SUB_RHYTHM: u8 = 8;

const KICK: u8 = 36;
const SNARE: u8 = 38;
const CLOSED_HAT: u8 = 42;
const OPEN_HAT: u8 = 46;
const HIGH_TOM: u8 = 50;
const MID_TOM: u8 = 47;
const LOW_TOM: u8 = 45;
const FLOOR_TOM: u8 = 43;

/// One hit: the step from the section's start, the key and the velocity.
type Hit = (u32, u8, u8);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut path = None;
    let mut sff2 = false;
    let mut name = String::from("TTD Test");
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--sff2" => sff2 = true,
            "--name" => match rest.next() {
                Some(value) => name.clone_from(value),
                None => usage(),
            },
            _ if path.is_none() && !arg.starts_with("--") => path = Some(arg.clone()),
            _ => usage(),
        }
    }
    let Some(path) = path else { usage() };

    let bytes = style(&name, sff2);
    if let Err(e) = std::fs::write(&path, &bytes) {
        eprintln!("{path}: {e}");
        std::process::exit(1);
    }
    println!(
        "wrote {path}: {} bytes, {}, sections Main A and Fill In AA",
        bytes.len(),
        if sff2 { "SFF2" } else { "SFF1" }
    );
}

fn usage() -> ! {
    eprintln!("usage: yamaha_style <out.sty> [--sff2] [--name <name>]");
    std::process::exit(2);
}

/// The whole style file.
fn style(name: &str, sff2: bool) -> Vec<u8> {
    let mut track = Track::default();

    // Bar 1: the setup.
    let microseconds = 60_000_000 / TEMPO_BPM;
    let [_, a, b, c] = microseconds.to_be_bytes();
    track.meta(0, 0x58, &[4, 2, 24, 8]); // 4/4
    track.meta(0, 0x51, &[a, b, c]);
    track.meta(0, 0x06, if sff2 { b"SFF2" } else { b"SFF1" });
    track.meta(0, 0x03, name.as_bytes()); // sequence name: the style's name
    track.meta(0, 0x06, b"SInt");
    track.sysex(0, &[0x7E, 0x7F, 0x09, 0x01, 0xF7]); // General MIDI on
    for channel in [SUB_RHYTHM, RHYTHM] {
        track.event(0, &[0xB0 | channel, 0, 127]); // bank MSB
        track.event(0, &[0xB0 | channel, 32, 0]); // bank LSB
        track.event(0, &[0xC0 | channel, 0]); // standard kit
        track.event(0, &[0xB0 | channel, 7, 100]); // volume
        track.event(0, &[0xB0 | channel, 10, 64]); // pan
        track.event(0, &[0xB0 | channel, 91, 40]); // reverb
        track.event(0, &[0xB0 | channel, 93, 0]); // chorus
    }

    // The sections, from bar 2.
    let mut start = TICKS_PER_BAR;
    for (marker, bars, hits) in [("Main A", 2, main_a()), ("Fill In AA", 1, fill_in_aa())] {
        track.meta(start, 0x06, marker.as_bytes());
        for (step, key, velocity) in hits {
            let tick = start + step * TICKS_PER_STEP;
            track.note(tick, RHYTHM, key, velocity);
        }
        start += bars * TICKS_PER_BAR;
    }
    track.end(start)
}

/// Two bars of a plain rock beat.
fn main_a() -> Vec<Hit> {
    let mut hits = Vec::new();
    for bar in 0..2 {
        let at = bar * STEPS_PER_BAR;
        for step in (0..STEPS_PER_BAR).step_by(2) {
            let accent = if step % 4 == 0 { 100 } else { 70 };
            let open = bar == 1 && step == 14;
            hits.push((at + step, if open { OPEN_HAT } else { CLOSED_HAT }, accent));
        }
        for step in [0, 8, 10] {
            hits.push((at + step, KICK, 110));
        }
        for step in [4, 12] {
            hits.push((at + step, SNARE, 110));
        }
    }
    hits
}

/// One bar: half a beat, then snare and toms down the kit.
fn fill_in_aa() -> Vec<Hit> {
    let mut hits = vec![
        (0, CLOSED_HAT, 100),
        (2, CLOSED_HAT, 70),
        (4, CLOSED_HAT, 100),
        (6, CLOSED_HAT, 70),
        (0, KICK, 110),
        (4, SNARE, 110),
    ];
    for step in 8..12 {
        hits.push((step, SNARE, 80 + (step as u8 - 8) * 10));
    }
    for (step, key) in [
        (12, HIGH_TOM),
        (13, MID_TOM),
        (14, LOW_TOM),
        (15, FLOOR_TOM),
    ] {
        hits.push((step, key, 110));
    }
    hits.push((12, KICK, 110));
    hits
}

/// Events collected with their ticks, written in tick order.
#[derive(Default)]
struct Track {
    /// Tick, order among events at the same tick (insertion order), bytes.
    events: Vec<(u32, usize, Vec<u8>)>,
}

impl Track {
    fn push(&mut self, tick: u32, bytes: Vec<u8>) {
        let order = self.events.len();
        self.events.push((tick, order, bytes));
    }

    fn event(&mut self, tick: u32, bytes: &[u8]) {
        self.push(tick, bytes.to_vec());
    }

    fn meta(&mut self, tick: u32, kind: u8, data: &[u8]) {
        let mut bytes = vec![0xFF, kind];
        write_vlq(&mut bytes, data.len() as u32);
        bytes.extend_from_slice(data);
        self.push(tick, bytes);
    }

    /// A system exclusive message; `data` follows the `F0` and ends with `F7`.
    fn sysex(&mut self, tick: u32, data: &[u8]) {
        let mut bytes = vec![0xF0];
        write_vlq(&mut bytes, data.len() as u32);
        bytes.extend_from_slice(data);
        self.push(tick, bytes);
    }

    fn note(&mut self, tick: u32, channel: u8, key: u8, velocity: u8) {
        self.push(tick, vec![0x90 | channel, key, velocity]);
        // The note off, as a note on with velocity 0, half a step later: before the next hit.
        self.push(tick + NOTE_TICKS, vec![0x90 | channel, key, 0]);
    }

    /// The file, with the end of track at `end`.
    fn end(mut self, end: u32) -> Vec<u8> {
        self.events.sort_by_key(|&(tick, order, _)| (tick, order));
        let mut track = Vec::new();
        let mut previous = 0;
        for (tick, _, bytes) in &self.events {
            write_vlq(&mut track, tick - previous);
            track.extend_from_slice(bytes);
            previous = *tick;
        }
        write_vlq(&mut track, end.saturating_sub(previous));
        track.extend_from_slice(&[0xFF, 0x2F, 0x00]);

        let mut file = Vec::new();
        file.extend_from_slice(b"MThd");
        file.extend_from_slice(&6u32.to_be_bytes());
        file.extend_from_slice(&0u16.to_be_bytes()); // format 0
        file.extend_from_slice(&1u16.to_be_bytes()); // one track
        file.extend_from_slice(&(TICKS_PER_BEAT as u16).to_be_bytes());
        file.extend_from_slice(b"MTrk");
        file.extend_from_slice(&(track.len() as u32).to_be_bytes());
        file.extend_from_slice(&track);
        file
    }
}

/// Seven bits per byte, most significant first, the top bit set on every byte but the last.
fn write_vlq(out: &mut Vec<u8>, value: u32) {
    let mut groups = vec![(value & 0x7F) as u8];
    let mut rest = value >> 7;
    while rest > 0 {
        groups.push((rest & 0x7F) as u8 | 0x80);
        rest >>= 7;
    }
    out.extend(groups.iter().rev());
}
