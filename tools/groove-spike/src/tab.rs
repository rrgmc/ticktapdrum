//! Parses plain-text (ASCII) drum tabs into bars of hits on the 16-step grid.
//!
//! A tab is a text with "systems": runs of lines that each start with a drum label and hold a row
//! of hit symbols split into bars by `|`, all aligned under each other:
//!
//! ```text
//! C |x---------------|----------------|
//! HH|--x-x-x-x-x-x-x-|x-x-x-x-x-x-x-x-|
//! S |----o-------o---|----o-------o---|
//! B |o-------o-o-----|o-------o-o-----|  x4
//! ```
//!
//! The parser is forgiving. Lyrics, guitar tabs, legends and comments between systems are skipped.
//! Each bar's width decides its grid: 16 columns are 16ths, 8 are 8ths, 4 quarters, 32 are 32nds,
//! 12 and 24 are triplets. Other widths are scaled onto 16 steps and counted as odd.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::analysis::Hit;

const STEPS: usize = 16;
const NORMAL: u8 = 100;
const ACCENT: u8 = 127;
const GHOST: u8 = 45;
/// The most times a repeat mark may repeat a system. A larger number is more likely a typo.
const MAX_REPEAT: usize = 16;

#[derive(Debug, Default)]
pub struct TabReport {
    pub path: PathBuf,
    pub systems: usize,
    pub drum_lines: usize,
    /// Systems whose lines do not have the same number of bars.
    pub misaligned: usize,
    /// Bars read, before repeats.
    pub bars_read: usize,
    /// Bar widths in columns, with counts.
    pub widths: BTreeMap<usize, usize>,
    pub hits: usize,
    /// Hits moved by more than a sixth of a step: triplets and odd widths.
    pub moved: usize,
    /// Hits merged into another on the same step and drum: 32nd notes, mostly.
    pub merged: usize,
    /// Hits with a flam, drag or roll symbol, played as one hit.
    pub ornaments: usize,
    pub ghosts: usize,
    pub accents: usize,
    pub unknown_labels: BTreeMap<String, usize>,
    pub unknown_symbols: BTreeMap<char, usize>,
    /// Lines that count beats, such as `1 e + a 2 e + a`.
    pub count_lines: usize,
    pub repeats: usize,
    pub tempo: Option<f64>,
    /// Bars after repeats.
    pub grid: Vec<Vec<Hit>>,
}

/// How a drum line's row reads: its label and the bars of symbols.
struct DrumLine {
    label: String,
    key: Option<u8>,
    bars: Vec<String>,
    repeat: Option<usize>,
}

pub fn parse(path: &Path, text: &str) -> TabReport {
    let mut report = TabReport {
        path: path.to_owned(),
        tempo: find_tempo(text),
        ..Default::default()
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if is_count_line(lines[i]) {
            report.count_lines += 1;
            i += 1;
            continue;
        }
        let Some(first) = drum_line(lines[i]) else {
            i += 1;
            continue;
        };
        let mut system = vec![first];
        i += 1;
        while i < lines.len() {
            if is_count_line(lines[i]) {
                report.count_lines += 1;
                i += 1;
                continue;
            }
            match drum_line(lines[i]) {
                Some(line) => {
                    system.push(line);
                    i += 1;
                }
                None => break,
            }
        }
        // A repeat mark may also sit alone on the line after the system.
        let after = lines
            .get(i)
            .and_then(|l| repeat_mark(l).filter(|_| l.len() < 30));
        add_system(&mut report, &system, after);
    }
    report
}

