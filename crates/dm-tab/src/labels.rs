//! Which drum a row label names: the tab's own legend first, then a built-in table.

use std::collections::BTreeMap;

use crate::read::{HIT_LETTERS, drum_line};

/// A legend: pairs such as `BS = Bass`, `F1| Floor Tom` or `H : Hihat`, read from anywhere in
/// the text. Maps simplified labels to drums, for labels whose name says which drum.
pub(crate) fn parse_legend(text: &str) -> BTreeMap<String, u8> {
    let mut legend = BTreeMap::new();
    for line in text.lines() {
        // A drum line is not a legend, even if it starts the same way.
        if drum_line(line).is_some() {
            continue;
        }
        // Find every "label separator" pair: a short word right before `=`, `:` or `|`. Each
        // pair's name runs up to the next pair's label.
        let chars: Vec<char> = line.chars().collect();
        // (label start, name start, label)
        let mut pairs: Vec<(usize, usize, String)> = Vec::new();
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
            let Some(key) = name_key(&name) else {
                continue;
            };
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
    legend
}

/// How a label is looked up in a legend. Single letters keep their case, since tabs use `S` for
/// a row and `s` for a symbol. Longer labels ignore case and punctuation.
pub(crate) fn legend_key(label: &str) -> String {
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

/// The General MIDI key for a row label in the built-in table, or `None` if it is not there.
pub(crate) fn label_key(label: &str) -> Option<u8> {
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

/// The drum a row label names: the legend's meaning if it gives one, else the built-in table's.
pub(crate) fn row_key(label: &str, legend: &BTreeMap<String, u8>) -> Option<u8> {
    legend
        .get(&legend_key(label))
        .copied()
        .or_else(|| label_key(label))
}
