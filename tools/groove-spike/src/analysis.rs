//! How a MIDI file fits the app's grid: timing, swing, keys, bars and repetition.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::keymap::{self, KeyMap};
use crate::read::MidiFile;

const STEPS_PER_BAR: usize = 16;

/// Half time: one step holds an 8th note instead of a 16th, and the tempo halves, so the app's
/// swing (on odd steps) becomes 8th-note swing. One bar of the file fills half a bar.
pub static HALF_TIME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn half_time() -> bool {
    HALF_TIME.load(std::sync::atomic::Ordering::Relaxed)
}
/// A note further than this from its grid point, in steps, is "off the grid": a triplet, a flam,
/// a roll or a badly late note. A quarter step is 31 ms at 120 BPM.
const FAR: f64 = 0.25;
/// A note closer than this, in steps, is "tight": a tenth of a step is 12.5 ms at 120 BPM.
const TIGHT: f64 = 0.10;

/// One hit on the grid: app key, step within the bar, velocity.
pub type Hit = (u8, usize, u8);

pub struct Analysis {
    pub path: PathBuf,
    pub format: u16,
    pub ppq: u32,
    pub bpm: f64,
    /// Whether the file had a tempo event at all.
    pub has_tempo: bool,
    pub tempo_changes: usize,
    /// The first time signature, or `None` if the file has none (MIDI then means 4/4).
    pub time_signature: Option<(u8, u8)>,
    pub time_signature_changes: usize,
    pub map: KeyMap,
    pub notes: usize,
    pub channels: BTreeSet<u8>,
    /// Notes used: on channel 10, or on the only channel the file uses.
    pub drum_notes: usize,
    /// The file's own drum keys before mapping, with counts.
    pub raw_keys: BTreeMap<u8, usize>,
    /// Notes whose key the app has no sound for, by key.
    pub dropped_keys: BTreeMap<u8, usize>,
    pub gs_extra_notes: usize,
    /// Mean distance from the nearest 16th, in steps, with no swing.
    pub err_straight: f64,
    /// The swing that fits best, as the app defines it: the fraction of a step every odd step is
    /// late, 0 to 0.5.
    pub swing: f64,
    /// The constant shift, in steps, that fits best: positive when the whole file plays late.
    pub offset: f64,
    pub err_swing: f64,
    /// Mean distance from the nearest 8th-note triplet.
    pub err_triplet8: f64,
    /// Mean distance from the nearest 16th-note triplet.
    pub err_triplet16: f64,
    /// Shares of notes within [`TIGHT`] and beyond [`FAR`] of their grid point, after swing.
    pub tight: f64,
    pub far: f64,
    /// Timing removed by snapping, in milliseconds: mean and 95th percentile.
    pub moved_ms_mean: f64,
    pub moved_ms_p95: f64,
    /// Notes lost because another note of the same drum landed on the same step.
    pub collisions: usize,
    /// Notes that snapped past the last bar and were wrapped to step 0.
    pub wrapped: usize,
    pub bars: usize,
    /// The step of the first note after snapping: how much silence the file starts with.
    pub first_step: usize,
    pub grid: Vec<Vec<Hit>>,
    pub distinct_bars: usize,
    /// Bars left after merging bars that differ in at most two hits.
    pub near_distinct_bars: usize,
    pub distinct_velocities: usize,
    /// Notes by the app's three loudness levels: soft, normal, loud.
    pub levels: [usize; 3],
}