fn add_system(report: &mut TabReport, system: &[DrumLine], after: Option<usize>) {
    report.systems += 1;
    report.drum_lines += system.len();
    let bars = system.iter().map(|l| l.bars.len()).max().unwrap_or(0);
    if system.iter().any(|l| l.bars.len() != bars) {
        report.misaligned += 1;
    }
    report.bars_read += bars;
    let repeat = system
        .iter()
        .filter_map(|l| l.repeat)
        .chain(after)
        .max()
        .unwrap_or(1)
        .clamp(1, MAX_REPEAT);
    if repeat > 1 {
        report.repeats += 1;
    }

    let mut parsed: Vec<Vec<Hit>> = vec![Vec::new(); bars];
    for line in system {
        match line.key {
            None => {
                *report.unknown_labels.entry(line.label.clone()).or_insert(0) += 1;
                continue;
            }
            Some(key) => {
                for (bar, symbols) in line.bars.iter().enumerate() {
                    read_bar(report, &line.label, key, symbols, &mut parsed[bar]);
                }
            }
        }
    }
    for bar in system.first().map_or(&[][..], |l| &l.bars[..]) {
        *report.widths.entry(bar.chars().count()).or_insert(0) += 1;
    }
    for bar in &mut parsed {
        let before = bar.len();
        bar.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(b.2.cmp(&a.2)));
        bar.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
        report.merged += before - bar.len();
    }
    for _ in 0..repeat {
        report.grid.extend(parsed.iter().cloned());
    }
}

/// Reads one bar of one drum line into hits.
fn read_bar(report: &mut TabReport, label: &str, key: u8, symbols: &str, out: &mut Vec<Hit>) {
    let chars: Vec<char> = symbols.chars().collect();
    let width = chars.len();
    if width == 0 {
        return;
    }
    let hat = key == 42;
    let ride = key == 51;
    let mut in_parens = false;
    for (col, &c) in chars.iter().enumerate() {
        let velocity;
        let mut key = key;
        match c {
            '-' | '.' | ' ' | '_' | '=' | '~' => continue,
            '(' | '[' => {
                in_parens = true;
                continue;
            }
            ')' | ']' => {
                in_parens = false;
                continue;
            }
            'x' | 'o' | '*' | '0' | '@' | '#' | '+' | 'f' | 'd' | 'r' | 'g' | 'b' | 'X' | 'O'
            | 'F' | 'D' | 'R' | 'G' | 'B' | 'c' => {
                velocity = match c {
                    'X' | 'O' | '@' => {
                        report.accents += 1;
                        ACCENT
                    }
                    'g' | 'G' => {
                        report.ghosts += 1;
                        GHOST
                    }
                    _ if in_parens => {
                        report.ghosts += 1;
                        GHOST
                    }
                    _ => NORMAL,
                };
                if matches!(c, 'f' | 'F' | 'd' | 'D' | 'r' | 'R') {
                    report.ornaments += 1;
                }
                // On a hi-hat line, o means open. On a ride line, b means the bell.
                if hat && matches!(c, 'o' | 'O') {
                    key = 46;
                }
                if ride && matches!(c, 'b' | 'B') {
                    key = 53;
                }
                if c == '+' && !hat {
                    *report.unknown_symbols.entry(c).or_insert(0) += 1;
                    continue;
                }
                let _ = label;
            }
            other => {
                *report.unknown_symbols.entry(other).or_insert(0) += 1;
                continue;
            }
        }
        report.hits += 1;
        let exact = col as f64 * STEPS as f64 / width as f64;
        let step = exact.round() as usize;
        if (exact - step as f64).abs() > 1.0 / 6.0 {
            report.moved += 1;
        }
        if step >= STEPS {
            // A hit snapped onto the next downbeat: keep it in this bar's last step.
            out.push((key, STEPS - 1, velocity));
        } else {
            out.push((key, step, velocity));
        }
    }
}

/// Recognises a drum line: a label, then a row of hit symbols with at least one bar line.
fn drum_line(line: &str) -> Option<DrumLine> {
    let line = line.trim_end();
    let trimmed = line.trim_start();
    // The label runs up to the first `|` or `:` (or space, if neither comes soon).
    let split = trimmed.find(['|', ':'])?;
    let label: String = trimmed[..split].trim().to_owned();
    if label.is_empty()
        || label.chars().count() > 8
        || !label.chars().next()?.is_ascii_alphabetic()
        || !label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " .-/'".contains(c))
    {
        return None;
    }
    let rest = &trimmed[split..];
    let last_bar = rest.rfind('|')?;
    let body = &rest[..=last_bar];
    let tail = &rest[last_bar + 1..];
    // The body must look like hits and rests, not words.
    let symbols: Vec<char> = body.chars().filter(|c| !"|: ".contains(*c)).collect();
    if symbols.len() < 4 {
        return None;
    }
    let rests = symbols.iter().filter(|c| matches!(c, '-' | '.')).count();
    let letters = symbols
        .iter()
        .filter(|c| c.is_ascii_alphabetic() && !"xXoOgGfFdDrRbBc".contains(**c))
        .count();
    if rests * 3 < symbols.len() || letters * 10 > symbols.len() {
        return None;
    }
    let bars: Vec<String> = body
        .split('|')
        .map(|b| b.trim_matches(':').trim().to_owned())
        .filter(|b| !b.is_empty())
        .collect();
    if bars.is_empty() {
        return None;
    }
    Some(DrumLine {
        key: label_key(&label),
        label,
        bars,
        repeat: repeat_mark(tail),
    })
}

