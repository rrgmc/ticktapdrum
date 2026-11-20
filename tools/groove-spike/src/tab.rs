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
//! 12 and 24 are triplets. A bar one column wider than that, with an empty first or last column
//! in every line, is a spacer column and is trimmed; a bar one column short is padded. Other widths
//! are scaled onto 16 steps and counted as odd.
//!
//! A legend in the tab, such as `BS = Bass drum`, overrides the built-in label table.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::analysis::Hit;

const STEPS: usize = 16;
const STANDARD: [usize; 4] = [4, 8, 16, 32];
const NORMAL: u8 = 100;
const ACCENT: u8 = 127;
const GHOST: u8 = 45;
/// The most times a repeat mark may repeat a system. A larger number is more likely a typo.
const MAX_REPEAT: usize = 16;
/// Letters that can be hit symbols, so a body holding them still looks like a drum line.
const HIT_LETTERS: &str = "xXoOgGfFdDrRbBcCsS";

#[derive(Debug, Default)]
pub struct TabReport {
    pub path: PathBuf,
    pub systems: usize,
    /// Systems skipped because they are guitar or bass tab (string names as labels).
    pub guitar_systems: usize,
    pub drum_lines: usize,
    /// Systems whose lines do not have the same number of bars.
    pub misaligned: usize,
    /// Bars read, before repeats.
    pub bars_read: usize,
    /// Bar widths in columns as written, with counts.
    pub raw_widths: BTreeMap<usize, usize>,
    /// Bar widths after trimming spacer columns and padding short bars.
    pub widths: BTreeMap<usize, usize>,
    /// Bars whose width was fixed by trimming or padding one column.
    pub fixed_bars: usize,
    /// Lines with no bar lines, cut into bars of 16.
    pub unpiped_lines: usize,
    /// Bars written as `%`, "same as the last bar".
    pub bar_repeats: usize,
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
    /// Labels the tab's own legend defined.
    pub legend_labels: usize,
    /// Drum lines whose label only the legend explained.
    pub legend_used: usize,
    /// Drum lines whose legend names a different drum family than the usual meaning of the label,
    /// as "label=legend/usual".
    pub legend_conflicts: Vec<String>,
    /// Lines that count beats, such as `1 e + a 2 e + a`.
    pub count_lines: usize,
    pub repeats: usize,
    pub tempo: Option<f64>,
    /// Bars with a snare hit, and those with one on beat 2 or 4: as written, and with a fixed width.
    pub snare_bars: [usize; 2],
    pub backbeat_bars: [usize; 2],
    /// Bars after repeats.
    pub grid: Vec<Vec<Hit>>,
}

/// How a drum line's row reads: its label and the bars of symbols.
struct DrumLine {
    label: String,
    bars: Vec<Vec<char>>,
    repeat: Option<usize>,
    piped: bool,
}

pub fn parse(path: &Path, text: &str) -> TabReport {
    let legend = parse_legend(text);
    let mut report = TabReport {
        path: path.to_owned(),
        tempo: find_tempo(text),
        legend_labels: legend.len(),
        ..Default::default()
    };
    let normalized: Vec<String> = text.lines().map(normalize).collect();
    let lines: Vec<&str> = normalized.iter().map(String::as_str).collect();
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
            .and_then(|l| repeat_mark(l).filter(|_| l.trim().len() < 30));
        add_system(&mut report, system, after, &legend);
    }
    report
}

/// Undoes web typography and other bar-line styles: `¦`, `│` and `I` between hits become `|`;
/// long dashes become `-`; an arrow `>` at the end of a bar, meaning "goes on", becomes a rest.
fn normalize(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let near_hits = |i: usize| {
        let symbol = |c: Option<&char>| c.is_some_and(|c| "-xXoO".contains(*c));
        i > 0
            && symbol(chars.get(i - 1))
            && (i + 1 == chars.len() || symbol(chars.get(i + 1)) || chars[i + 1] == ' ')
    };
    chars
        .iter()
        .enumerate()
        .map(|(i, &c)| match c {
            '¦' | '│' | '┃' => '|',
            'I' if near_hits(i) => '|',
            '—' | '–' | '─' => '-',
            '>' if chars.get(i + 1).is_none_or(|c| *c == '|' || *c == ' ') => '-',
            other => other,
        })
        .collect()
}