pub fn analyze(path: &Path, file: &MidiFile, map: KeyMap) -> Analysis {
    let channels: BTreeSet<u8> = file.notes.iter().map(|n| n.channel).collect();
    let drum_channel = if channels.contains(&9) || channels.len() != 1 {
        9
    } else {
        *channels.first().unwrap()
    };
    let drums: Vec<_> = file
        .notes
        .iter()
        .filter(|n| n.channel == drum_channel)
        .copied()
        .collect();
    let map = map.resolve(drums.iter().map(|n| n.key));
    let mut raw_keys = BTreeMap::new();
    for n in &drums {
        *raw_keys.entry(n.key).or_insert(0) += 1;
    }

    let scale = if half_time() { 2.0 } else { 1.0 };
    let bpm = file.tempos.first().map_or(120.0, |t| t.1) / scale;
    let tempo_changes = file
        .tempos
        .windows(2)
        .filter(|w| (w[0].1 - w[1].1).abs() > 0.01)
        .count();
    let time_signature = file.time_signatures.first().map(|t| (t.1, t.2));
    let time_signature_changes = file
        .time_signatures
        .windows(2)
        .filter(|w| (w[0].1, w[0].2) != (w[1].1, w[1].2))
        .count();

    let step_ticks = f64::from(file.ppq) / 4.0 * scale;
    let mut dropped_keys = BTreeMap::new();
    let mut gs_extra_notes = 0;
    // (position in steps, app key, velocity)
    let mut placed: Vec<(f64, u8, u8)> = Vec::new();
    for n in &drums {
        match map.map(n.key) {
            Some(k) => {
                if keymap::is_gs_extra(k) {
                    gs_extra_notes += 1;
                }
                placed.push((n.tick as f64 / step_ticks, k, n.velocity));
            }
            None => *dropped_keys.entry(n.key).or_insert(0) += 1,
        }
    }

    let raw: Vec<f64> = placed.iter().map(|p| p.0).collect();
    let err_straight = mean(raw.iter().map(|&x| swing_error(x, 0.0)));
    // Search swing and a constant offset together. A recording that starts a little early or
    // late, or a kit with latency, shifts every note by the same amount.
    let (mut swing, mut offset, mut err_swing) = (0.0, 0.0, err_straight);
    for o in -12..=12 {
        let o = f64::from(o) / 50.0;
        for i in 0..=50 {
            let s = f64::from(i) / 100.0;
            let e = mean(raw.iter().map(|&x| swing_error(x - o, s)));
            if e < err_swing - 1e-9 {
                (swing, offset, err_swing) = (s, o, e);
            }
        }
    }
    // A fit that barely helps is noise in a straight, on-time groove.
    if err_straight - err_swing < 0.01 {
        (swing, offset, err_swing) = (0.0, 0.0, err_straight);
    }
    let positions: Vec<f64> = raw.iter().map(|&x| (x - offset).max(0.0)).collect();
    let err_triplet8 = mean(positions.iter().map(|&x| grid_error(x, 4.0 / 3.0)));
    let err_triplet16 = mean(positions.iter().map(|&x| grid_error(x, 2.0 / 3.0)));

    let total_steps = positions
        .iter()
        .copied()
        .fold(file.end_tick as f64 / step_ticks, f64::max);
    let bars = (((total_steps - 0.5) / STEPS_PER_BAR as f64).ceil() as usize).max(1);

    let step_ms = 15_000.0 / bpm;
    let mut moved = Vec::new();
    let (mut tight, mut far, mut wrapped) = (0usize, 0usize, 0usize);
    let mut cells: BTreeMap<(usize, u8, usize), (u8, usize)> = BTreeMap::new();
    for (&(_, key, velocity), &x) in placed.iter().zip(&positions) {
        let (q, err) = snap(x, swing);
        moved.push(err * step_ms);
        if err <= TIGHT {
            tight += 1;
        }
        if err > FAR {
            far += 1;
        }
        let mut q = q;
        if q >= bars * STEPS_PER_BAR {
            q %= bars * STEPS_PER_BAR;
            wrapped += 1;
        }
        let cell = cells
            .entry((q / STEPS_PER_BAR, key, q % STEPS_PER_BAR))
            .or_insert((0, 0));
        cell.0 = cell.0.max(velocity);
        cell.1 += 1;
    }
    let first_step = cells
        .keys()
        .map(|&(bar, _, step)| bar * STEPS_PER_BAR + step)
        .min()
        .unwrap_or(0);
    let collisions = cells.values().map(|c| c.1 - 1).sum();
    let mut grid: Vec<Vec<Hit>> = vec![Vec::new(); bars];
    for (&(bar, key, step), &(velocity, _)) in &cells {
        grid[bar].push((key, step, velocity));
    }
    moved.sort_by(f64::total_cmp);

    let shapes: Vec<BTreeSet<(u8, usize)>> = grid
        .iter()
        .map(|b| b.iter().map(|&(k, s, _)| (k, s)).collect())
        .collect();
    let distinct_bars = shapes.iter().collect::<BTreeSet<_>>().len();
    let mut clusters: Vec<&BTreeSet<(u8, usize)>> = Vec::new();
    for s in &shapes {
        if !clusters
            .iter()
            .any(|c| c.symmetric_difference(s).count() <= 2)
        {
            clusters.push(s);
        }
    }

    let mut levels = [0; 3];
    let mut velocities = BTreeSet::new();
    for &(_, _, v) in &placed {
        velocities.insert(v);
        levels[match v {
            0..80 => 0,
            80..114 => 1,
            _ => 2,
        }] += 1;
    }
    let n = placed.len().max(1) as f64;
    Analysis {
        path: path.to_owned(),
        format: file.format,
        ppq: file.ppq,
        bpm,
        has_tempo: !file.tempos.is_empty(),
        tempo_changes,
        time_signature,
        time_signature_changes,
        map,
        notes: file.notes.len(),
        channels,
        drum_notes: drums.len(),
        raw_keys,
        dropped_keys,
        gs_extra_notes,
        err_straight,
        swing,
        offset,
        err_swing,
        err_triplet8,
        err_triplet16,
        tight: tight as f64 / n,
        far: far as f64 / n,
        moved_ms_mean: mean(moved.iter().copied()),
        moved_ms_p95: percentile(&moved, 0.95),
        collisions,
        wrapped,
        bars,
        first_step,
        grid,
        distinct_bars,
        near_distinct_bars: clusters.len(),
        distinct_velocities: velocities.len(),
        levels,
    }
}

