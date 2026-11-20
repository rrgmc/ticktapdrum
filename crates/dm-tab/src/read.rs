//! Reading single lines: drum lines, count lines, repeat marks, tempo, and the hits in a bar.

use dm_project::{ACCENT_VELOCITY, DEFAULT_VELOCITY, SOFT_VELOCITY, STEPS_PER_BAR};

/// Letters that can be hit symbols, so a body holding them still looks like a drum line.
pub(crate) const HIT_LETTERS: &str = "xXoOgGfFdDrRbBcCsS";

/// A drum line: its label, the bars of symbols, and a repeat mark written after it.
pub(crate) struct DrumLine {
    pub label: String,
    pub bars: Vec<Vec<char>>,
    pub repeat: Option<usize>,
    /// Whether the line has bar lines at all.
    pub piped: bool,
}

/// One hit symbol read from a bar: its column, drum and velocity.
pub(crate) struct Symbol {
    pub column: usize,
    pub key: u8,
    pub velocity: u8,
}

/// Undoes web typography and other bar-line styles. `¦`, `│` and `I` between hits become `|`.
/// Long dashes become `-`. An arrow `>` at the end of a bar, meaning "goes on", becomes a rest.
pub(crate) fn normalize(line: &str) -> String {
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

pub(crate) fn is_rest(c: char) -> bool {
    matches!(c, '-' | '.' | '_' | '=' | '~' | ' ')
}

/// Reads one bar of one drum line. `key` is the row's drum; some symbols change it, such as `o`
/// on a hi-hat row, which is the open hat.
pub(crate) fn read_bar(key: u8, chars: &[char]) -> Vec<Symbol> {
    let hat = key == 42;
    let cymbal = matches!(key, 49 | 51 | 52 | 55 | 57);
    let mut in_parens = false;
    let mut out = Vec::new();
    for (column, &c) in chars.iter().enumerate() {
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
            // `+` is a closed hat on a hi-hat row and means nothing elsewhere.
            '+' if !hat => continue,
            'x' | 'o' | '*' | '0' | '@' | '#' | '+' | 'f' | 'd' | 'r' | 'g' | 'b' | 'X' | 'O'
            | 'F' | 'D' | 'R' | 'G' | 'B' | 'c' | 'C' | 's' | 'S' | '/' | '%' => {
                // Symbols whose meaning depends on the row.
                match c {
                    'o' | 'O' if hat => key = 46,
                    'b' | 'B' if key == 51 => key = 53,
                    'C' if cymbal || hat => key = 49,
                    'c' if cymbal => key = 52,
                    'r' | 'R' if cymbal => key = 51,
                    's' | 'S' if cymbal => key = 55,
                    _ => {}
                }
                match c {
                    'X' | 'O' | '@' => ACCENT_VELOCITY,
                    'g' | 'G' => SOFT_VELOCITY,
                    _ if in_parens => SOFT_VELOCITY,
                    _ => DEFAULT_VELOCITY,
                }
            }
            _ => continue,
        };
        out.push(Symbol {
            column,
            key,
            velocity,
        });
    }
    out
}

/// The step a column of a bar `width` columns wide lands on. Only standard widths reach here,
/// so the division is exact except for 32, where two columns share a step.
pub(crate) fn step_of(column: usize, width: usize) -> usize {
    (column * STEPS_PER_BAR / width).min(STEPS_PER_BAR - 1)
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
pub(crate) fn drum_line(line: &str) -> Option<DrumLine> {
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
    // The label is ASCII, so its length in characters is also its length in bytes.
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

/// A repeat mark such as `x4`, `4x`, `(x2)` or `repeat 3 times`. The number is total plays.
pub(crate) fn repeat_mark(text: &str) -> Option<usize> {
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

/// Whether a line counts beats, such as `1 e + a 2 e + a`.
pub(crate) fn is_count_line(line: &str) -> bool {
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

/// A tempo written in the text, such as `120 bpm`, `BPM: 96` or `Tempo = 140`.
pub(crate) fn find_tempo(text: &str) -> Option<f32> {
    let lower = text.to_ascii_lowercase();
    let number = |s: &str| -> Option<f32> {
        let n: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        n.parse::<f32>().ok().filter(|n| (40.0..=300.0).contains(n))
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