fn is_rest(c: char) -> bool {
    matches!(c, '-' | '.' | '_' | '=' | '~' | ' ')
}

fn add_system(
    report: &mut TabReport,
    mut system: Vec<DrumLine>,
    after: Option<usize>,
    legend: &BTreeMap<String, u8>,
) {
    let guitar = system
        .iter()
        .filter(|l| {
            matches!(
                l.label.as_str(),
                "e" | "E" | "A" | "a" | "D" | "d" | "G" | "g"
            )
        })
        .count();
    if guitar >= 3 {
        report.guitar_systems += 1;
        return;
    }
    report.systems += 1;
    report.drum_lines += system.len();

    // A line with no bar lines at all is cut into bars of 16.
    for line in &mut system {
        if !line.piped && line.bars.len() == 1 {
            let bar = &line.bars[0];
            if bar.len() > STEPS && bar.len() % STEPS == 0 {
                line.bars = bar.chunks(STEPS).map(<[char]>::to_vec).collect();
                report.unpiped_lines += 1;
            }
        }
    }

    let bars = system.iter().map(|l| l.bars.len()).max().unwrap_or(0);
    if system.iter().any(|l| l.bars.len() != bars) {
        report.misaligned += 1;
    }
    report.bars_read += bars;

    // `%` alone in a bar repeats the line's previous bar.
    for line in &mut system {
        for b in 1..line.bars.len() {
            let content: Vec<char> = line.bars[b]
                .iter()
                .copied()
                .filter(|c| !is_rest(*c))
                .collect();
            if content == ['%'] {
                line.bars[b] = line.bars[b - 1].clone();
                report.bar_repeats += 1;
            }
        }
    }

    // Fix widths one column off a standard width, bar by bar across the system.
    let mut fixed_bars: Vec<usize> = Vec::new();
    for b in 0..bars {
        let column: Vec<usize> = system
            .iter()
            .filter_map(|l| l.bars.get(b).map(Vec::len))
            .collect();
        let Some(&width) = column.first() else {
            continue;
        };
        *report.raw_widths.entry(width).or_insert(0) += 1;
        let same = column.iter().all(|&w| w == width);
        let mut fixed = width;
        if same && !STANDARD.contains(&width) {
            let all =
                |f: &dyn Fn(&Vec<char>) -> bool| system.iter().filter_map(|l| l.bars.get(b)).all(f);
            if STANDARD.contains(&(width - 1)) && all(&|bar| is_rest(bar[0])) {
                for line in &mut system {
                    if let Some(bar) = line.bars.get_mut(b) {
                        bar.remove(0);
                    }
                }
                fixed = width - 1;
            } else if STANDARD.contains(&(width - 1)) && all(&|bar| is_rest(bar[width - 1])) {
                for line in &mut system {
                    if let Some(bar) = line.bars.get_mut(b) {
                        bar.pop();
                    }
                }
                fixed = width - 1;
            } else if STANDARD.contains(&(width + 1)) {
                for line in &mut system {
                    if let Some(bar) = line.bars.get_mut(b) {
                        bar.push('-');
                    }
                }
                fixed = width + 1;
            }
        }
        if fixed != width {
            fixed_bars.push(b);
            report.fixed_bars += 1;
        }
        *report.widths.entry(fixed).or_insert(0) += 1;
    }

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
    for line in &system {
        let key = match legend.get(&legend_key(&line.label)) {
            Some(&k) => {
                match label_key(&line.label) {
                    None => report.legend_used += 1,
                    // Count real disagreements, not a different drum of the same family.
                    Some(built_in)
                        if dm_project::drum_family(built_in) != dm_project::drum_family(k) =>
                    {
                        let conflict = format!(
                            "{}={}/{}",
                            line.label,
                            dm_project::drum_name(k),
                            dm_project::drum_name(built_in)
                        );
                        if !report.legend_conflicts.contains(&conflict) {
                            report.legend_conflicts.push(conflict);
                        }
                    }
                    Some(_) => {}
                }
                Some(k)
            }
            None => label_key(&line.label),
        };
        let Some(key) = key else {
            *report.unknown_labels.entry(line.label.clone()).or_insert(0) += 1;
            continue;
        };
        for (bar, symbols) in line.bars.iter().enumerate() {
            read_bar(report, key, symbols, &mut parsed[bar]);
        }
    }
    for (b, bar) in parsed.iter().enumerate() {
        let snare: Vec<usize> = bar.iter().filter(|h| h.0 == 38).map(|h| h.1).collect();
        if !snare.is_empty() {
            let backbeat = snare.contains(&4) || snare.contains(&12);
            let i = usize::from(fixed_bars.contains(&b));
            report.snare_bars[i] += 1;
            report.backbeat_bars[i] += usize::from(backbeat);
        }
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
fn read_bar(report: &mut TabReport, key: u8, chars: &[char], out: &mut Vec<Hit>) {
    let width = chars.len();
    if width == 0 {
        return;
    }
    let hat = key == 42;
    let cymbal = matches!(key, 49 | 51 | 52 | 55 | 57);
    let mut in_parens = false;
    for (col, &c) in chars.iter().enumerate() {
        let mut key = key;
        let velocity = match c {
            c if is_rest(c) => continue,
            '(' | '[' => {
                in_parens = true;
                continue;
            }
            ')' | ']' => {
                in_parens = false;
                continue;
            }
            '+' if !hat => {
                *report.unknown_symbols.entry(c).or_insert(0) += 1;
                continue;
            }
            'x' | 'o' | '*' | '0' | '@' | '#' | '+' | 'f' | 'd' | 'r' | 'g' | 'b' | 'X' | 'O'
            | 'F' | 'D' | 'R' | 'G' | 'B' | 'c' | 'C' | 's' | 'S' | '/' | '%' => {
                // Symbols whose meaning depends on the line.
                match c {
                    'o' | 'O' if hat => key = 46,
                    'b' | 'B' if key == 51 => key = 53,
                    'C' if cymbal || hat => key = 49,
                    'c' if cymbal => key = 52,
                    'r' | 'R' if cymbal => key = 51,
                    's' | 'S' if cymbal => key = 55,
                    'f' | 'F' | 'd' | 'D' | 'r' | 'R' => report.ornaments += 1,
                    _ => {}
                }
                match c {
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
                }
            }
            other => {
                *report.unknown_symbols.entry(other).or_insert(0) += 1;
                continue;
            }
        };
        report.hits += 1;
        let exact = col as f64 * STEPS as f64 / width as f64;
        let step = exact.round() as usize;
        if (exact - step as f64).abs() > 1.0 / 6.0 {
            report.moved += 1;
        }
        // A hit snapped onto the next downbeat stays in this bar's last step.
        out.push((key, step.min(STEPS - 1), velocity));
    }
}

/// Whether a run of characters looks like hits and rests rather than words or guitar frets.
fn looks_like_hits(symbols: &[char]) -> bool {
    if symbols.len() < 4 {
        return false;
    }
    let rests = symbols.iter().filter(|c| matches!(c, '-' | '.')).count();
    let letters = symbols
        .iter()
        .filter(|c| c.is_ascii_alphabetic() && !HIT_LETTERS.contains(**c))
        .count();
    let digits = symbols.iter().filter(|c| c.is_ascii_digit()).count();
    rests * 3 >= symbols.len() && letters * 10 <= symbols.len() && digits * 10 <= symbols.len()
}

/// Recognises a drum line: a label, then a row of hit symbols, with or without bar lines.
fn drum_line(line: &str) -> Option<DrumLine> {
    let trimmed = line.trim();
    let label_len = trimmed
        .char_indices()
        .take_while(|&(i, c)| {
            if i == 0 {
                c.is_ascii_alphabetic()
            } else {
                c.is_ascii_alphanumeric() || ".'".contains(c)
            }
        })
        .count();
    if label_len == 0 || label_len > 8 {
        return None;
    }
    let label = trimmed[..label_len].to_owned();
    let rest = trimmed[label_len..].trim_start_matches([' ', ':', '=']);
    let piped = rest.contains('|');
    let (body, tail) = if piped {
        let rest = rest.trim_start_matches(['|', ':']);
        match rest.rfind('|') {
            Some(last) => {
                let tail = &rest[last + 1..];
                // Text after the last bar line is a bar of its own if it looks like one.
                let tail_symbols: Vec<char> = tail.trim().chars().collect();
                if looks_like_hits(&tail_symbols) && !tail.trim().contains(' ') {
                    (rest, "")
                } else {
                    (&rest[..last], tail)
                }
            }
            None => (rest, ""),
        }
    } else {
        // No bar lines: the row is the first run without spaces.
        let end = rest.find(' ').unwrap_or(rest.len());
        (&rest[..end], &rest[end..])
    };
    let symbols: Vec<char> = body.chars().filter(|c| !"|: ".contains(*c)).collect();
    if !looks_like_hits(&symbols) {
        return None;
    }
    let bars: Vec<Vec<char>> = body
        .split('|')
        .map(|b| {
            b.trim_matches(':')
                .chars()
                .filter(|c| *c != ' ')
                .collect::<Vec<char>>()
        })
        .filter(|b| !b.is_empty())
        .collect();
    if bars.is_empty() {
        return None;
    }
    Some(DrumLine {
        label,
        bars,
        repeat: repeat_mark(tail),
        piped,
    })
}

/// A legend: pairs such as `BS = Bass`, `F1| Floor Tom` or `H : Hihat`, read from anywhere in
/// the text. Maps simplified labels to drums, for labels whose name says which drum.
fn parse_legend(text: &str) -> BTreeMap<String, u8> {
    let mut legend = BTreeMap::new();
    for line in text.lines() {
        // A drum line is not a legend, even if it starts the same way.
        if drum_line(line).is_some() {
            continue;
        }
        // Find every "label separator" pair: a short word right before `=`, `:` or `|`. Each
        // pair's name runs up to the next pair's label.
        let chars: Vec<char> = line.chars().collect();
        let mut pairs: Vec<(usize, usize, String)> = Vec::new(); // (label start, name start, label)
        let mut i = 0;
        while i < chars.len() {
            if matches!(chars[i], '=' | ':' | '|') {
                let mut end = i;
                while end > 0 && chars[end - 1] == ' ' {
                    end -= 1;
                }
                let mut start = end;
                while start > 0 && chars[start - 1].is_ascii_alphanumeric() {
                    start -= 1;
                }
                let mut name = i + 1;
                while name < chars.len() && matches!(chars[name], '=' | ':' | '|' | ' ' | '-') {
                    name += 1;
                }
                let label: String = chars[start..end].iter().collect();
                if (1..=5).contains(&label.len()) {
                    pairs.push((start, name, label));
                }
                i = name.max(i + 1);
            } else {
                i += 1;
            }
        }
        for (n, (_, name_start, label)) in pairs.iter().enumerate() {
            let name_end = pairs.get(n + 1).map_or(chars.len(), |p| p.0);
            if *name_start >= name_end {
                continue;
            }
            let name: String = chars[*name_start..name_end].iter().collect();
            if name.contains("--") {
                continue;
            }
            if let Some(key) = name_key(&name) {
                // A lowercase hit-symbol letter defines a symbol, such as "b = ride bell", not a row.
                if label.len() == 1
                    && HIT_LETTERS.contains(label.as_str())
                    && *label != label.to_ascii_uppercase()
                {
                    continue;
                }
                legend.entry(legend_key(label)).or_insert(key);
            }
        }
    }
    legend
}

/// How a label is looked up in a legend: single letters keep their case, since tabs use `S` for
/// a row and `s` for a symbol; longer labels ignore case and punctuation.
fn legend_key(label: &str) -> String {
    let simple: String = label.chars().filter(char::is_ascii_alphanumeric).collect();
    if simple.len() == 1 {
        simple
    } else {
        simple.to_ascii_lowercase()
    }
}

/// The drum a name such as "Floor Tom" or "Hi-hat (foot)" describes.
fn name_key(name: &str) -> Option<u8> {
    let n = name.to_ascii_lowercase();
    let has = |w: &str| n.contains(w);
    Some(if has("kick") || has("bass") {
        36
    } else if has("snare") {
        38
    } else if has("floor") {
        43
    } else if has("hat") || has("hh") {
        if has("open") {
            46
        } else if has("foot") || has("pedal") {
            44
        } else {
            42
        }
    } else if has("ride") {
        if has("bell") { 53 } else { 51 }
    } else if has("crash") {
        49
    } else if has("china") {
        52
    } else if has("splash") {
        55
    } else if has("tom") {
        if has("low") {
            45
        } else if has("mid") {
            47
        } else {
            50
        }
    } else if has("cowbell") {
        56
    } else if has("clap") {
        39
    } else if has("stick") || has("rim") {
        37
    } else if has("tamb") {
        54
    } else if has("wood") {
        76
    } else {
        return None;
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
    let number = |s: &str| -> Option<f64> {
        let n: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        n.parse::<f64>().ok().filter(|n| (40.0..=300.0).contains(n))
    };
    for (i, _) in lower.match_indices("bpm") {
        let before: String = lower[..i]
            .trim_end()
            .trim_end_matches(['=', ':', ' '])
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let before: String = before.chars().rev().collect();
        if let Some(n) = number(&before) {
            return Some(n);
        }
        if let Some(n) = number(lower[i + 3..].trim_start_matches([' ', '=', ':'])) {
            return Some(n);
        }
    }
    for (i, _) in lower.match_indices("tempo") {
        if let Some(n) = number(lower[i + 5..].trim_start_matches([' ', '=', ':', '-'])) {
            return Some(n);
        }
    }
    None
}

/// The General MIDI key for a row label, or `None` if it is not a known drum.
pub fn label_key(label: &str) -> Option<u8> {
    let l: String = label
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    Some(match l.as_str() {
        "h" | "hh" | "hat" | "hihat" | "hhat" | "chh" | "hc" | "hho" | "hhc" | "hh1" | "hi" => 42,
        "oh" | "ohh" | "ho" => 46,
        "hf" | "hhf" | "hp" | "ph" | "p" | "pd" | "ped" | "pedal" | "hfoot" | "fh" | "hhp"
        | "lf" => 44,
        "s" | "sd" | "sn" | "snr" | "snare" | "sr" | "snd" => 38,
        "b" | "bd" | "k" | "kd" | "kick" | "bass" | "bassdrum" | "b1" | "b2" | "bd1" | "bd2"
        | "k1" | "k2" | "kk" | "bs" | "bb" | "bk" | "kck" => 36,
        "c" | "cc" | "cr" | "crash" | "c1" | "cr1" | "cc1" | "cy" | "cym" | "cym1" => 49,
        "c2" | "cr2" | "cc2" | "cym2" | "c3" | "cr3" => 57,
        "r" | "rd" | "ride" | "rc" | "ri" | "rcy" => 51,
        "rb" | "bell" => 53,
        "ch" | "cn" | "chn" | "china" | "ch1" | "ch2" => 52,
        "sp" | "spl" | "splash" | "sp1" | "sp2" => 55,
        "t" | "t1" | "ht" | "tom" | "tom1" | "tt" | "rt" | "ht1" => 50,
        "t2" | "mt" | "m" | "tom2" | "ht2" => 47,
        "t3" | "lt" | "tom3" => 45,
        "ft" | "f" | "fl" | "t4" | "floor" | "ftom" | "lft" | "ft1" | "ft2" | "t5" | "f1"
        | "f2" | "flt" | "flt1" | "flt2" | "flrtom" => 43,
        "cb" | "cow" | "cowbell" => 56,
        "tb" | "tam" | "tamb" | "tambourine" => 54,
        "cl" | "clap" => 39,
        "ss" | "xs" | "rim" | "sst" | "cs" | "stick" => 37,
        "wb" | "wood" => 76,
        _ => return None,
    })
}

/// How cleanly a tab parses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabVerdict {
    /// Every bar has a standard width as written, every label is known and the systems line up.
    Clean,
    /// Something was guessed, fixed or dropped, but most of the tab came through.
    Partial,
    /// Little or nothing came through.
    Failed,
}

impl TabReport {
    fn sum_widths(widths: &BTreeMap<usize, usize>, of: &[usize]) -> usize {
        of.iter().map(|w| widths.get(w).copied().unwrap_or(0)).sum()
    }

    pub fn standard_bars(&self) -> usize {
        Self::sum_widths(&self.widths, &STANDARD)
    }

    pub fn triplet_bars(&self) -> usize {
        Self::sum_widths(&self.widths, &[6, 12, 24])
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
            || self.fixed_bars > 0
            || !self.unknown_symbols.is_empty()
            || self.moved > 0
        {
            TabVerdict::Partial
        } else {
            TabVerdict::Clean
        }
    }

    pub const CSV_HEADER: &str = "file,verdict,systems,guitar_systems,drum_lines,misaligned,bars_read,bars_after_repeats,standard_bars,fixed_bars,triplet_bars,unpiped_lines,bar_repeats,widths,hits,moved,merged,ornaments,ghosts,accents,unknown_labels,unknown_symbols,legend_labels,legend_used,count_lines,repeats,tempo,bar_ids";

    /// The bars after repeats as ids, equal ids for bars with the same hits (velocity ignored),
    /// numbered in order of first appearance: "0 0 1 0 2".
    pub fn bar_ids(&self) -> String {
        let mut seen: Vec<Vec<(u8, usize)>> = Vec::new();
        let mut ids = Vec::new();
        for bar in &self.grid {
            let mut shape: Vec<(u8, usize)> = bar.iter().map(|h| (h.0, h.1)).collect();
            shape.sort_unstable();
            let id = match seen.iter().position(|s| *s == shape) {
                Some(i) => i,
                None => {
                    seen.push(shape);
                    seen.len() - 1
                }
            };
            ids.push(id.to_string());
        }
        ids.join(" ")
    }

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
            "\"{}\",{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},\"{}\",{},{},{},{},{},{},{}",
            self.path.display(),
            self.verdict(),
            self.systems,
            self.guitar_systems,
            self.drum_lines,
            self.misaligned,
            self.bars_read,
            self.grid.len(),
            self.standard_bars(),
            self.fixed_bars,
            self.triplet_bars(),
            self.unpiped_lines,
            self.bar_repeats,
            widths.join(" "),
            self.hits,
            self.moved,
            self.merged,
            self.ornaments,
            self.ghosts,
            self.accents,
            labels.join(" ").replace('"', "'"),
            symbols.join(" "),
            self.legend_labels,
            self.legend_used,
            self.count_lines,
            self.repeats,
            self.tempo.map_or(String::new(), |t| t.to_string()),
            self.bar_ids(),
        )
    }

    pub fn report(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", self.path.display());
        let _ = writeln!(
            s,
            "  verdict {:?}; {} systems ({} guitar skipped), {} drum lines, {} misaligned; tempo {:?}",
            self.verdict(),
            self.systems,
            self.guitar_systems,
            self.drum_lines,
            self.misaligned,
            self.tempo
        );
        let _ = writeln!(
            s,
            "  {} bars read, {} after {} repeats; {} bar repeats (%); widths {:?}, {} fixed; {} lines without bar lines",
            self.bars_read,
            self.grid.len(),
            self.repeats,
            self.bar_repeats,
            self.widths,
            self.fixed_bars,
            self.unpiped_lines
        );
        let _ = writeln!(
            s,
            "  {} hits: {} moved, {} merged, {} ornaments, {} ghosts, {} accents",
            self.hits, self.moved, self.merged, self.ornaments, self.ghosts, self.accents
        );
        let _ = write!(
            s,
            "  unknown labels {:?}; unknown symbols {:?}; legend defined {} labels, explained {} lines; {} count lines",
            self.unknown_labels,
            self.unknown_symbols,
            self.legend_labels,
            self.legend_used,
            self.count_lines
        );
        if !self.legend_conflicts.is_empty() {
            let _ = write!(
                s,
                "
  legend conflicts: {:?}",
                self.legend_conflicts
            );
        }
        s
    }
}

/// A summary line: its name, and which tabs it counts.
type Check<'a> = (&'a str, &'a dyn Fn(&TabReport) -> bool);

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
    let lines: [Check; 13] = [
        ("no system found", &|r| r.systems == 0),
        ("with guitar systems skipped", &|r| r.guitar_systems > 0),
        ("with misaligned systems", &|r| r.misaligned > 0),
        ("with fixed bar widths", &|r| r.fixed_bars > 0),
        ("with lines without bar lines", &|r| r.unpiped_lines > 0),
        ("with % bar repeats", &|r| r.bar_repeats > 0),
        ("with unknown labels", &|r| !r.unknown_labels.is_empty()),
        ("with unknown symbols", &|r| !r.unknown_symbols.is_empty()),
        ("with a legend", &|r| r.legend_labels > 0),
        ("with triplet-width bars", &|r| r.triplet_bars() > 0),
        ("with repeat marks used", &|r| r.repeats > 0),
        ("with a tempo", &|r| r.tempo.is_some()),
        ("with count lines", &|r| r.count_lines > 0),
    ];
    for (name, f) in lines {
        let c = count(f);
        println!("  {name}: {c} ({:.1}%)", pct(c));
    }

    let mut raw: BTreeMap<usize, usize> = BTreeMap::new();
    let mut widths: BTreeMap<usize, usize> = BTreeMap::new();
    let mut labels: BTreeMap<String, usize> = BTreeMap::new();
    let mut symbols: BTreeMap<char, usize> = BTreeMap::new();
    let (mut bars, mut hits, mut moved, mut merged, mut orn, mut legend) = (0, 0, 0, 0, 0, 0);
    for r in results {
        for (w, c) in &r.raw_widths {
            *raw.entry(*w).or_insert(0) += c;
        }
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
        legend += r.legend_used;
    }
    let raw_standard = TabReport::sum_widths(&raw, &STANDARD);
    let standard = TabReport::sum_widths(&widths, &STANDARD);
    println!(
        "  bars: {bars}; standard width as written {raw_standard} ({:.1}%), after fixing {standard} ({:.1}%)",
        100.0 * raw_standard as f64 / bars.max(1) as f64,
        100.0 * standard as f64 / bars.max(1) as f64
    );
    let mut w: Vec<_> = raw.into_iter().collect();
    w.sort_by_key(|e| std::cmp::Reverse(e.1));
    println!("  bar widths as written: {:?}", &w[..w.len().min(14)]);
    println!(
        "  hits: {hits}, moved {moved} ({:.1}%), merged {merged} ({:.1}%), ornaments {orn}; lines explained by a legend {legend}",
        100.0 * moved as f64 / hits.max(1) as f64,
        100.0 * merged as f64 / hits.max(1) as f64
    );
    for (i, name) in ["as written", "with a fixed width"].iter().enumerate() {
        let snare: usize = results.iter().map(|r| r.snare_bars[i]).sum();
        let backbeat: usize = results.iter().map(|r| r.backbeat_bars[i]).sum();
        println!(
            "  bars with a snare, {name}: {snare}; with a snare on beat 2 or 4: {backbeat} ({:.1}%)",
            100.0 * backbeat as f64 / snare.max(1) as f64
        );
    }
    let mut l: Vec<_> = labels.into_iter().collect();
    l.sort_by_key(|e| std::cmp::Reverse(e.1));
    println!("  unknown labels: {:?}", &l[..l.len().min(25)]);
    let mut s: Vec<_> = symbols.into_iter().collect();
    s.sort_by_key(|e| std::cmp::Reverse(e.1));
    println!("  unknown symbols: {:?}", &s[..s.len().min(20)]);
    let conflicts: Vec<&String> = results.iter().flat_map(|r| &r.legend_conflicts).collect();
    let tabs = results
        .iter()
        .filter(|r| !r.legend_conflicts.is_empty())
        .count();
    println!(
        "  legend conflicts with usual labels: {} lines in {tabs} tabs: {conflicts:?}",
        conflicts.len()
    );
}