/// Distance in steps from `x` to the nearest point of the swung 16th grid: in each pair of
/// steps, points at 0 and `1 + swing`.
fn swing_error(x: f64, swing: f64) -> f64 {
    snap(x, swing).1
}

/// The step a position snaps to on the swung grid, and the distance moved.
fn snap(x: f64, swing: f64) -> (usize, f64) {
    let pair = (x / 2.0).floor();
    let r = x - pair * 2.0;
    let candidates = [(0usize, 0.0), (1, 1.0 + swing), (2, 2.0)];
    let (offset, err) = candidates
        .iter()
        .map(|&(o, p)| (o, (r - p).abs()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    ((pair as usize) * 2 + offset, err)
}

fn grid_error(x: f64, spacing: f64) -> f64 {
    let r = x / spacing;
    (r - r.round()).abs() * spacing
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0usize), |(s, c), v| (s + v, c + 1));
    if count == 0 { 0.0 } else { sum / count as f64 }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

/// How cleanly a file imports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// Everything lands on the grid and every drum has a row.
    Clean,
    /// A few notes moved far or were dropped, but the groove survives.
    Usable,
    /// The groove is not a 16th-note 4/4 groove, or much of it is lost.
    Poor,
}

impl Analysis {
    pub fn non_four_four(&self) -> bool {
        self.time_signature.is_some_and(|ts| ts != (4, 4)) || self.time_signature_changes > 0
    }

    /// Whether 8th-note triplets explain the timing better than swung 16ths do.
    pub fn triplet_feel(&self) -> bool {
        self.err_triplet8 < self.err_swing * 0.75
    }

    pub fn dropped(&self) -> usize {
        self.dropped_keys.values().sum()
    }

    pub fn verdict(&self) -> Verdict {
        let used = self.drum_notes.max(1) as f64;
        let lost = (self.dropped() + self.collisions) as f64 / used;
        if self.drum_notes == 0 || self.non_four_four() || self.far > 0.10 || lost > 0.10 {
            Verdict::Poor
        } else if self.far > 0.02 || lost > 0.02 || self.tempo_changes > 0 || self.triplet_feel() {
            Verdict::Usable
        } else {
            Verdict::Clean
        }
    }

    pub const CSV_HEADER: &str = "file,verdict,format,ppq,bpm,tempo_changes,time_sig,ts_changes,map,channels,notes,drum_notes,dropped,gs_extra,err_straight,swing,offset,err_swing,err_trip8,err_trip16,tight,far,moved_ms_mean,moved_ms_p95,collisions,wrapped,bars,first_step,distinct_bars,near_distinct_bars,distinct_velocities,soft,normal,loud";