/// A repeat mark such as `x4`, `4x`, `(x2)` or `repeat 3 times`.
fn repeat_mark(text: &str) -> Option<usize> {
    let t = text.to_ascii_lowercase();
    let t = t.trim();
    let bytes = t.as_bytes();
    for (i, w) in bytes.windows(2).enumerate() {
        // "x4" or "x 4"
        if w[0] == b'x' {
            let rest = t[i + 1..].trim_start();
            let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = n.parse::<usize>() {
                return Some(n);
            }
        }
        // "4x"
        if w[0].is_ascii_digit() && w[1] == b'x' {
            let start = t[..=i]
                .rfind(|c: char| !c.is_ascii_digit())
                .map_or(0, |p| p + 1);
            if let Ok(n) = t[start..=i].parse::<usize>() {
                return Some(n);
            }
        }
    }
    if let Some(p) = t.find("repeat") {
        let n: String = t[p..]
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        return n.parse().ok();
    }
    None
}

fn is_count_line(line: &str) -> bool {
    let t: String = line
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '|')
        .collect();
    t.len() >= 4
        && t.chars().any(|c| c.is_ascii_digit())
        && t.chars()
            .all(|c| "1234+&eaET.-".contains(c) || c.is_ascii_digit())
        && t.contains(['+', '&', 'e'])
}

fn find_tempo(text: &str) -> Option<f64> {
    let lower = text.to_ascii_lowercase();
    for (i, _) in lower.match_indices("bpm") {
        let before: String = lower[..i]
            .trim_end()
            .trim_end_matches(['=', ':', ' '])
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let n: String = before.chars().rev().collect();
        if let Ok(n) = n.parse::<f64>()
            && (40.0..=300.0).contains(&n)
        {
            return Some(n);
        }
        let after: String = lower[i + 3..]
            .trim_start_matches([' ', '=', ':'])
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(n) = after.parse::<f64>()
            && (40.0..=300.0).contains(&n)
        {
            return Some(n);
        }
    }
    for (i, _) in lower.match_indices("tempo") {
        let after: String = lower[i + 5..]
            .trim_start_matches([' ', '=', ':', '-'])
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(n) = after.parse::<f64>()
            && (40.0..=300.0).contains(&n)
        {
            return Some(n);
        }
    }
    None
}

/// The General MIDI key for a row label, or `None` if it is not a known drum.
pub fn label_key(label: &str) -> Option<u8> {
    let l: String = label
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    Some(match l.as_str() {
        "h" | "hh" | "hat" | "hihat" | "hhat" | "chh" | "hc" | "hho" | "hhc" => 42,
        "oh" | "ohh" | "ho" => 46,
        "hf" | "hhf" | "hp" | "ph" | "p" | "pd" | "ped" | "pedal" | "hfoot" | "fh" | "hhp" => 44,
        "s" | "sd" | "sn" | "snr" | "snare" | "sr" => 38,
        "b" | "bd" | "k" | "kd" | "kick" | "bass" | "bassdrum" | "b1" | "b2" | "bd1" | "bd2"
        | "k1" | "k2" | "kk" => 36,
        "c" | "cc" | "cr" | "crash" | "c1" | "cr1" | "cc1" | "cy" | "cym" => 49,
        "c2" | "cr2" | "cc2" => 57,
        "r" | "rd" | "ride" | "rc" | "ri" | "rcy" => 51,
        "rb" | "bell" => 53,
        "ch" | "cn" | "chn" | "china" => 52,
        "sp" | "spl" | "splash" => 55,
        "t" | "t1" | "ht" | "hi" | "tom" | "tom1" | "tt" | "rt" | "ht1" => 50,
        "t2" | "mt" | "m" | "tom2" | "ht2" => 47,
        "t3" | "lt" | "tom3" => 45,
        "ft" | "f" | "fl" | "t4" | "floor" | "ftom" | "lft" | "ft1" | "ft2" | "t5" => 43,
        "cb" | "cow" | "cowbell" => 56,
        "tb" | "tam" | "tamb" | "tambourine" => 54,
        "cl" | "clap" | "hc1" => 39,
        "ss" | "xs" | "rim" | "sst" | "cs" | "stick" => 37,
        _ => return None,
    })
}

