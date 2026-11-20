//! Writes `crates/dm-groove/src/yamaha_kits.rs`, the key map for drum kits in Yamaha styles, from
//! the "Drum&SFX Kit List SX720" sheet of Yamaha's PSR-SX920/PSR-SX720 Data List.
//!
//! The Data List is on Yamaha's PSR-SX720 support page, under Manuals & Documents. It downloads as
//! a `.zip` holding `psrsx920_sx720_en_dl_a0.xlsx`; pass that spreadsheet. It is not committed.
//! See "Import grooves from a keyboard style" in `docs/DESIGN.md` and
//! `docs/research/style-import-spike.md`.
//!
//! `cargo run -p dm-yamaha-kits -- <psrsx920_sx720_en_dl_a0.xlsx>`
//!
//! For each kit the table holds its bank, program and name, and for each key that sounds the
//! General MIDI key it becomes. A key whose sound has no place on the app's rows (clicks, loop
//! slices, effects) is left out of the table, and so is its note.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Read;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The sheet holding the kits.
const SHEET: &str = "Drum&amp;SFX Kit List SX720";
/// Where the table goes, from the workspace root.
const OUT: &str = "crates/dm-groove/src/yamaha_kits.rs";

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: dm-yamaha-kits <psrsx920_sx720_en_dl_a0.xlsx>")?;
    let mut zip = zip::ZipArchive::new(File::open(&path)?)?;
    let read = |zip: &mut zip::ZipArchive<File>, name: &str| -> Result<String> {
        let mut s = String::new();
        zip.by_name(name)?.read_to_string(&mut s)?;
        Ok(s)
    };
    let strings = shared_strings(&read(&mut zip, "xl/sharedStrings.xml")?);
    let sheet_file = sheet_path(
        &read(&mut zip, "xl/workbook.xml")?,
        &read(&mut zip, "xl/_rels/workbook.xml.rels")?,
    )?;
    let rows = rows(&read(&mut zip, &sheet_file)?, &strings);
    let kits = kits(&rows)?;
    let table = table(&kits);
    std::fs::write(OUT, table)?;
    let keys: usize = kits.iter().map(|k| k.keys.len()).sum();
    let kept: usize = kits
        .iter()
        .map(|k| k.keys.iter().filter(|(_, g)| g.is_some()).count())
        .sum();
    println!(
        "wrote {OUT}: {} kits, {kept} of {keys} keys kept",
        kits.len()
    );
    Ok(())
}