    pub fn csv_row(&self) -> String {
        let ts = self
            .time_signature
            .map_or("none".to_owned(), |(n, d)| format!("{n}/{d}"));
        let channels: Vec<String> = self.channels.iter().map(|c| (c + 1).to_string()).collect();
        format!(
            "\"{}\",{:?},{},{},{:.2},{},{},{},{},{},{},{},{},{},{:.4},{:.2},{:.2},{:.4},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2},{},{},{},{},{},{},{},{},{},{}",
            self.path.display(),
            self.verdict(),
            self.format,
            self.ppq,
            self.bpm,
            self.tempo_changes,
            ts,
            self.time_signature_changes,
            self.map.name(),
            channels.join(" "),
            self.notes,
            self.drum_notes,
            self.dropped(),
            self.gs_extra_notes,
            self.err_straight,
            self.swing,
            self.offset,
            self.err_swing,
            self.err_triplet8,
            self.err_triplet16,
            self.tight,
            self.far,
            self.moved_ms_mean,
            self.moved_ms_p95,
            self.collisions,
            self.wrapped,
            self.bars,
            self.first_step,
            self.distinct_bars,
            self.near_distinct_bars,
            self.distinct_velocities,
            self.levels[0],
            self.levels[1],
            self.levels[2],
        )
    }

    pub fn report(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", self.path.display());
        let _ = writeln!(
            s,
            "  verdict {:?}; format {}, {} ppq, {:.1} BPM{}, {} tempo changes, time signature {}",
            self.verdict(),
            self.format,
            self.ppq,
            self.bpm,
            if self.has_tempo { "" } else { " (none given)" },
            self.tempo_changes,
            self.time_signature
                .map_or("none (4/4)".to_owned(), |(n, d)| format!("{n}/{d}"))
        );
        let _ = writeln!(
            s,
            "  {} notes, {} on the drum channel; key map {}; dropped {:?}; GS extras {}",
            self.notes,
            self.drum_notes,
            self.map.name(),
            self.dropped_keys,
            self.gs_extra_notes
        );
        let _ = writeln!(
            s,
            "  grid error (steps): straight {:.3}, swing {:.2} offset {:+.2} -> {:.3}, 8th triplets {:.3}, 16th triplets {:.3}",
            self.err_straight,
            self.swing,
            self.offset,
            self.err_swing,
            self.err_triplet8,
            self.err_triplet16
        );
        let _ = writeln!(
            s,
            "  {:.1}% tight, {:.1}% far; moved {:.1} ms on average, {:.1} ms at p95; {} collisions, {} wrapped",
            self.tight * 100.0,
            self.far * 100.0,
            self.moved_ms_mean,
            self.moved_ms_p95,
            self.collisions,
            self.wrapped
        );
        let _ = write!(
            s,
            "  {} bars, {} distinct, {} after merging near-duplicates; {} distinct velocities, levels {:?}",
            self.bars,
            self.distinct_bars,
            self.near_distinct_bars,
            self.distinct_velocities,
            self.levels
        );
        s
    }
}