/// How cleanly a tab parses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabVerdict {
    /// Every bar has a standard width, every label is known and the systems line up.
    Clean,
    /// Something was guessed or dropped, but most of the tab came through.
    Partial,
    /// Little or nothing came through.
    Failed,
}

impl TabReport {
    pub fn standard_bars(&self) -> usize {
        [4, 8, 16, 32]
            .iter()
            .map(|w| self.widths.get(w).copied().unwrap_or(0))
            .sum()
    }

    pub fn triplet_bars(&self) -> usize {
        [6, 12, 24]
            .iter()
            .map(|w| self.widths.get(w).copied().unwrap_or(0))
            .sum()
    }

    pub fn verdict(&self) -> TabVerdict {
        if self.systems == 0 || self.hits == 0 {
            return TabVerdict::Failed;
        }
        let bars = self.bars_read.max(1) as f64;
        let odd = (self.bars_read - self.standard_bars().min(self.bars_read)) as f64 / bars;
        let unknown: usize = self.unknown_labels.values().sum();
        if odd > 0.5 || unknown * 2 > self.drum_lines {
            TabVerdict::Failed
        } else if odd > 0.0
            || unknown > 0
            || self.misaligned > 0
            || !self.unknown_symbols.is_empty()
            || self.moved > 0
        {
            TabVerdict::Partial
        } else {
            TabVerdict::Clean
        }
    }

    pub const CSV_HEADER: &str = "file,verdict,systems,drum_lines,misaligned,bars_read,bars_after_repeats,standard_bars,triplet_bars,widths,hits,moved,merged,ornaments,ghosts,accents,unknown_labels,unknown_symbols,count_lines,repeats,tempo";

    pub fn csv_row(&self) -> String {
        let widths: Vec<String> = self
            .widths
            .iter()
            .map(|(w, c)| format!("{w}:{c}"))
            .collect();
        let labels: Vec<String> = self
            .unknown_labels
            .iter()
            .map(|(l, c)| format!("{l}:{c}"))
            .collect();
        let symbols: Vec<String> = self
            .unknown_symbols
            .iter()
            .map(|(s, c)| format!("{}:{c}", *s as u32))
            .collect();
        format!(
            "\"{}\",{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},\"{}\",{},{},{},{}",
            self.path.display(),
            self.verdict(),
            self.systems,
            self.drum_lines,
            self.misaligned,
            self.bars_read,
            self.grid.len(),
            self.standard_bars(),
            self.triplet_bars(),
            widths.join(" "),
            self.hits,
            self.moved,
            self.merged,
            self.ornaments,
            self.ghosts,
            self.accents,
            labels.join(" ").replace('"', "'"),
            symbols.join(" "),
            self.count_lines,
            self.repeats,
            self.tempo.map_or(String::new(), |t| t.to_string()),
        )
    }

    pub fn report(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", self.path.display());
        let _ = writeln!(
            s,
            "  verdict {:?}; {} systems, {} drum lines, {} misaligned; tempo {:?}",
            self.verdict(),
            self.systems,
            self.drum_lines,
            self.misaligned,
            self.tempo
        );
        let _ = writeln!(
            s,
            "  {} bars read, {} after {} repeats; widths {:?}",
            self.bars_read,
            self.grid.len(),
            self.repeats,
            self.widths
        );
        let _ = writeln!(
            s,
            "  {} hits: {} moved, {} merged, {} ornaments, {} ghosts, {} accents",
            self.hits, self.moved, self.merged, self.ornaments, self.ghosts, self.accents
        );
        let _ = write!(
            s,
            "  unknown labels {:?}; unknown symbols {:?}; {} count lines",
            self.unknown_labels, self.unknown_symbols, self.count_lines
        );
        s
    }
}