/// One kit: bank select MSB, program counted from 0, name, and each sounding key with the
/// General MIDI key it becomes, or `None` to leave it out.
struct Kit {
    msb: u8,
    program: u8,
    name: String,
    keys: Vec<(u8, Option<u8>)>,
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// The text of every `<tag ...>text</tag>` in `xml`, in order.
fn texts<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        // Skip longer tag names that start the same, such as <si> for <s.
        if !after.starts_with('>') && !after.starts_with(' ') {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            out.push("");
            rest = &after[gt + 1..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        out.push(&body[..end]);
        rest = &body[end + close.len()..];
    }
    out
}

fn shared_strings(xml: &str) -> Vec<String> {
    texts(xml, "si")
        .into_iter()
        .map(|si| texts(si, "t").into_iter().map(unescape).collect())
        .collect()
}

/// The sheet's file inside the workbook.
fn sheet_path(workbook: &str, rels: &str) -> Result<String> {
    let at = workbook
        .find(&format!("name=\"{SHEET}\""))
        .ok_or("no Drum&SFX Kit List SX720 sheet")?;
    let tag_end = at + workbook[at..].find('>').ok_or("bad workbook")?;
    let tag = &workbook[..tag_end];
    let id_at = tag.rfind("r:id=\"").ok_or("sheet without an id")? + 6;
    let id = &tag[id_at..id_at + tag[id_at..].find('"').ok_or("bad id")?];
    let rel_at = rels
        .find(&format!("Id=\"{id}\""))
        .ok_or("sheet id not in the relationships")?;
    let rel = &rels[rel_at..rel_at + rels[rel_at..].find('>').ok_or("bad relationship")?];
    let target_at = rel.find("Target=\"").ok_or("no target")? + 8;
    let target = &rel[target_at..target_at + rel[target_at..].find('"').ok_or("bad target")?];
    Ok(format!(
        "xl/{}",
        target.trim_start_matches('/').trim_start_matches("xl/")
    ))
}

/// The sheet as rows of cells by column, strings resolved.
fn rows(xml: &str, strings: &[String]) -> Vec<BTreeMap<usize, String>> {
    let mut out = Vec::new();
    for row in texts(xml, "row") {
        let mut cells = BTreeMap::new();
        let mut rest = row;
        while let Some(i) = rest.find("<c ") {
            let after = &rest[i..];
            let Some(gt) = after.find('>') else { break };
            let tag = &after[..gt];
            let (body, next) = if tag.ends_with('/') {
                ("", &after[gt + 1..])
            } else {
                let end = after.find("</c>").unwrap_or(after.len());
                (&after[gt + 1..end], &after[(end + 4).min(after.len())..])
            };
            rest = next;
            let Some(r) = tag.find("r=\"") else { continue };
            let column = tag[r + 3..]
                .chars()
                .take_while(char::is_ascii_uppercase)
                .fold(0usize, |n, c| n * 26 + (c as usize - 'A' as usize + 1))
                - 1;
            let value = texts(body, "v").first().map(|v| v.to_string());
            let text = if tag.contains("t=\"s\"") {
                value
                    .and_then(|v| v.parse::<usize>().ok())
                    .and_then(|i| strings.get(i).cloned())
            } else if let Some(v) = value {
                Some(unescape(&v))
            } else {
                texts(body, "t").first().map(|t| unescape(t))
            };
            if let Some(text) = text {
                cells.insert(column, text.trim().to_string());
            }
        }
        out.push(cells);
    }
    out
}

/// The kits: names on the row that starts "Kit Name", bank and program ("127-0-74", the program
/// counted from 1) on the next row, then one row per key with the MIDI note number in column B.
/// Each kit takes four columns, its sound's name in the first.
fn kits(rows: &[BTreeMap<usize, String>]) -> Result<Vec<Kit>> {
    let names_row = rows
        .iter()
        .position(|r| r.values().any(|v| v == "Kit Name"))
        .ok_or("no Kit Name row")?;
    let names = &rows[names_row];
    let ids = &rows[names_row + 1];
    let mut kits: Vec<(usize, Kit)> = Vec::new();
    for (&column, id) in ids {
        let parts: Vec<&str> = id.split('-').collect();
        let [msb, _lsb, program] = parts[..] else {
            continue;
        };
        let (Ok(msb), Ok(program)) = (msb.parse::<u8>(), program.parse::<u8>()) else {
            continue;
        };
        let name = names.get(&column).cloned().unwrap_or_default();
        kits.push((
            column,
            Kit {
                msb,
                program: program - 1,
                name,
                keys: Vec::new(),
            },
        ));
    }
    for row in &rows[names_row + 2..] {
        let Some(key) = row.get(&1).and_then(|k| k.parse::<u8>().ok()) else {
            continue;
        };
        for (column, kit) in &mut kits {
            if let Some(sound) = row.get(column).filter(|s| !s.is_empty()) {
                kit.keys.push((key, general_midi(key, sound)));
            }
        }
    }
    if kits.is_empty() {
        return Err("no kits found".into());
    }
    Ok(kits.into_iter().map(|(_, k)| k).collect())
}

/// General MIDI keys that play the same kind of sound. A sound already on a key of its own kind
/// keeps that key, so toms keep their pitch.
const FAMILIES: [&[u8]; 13] = [
    &[35, 36],
    &[37, 38, 40],
    &[42, 44, 46],
    &[41, 43, 45, 47, 48, 50],
    &[49, 51, 52, 53, 55, 57, 59],
    &[60, 61],
    &[62, 63, 64],
    &[65, 66],
    &[67, 68],
    &[73, 74],
    &[76, 77],
    &[78, 79],
    &[80, 81],
];

fn family(key: u8) -> Option<usize> {
    FAMILIES.iter().position(|f| f.contains(&key))
}

/// The General MIDI key for a sound, from its name, or `None` to leave it out.
fn general_midi(key: u8, sound: &str) -> Option<u8> {
    let named = by_name(key, sound)?;
    if (35..=81).contains(&key) && family(named).is_some() && family(named) == family(key) {
        return Some(key);
    }
    Some(named)
}

fn by_name(key: u8, sound: &str) -> Option<u8> {
    let n = format!(" {} ", sound.to_lowercase().replace('-', " "));
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    let low = has(&[" low ", " lo ", " l "]);
    if has(&["hi hat", "hihat"]) {
        if has(&["pedal"]) {
            return Some(44);
        }
        if has(&["open"]) {
            return Some(46);
        }
        // "Hi-Hat Tip 00" to "Hi-Hat Edge 99": how far the hi-hat is open.
        let open = n
            .split(|c: char| !c.is_ascii_digit())
            .filter(|d| d.len() == 2)
            .find_map(|d| d.parse::<u8>().ok());
        return Some(if open.is_some_and(|o| o >= 50) {
            46
        } else {
            42
        });
    }
    if has(&["kick", "bass drum", "bassdrum", " bd "]) {
        return Some(36);
    }
    if has(&["side stick", "sidestick", "cross stick"]) {
        return Some(37);
    }
    if has(&["snare", "brush slap", "brush tap", "brush swirl"]) {
        return Some(38);
    }
    if has(&[" rim "]) {
        return Some(37);
    }
    if has(&["clap"]) {
        return Some(39);
    }
    if has(&[" tom"]) {
        return Some(if low { 45 } else { 47 });
    }
    let found = [
        (&["crash"][..], 49),
        (&["china"], 52),
        (&["splash"], 55),
        (&["tambourine"], 54),
        (&["cowbell"], 56),
        (&["shaker", "maraca"], 70),
        (&["cabasa"], 69),
        (&["guiro"], 73),
        (&["clave"], 75),
        (&["wood block", "woodblock"], 76),
    ]
    .iter()
    .find(|(words, _)| has(words))
    .map(|&(_, k)| k);
    if found.is_some() {
        return found;
    }
    if has(&["ride"]) {
        return Some(if has(&["bell", "cup"]) { 53 } else { 51 });
    }
    if has(&["conga"]) {
        return Some(if low { 64 } else { 63 });
    }
    if has(&["bongo"]) {
        return Some(if low { 61 } else { 60 });
    }
    if has(&["timbale"]) {
        return Some(if low { 66 } else { 65 });
    }
    if has(&["agogo"]) {
        return Some(if low { 68 } else { 67 });
    }
    if has(&["cuica"]) {
        return Some(78);
    }
    if has(&["triangle"]) {
        return Some(if has(&["mute", "close"]) { 80 } else { 81 });
    }
    // Clicks, loops, glitches, noises and effects have no row. Any other sound on a General
    // MIDI key keeps it.
    let effect = has(&[
        "click",
        "loop",
        "glitch",
        "noise",
        "reverse",
        "impact",
        "fx",
        "scratch",
        "metronome",
        "count",
        " vo ",
        "voice",
    ]);
    ((35..=81).contains(&key) && !effect).then_some(key)
}

/// The General MIDI program of the nearest kit in the app's short list of kits (see
/// `SHORT_LIST` in `crates/dm-ui/src/sounds.rs`), from the Yamaha kit's name. `None` for
/// percussion and effect kits (bank MSB 126), which have no counterpart there.
fn similar(msb: u8, name: &str) -> Option<u8> {
    if msb != 127 {
        return None;
    }
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    Some(if has(&["brush"]) {
        40
    } else if has(&["jazz"]) {
        32
    } else if has(&["room"]) {
        8
    } else if has(&["rock", "power"]) {
        16
    } else if has(&["t8", "808"]) {
        25
    } else if has(&["t9", "909", "house"]) {
        30
    } else if has(&["edm", "dance", "dubstep"]) {
        26
    } else if has(&["hiphop", "hip hop", "break", "trap"]) {
        9
    } else if has(&["electro", "machine", "analog", "80s"]) {
        24
    } else if has(&["symphony", "orchestra"]) {
        48
    } else {
        0
    })
}

fn table(kits: &[Kit]) -> String {
    let mut s = String::new();
    s.push_str(
        "//! Generated by `tools/yamaha-kits` from the \"Drum&SFX Kit List SX720\" sheet of Yamaha's\n\
         //! PSR-SX920/PSR-SX720 Data List. Do not edit; run the tool again.\n\
         //!\n\
         //! Each kit: bank select MSB, program counted from 0, its name, the General MIDI program of\n\
         //! the nearest kit in the app's short list (`None` for percussion and effect kits), and for\n\
         //! each key that has a place on the app's rows, the General MIDI key it becomes. Other keys\n\
         //! are left out.\n\n\
         use super::YamahaKit;\n\n\
         #[rustfmt::skip]\n\
         pub(super) const KITS: &[YamahaKit] = &[\n",
    );
    for kit in kits {
        let keys: Vec<String> = kit
            .keys
            .iter()
            .filter_map(|&(k, g)| g.map(|g| format!("({k}, {g})")))
            .collect();
        let _ = writeln!(
            s,
            "    YamahaKit {{ msb: {}, program: {}, name: {:?}, similar: {:?}, keys: &[{}] }},",
            kit.msb,
            kit.program,
            kit.name,
            similar(kit.msb, &kit.name),
            keys.join(", ")
        );
    }
    s.push_str("];\n");
    s
}