/// Prints totals over many files, grouped by their top folder.
pub fn print_summary(results: &[Analysis], failed: &[(PathBuf, String)]) {
    let n = results.len();
    println!("{n} files read, {} failed", failed.len());
    for (path, why) in failed.iter().take(10) {
        println!("  failed: {}: {why}", path.display());
    }
    if n == 0 {
        return;
    }
    let count = |f: &dyn Fn(&Analysis) -> bool| results.iter().filter(|a| f(a)).count();
    let pct = |c: usize| 100.0 * c as f64 / n as f64;
    for v in [Verdict::Clean, Verdict::Usable, Verdict::Poor] {
        let c = count(&|a| a.verdict() == v);
        println!("  {v:?}: {c} ({:.1}%)", pct(c));
    }
    let c = count(&|a| a.non_four_four());
    println!("  not 4/4: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.time_signature.is_none());
    println!("  no time signature: {c} ({:.1}%)", pct(c));
    let c = count(&|a| !a.has_tempo);
    println!("  no tempo: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.tempo_changes > 0);
    println!("  tempo changes: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.triplet_feel());
    println!("  triplet feel: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.swing > 0.0);
    println!("  swung (swing > 0): {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.dropped() > 0);
    println!("  with dropped keys: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.gs_extra_notes > 0);
    println!("  with GS extra keys: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.first_step >= 4);
    println!("  first note a beat or more late: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.first_step >= 16);
    println!("  first bar empty: {c} ({:.1}%)", pct(c));
    let c = count(&|a| a.collisions > 0);
    println!("  with collisions: {c} ({:.1}%)", pct(c));
    let c = count(&|a| !a.channels.contains(&9));
    println!("  drums not on channel 10: {c} ({:.1}%)", pct(c));

    let mut far: Vec<f64> = results.iter().map(|a| a.far).collect();
    far.sort_by(f64::total_cmp);
    let mut moved: Vec<f64> = results.iter().map(|a| a.moved_ms_mean).collect();
    moved.sort_by(f64::total_cmp);
    let mut tight: Vec<f64> = results.iter().map(|a| a.tight).collect();
    tight.sort_by(f64::total_cmp);
    println!(
        "  far notes per file: median {:.1}%, p90 {:.1}%, max {:.1}%",
        percentile(&far, 0.5) * 100.0,
        percentile(&far, 0.9) * 100.0,
        percentile(&far, 1.0) * 100.0
    );
    println!(
        "  tight notes per file: median {:.1}%, p10 {:.1}%",
        percentile(&tight, 0.5) * 100.0,
        percentile(&tight, 0.1) * 100.0
    );
    println!(
        "  mean timing moved per file: median {:.1} ms, p90 {:.1} ms",
        percentile(&moved, 0.5),
        percentile(&moved, 0.9)
    );

    let mut swing_hist = BTreeMap::new();
    for a in results {
        *swing_hist
            .entry((a.swing * 10.0).floor() as u32)
            .or_insert(0) += 1;
    }
    let swing_text: Vec<String> = swing_hist
        .iter()
        .map(|(k, v)| format!("{:.1}-{:.1}: {v}", *k as f64 / 10.0, (*k + 1) as f64 / 10.0))
        .collect();
    println!("  swing: {}", swing_text.join(", "));

    let buckets = [
        (1, 1),
        (2, 2),
        (3, 4),
        (5, 8),
        (9, 16),
        (17, 64),
        (65, usize::MAX),
    ];
    let bar_text: Vec<String> = buckets
        .iter()
        .map(|&(lo, hi)| {
            let c = count(&|a| (lo..=hi).contains(&a.bars));
            let label = if hi == usize::MAX {
                format!("{lo}+")
            } else if lo == hi {
                lo.to_string()
            } else {
                format!("{lo}-{hi}")
            };
            format!("{label}: {c}")
        })
        .collect();
    println!("  bars: {}", bar_text.join(", "));

    let mut keys: BTreeMap<u8, usize> = BTreeMap::new();
    for a in results {
        for (k, c) in &a.dropped_keys {
            *keys.entry(*k).or_insert(0) += c;
        }
    }
    println!("  dropped keys over all files: {keys:?}");
    let mut raw: BTreeMap<u8, usize> = BTreeMap::new();
    for a in results {
        for (k, c) in &a.raw_keys {
            *raw.entry(*k).or_insert(0) += c;
        }
    }
    let mut raw: Vec<_> = raw.into_iter().collect();
    raw.sort_by_key(|e| std::cmp::Reverse(e.1));
    let raw_text: Vec<String> = raw
        .iter()
        .take(16)
        .map(|(k, c)| format!("{k} {} x{c}", dm_project::drum_name(*k)))
        .collect();
    println!("  most used file keys: {}", raw_text.join(", "));
    let mut maps: BTreeMap<&str, usize> = BTreeMap::new();
    for a in results {
        *maps.entry(a.map.name()).or_insert(0) += 1;
    }
    println!("  key maps: {maps:?}");
    let mut ppqs: BTreeMap<u32, usize> = BTreeMap::new();
    for a in results {
        *ppqs.entry(a.ppq).or_insert(0) += 1;
    }
    println!("  ppq: {ppqs:?}");
}