/// Prints totals over many tabs.
pub fn print_summary(results: &[TabReport], failed: &[(PathBuf, String)]) {
    let n = results.len();
    println!("{n} tabs read, {} unreadable", failed.len());
    if n == 0 {
        return;
    }
    let pct = |c: usize| 100.0 * c as f64 / n as f64;
    let count = |f: &dyn Fn(&TabReport) -> bool| results.iter().filter(|r| f(r)).count();
    for v in [TabVerdict::Clean, TabVerdict::Partial, TabVerdict::Failed] {
        let c = count(&|r| r.verdict() == v);
        println!("  {v:?}: {c} ({:.1}%)", pct(c));
    }
    let c = count(&|r| r.systems == 0);
    println!("  no system found: {c} ({:.1}%)", pct(c));
    let c = count(&|r| r.misaligned > 0);
    println!("  with misaligned systems: {c} ({:.1}%)", pct(c));
    let c = count(&|r| !r.unknown_labels.is_empty());
    println!("  with unknown labels: {c} ({:.1}%)", pct(c));
    let c = count(&|r| !r.unknown_symbols.is_empty());
    println!("  with unknown symbols: {c} ({:.1}%)", pct(c));
    let c = count(&|r| r.triplet_bars() > 0);
    println!("  with triplet-width bars: {c} ({:.1}%)", pct(c));
    let c = count(&|r| r.repeats > 0);
    println!("  with repeat marks used: {c} ({:.1}%)", pct(c));
    let c = count(&|r| r.tempo.is_some());
    println!("  with a tempo: {c} ({:.1}%)", pct(c));
    let c = count(&|r| r.count_lines > 0);
    println!("  with count lines: {c} ({:.1}%)", pct(c));

    let mut widths: BTreeMap<usize, usize> = BTreeMap::new();
    let mut labels: BTreeMap<String, usize> = BTreeMap::new();
    let mut symbols: BTreeMap<char, usize> = BTreeMap::new();
    let (mut bars, mut hits, mut moved, mut merged, mut orn) = (0, 0, 0, 0, 0);
    for r in results {
        for (w, c) in &r.widths {
            *widths.entry(*w).or_insert(0) += c;
        }
        for (l, c) in &r.unknown_labels {
            *labels.entry(l.clone()).or_insert(0) += c;
        }
        for (s, c) in &r.unknown_symbols {
            *symbols.entry(*s).or_insert(0) += c;
        }
        bars += r.bars_read;
        hits += r.hits;
        moved += r.moved;
        merged += r.merged;
        orn += r.ornaments;
    }
    let standard: usize = [4, 8, 16, 32]
        .iter()
        .map(|w| widths.get(w).copied().unwrap_or(0))
        .sum();
    println!(
        "  bars: {bars}, {standard} of standard width ({:.1}%)",
        100.0 * standard as f64 / bars.max(1) as f64
    );
    let mut w: Vec<_> = widths.into_iter().collect();
    w.sort_by_key(|e| std::cmp::Reverse(e.1));
    println!("  bar widths: {:?}", &w[..w.len().min(14)]);
    println!(
        "  hits: {hits}, moved {moved} ({:.1}%), merged {merged} ({:.1}%), ornaments {orn}",
        100.0 * moved as f64 / hits.max(1) as f64,
        100.0 * merged as f64 / hits.max(1) as f64
    );
    let mut l: Vec<_> = labels.into_iter().collect();
    l.sort_by_key(|e| std::cmp::Reverse(e.1));
    println!("  unknown labels: {:?}", &l[..l.len().min(25)]);
    let mut s: Vec<_> = symbols.into_iter().collect();
    s.sort_by_key(|e| std::cmp::Reverse(e.1));
    println!("  unknown symbols: {:?}", &s[..s.len().min(20)]);
}
