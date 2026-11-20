//! Picks the grooves the app ships from Google Magenta's Groove MIDI Dataset (CC BY 4.0): a few
//! beats per style, from the files the app accepts. Writes them as a pack in the app's JSON form.
//!
//! Usage: `cargo run --release -p dm-groove --example curate_magenta -- <groove-v1.0.0-midionly.zip>
//! [<out.json>]`. Without an output file it only lists what it would pick.
//!
//! The dataset's files are named like `149_latin-brazilian-baiao_95_beat_4-4`: a take number,
//! the style, the tempo, `beat` or `fill`, and the time signature. Only beats are picked, at most
//! [`PER_STYLE`] per style. Styles such as `funk-groove5` (ten set grooves several drummers
//! played) count as their style, `funk`, and a file name picked once is not picked again, since
//! several drummers' takes share it. Beats that repeat come first, as they loop cleanly, then
//! longer ones, then by take number, so the choice is the same every run.

use std::collections::BTreeMap;

use dm_groove::{Groove, Pack};
use dm_project::BarHit;

/// The most beats picked for one style.
const PER_STYLE: usize = 3;
/// The fewest notes a picked beat has, so a near-silent take is left out.
const MIN_NOTES: usize = 8;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(zip) = args.first() else {
        eprintln!("usage: curate_magenta <groove-v1.0.0-midionly.zip> [<out.json>]");
        std::process::exit(2);
    };
    let bytes = std::fs::read(zip).unwrap_or_else(|e| {
        eprintln!("{zip}: {e}");
        std::process::exit(1);
    });
    let import = dm_groove::import("groove.zip", &bytes).unwrap_or_else(|e| {
        eprintln!("{zip}: {e:?}");
        std::process::exit(1);
    });

    // Beats by style: (take number, unit length in bars, groove).
    let mut styles: BTreeMap<String, Vec<(u32, usize, Groove)>> = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    for groove in import.grooves {
        if !seen.insert(groove.name.clone()) {
            continue;
        }
        let parts: Vec<&str> = groove.name.split('_').collect();
        let [take, style, _bpm, kind, _signature] = parts[..] else {
            continue;
        };
        let notes: usize = groove.bars.iter().map(Vec::len).sum();
        if kind != "beat" || notes < MIN_NOTES {
            continue;
        }
        let unit = dm_project::repeat_unit(&groove.bars);
        let take = take.parse().unwrap_or(u32::MAX);
        // `funk-groove5` is a funk groove.
        let style = match style.split_once("-groove") {
            Some((base, _)) => base,
            None => style,
        };
        styles
            .entry(style.to_owned())
            .or_default()
            .push((take, unit, groove));
    }

    let mut picked = Vec::new();
    for (style, mut beats) in styles {
        // Beats that repeat first, as they loop cleanly; then longer ones, with more of the
        // drummer's playing.
        beats.sort_by_key(|(take, unit, groove)| {
            (*unit >= groove.bars.len(), std::cmp::Reverse(*unit), *take)
        });
        let label = style_label(&style);
        for (n, (_, unit, groove)) in beats.into_iter().take(PER_STYLE).enumerate() {
            // The whole take, as the import gives it: at most 16 bars, exact repeats collapsed.
            let bars: Vec<Vec<BarHit>> = groove.bars[..unit].to_vec();
            println!(
                "{label} {}: {} ({} bars of {}, {} BPM)",
                n + 1,
                groove.name,
                bars.len(),
                groove.bars.len(),
                groove.bpm
            );
            picked.push(Groove {
                name: format!("{label} {}", n + 1),
                folder: format!("{} BPM", groove.bpm),
                bpm: groove.bpm,
                bars,
            });
        }
    }
    println!("{} grooves", picked.len());

    if let Some(out) = args.get(1) {
        let pack = Pack {
            name: "Groove MIDI Dataset".to_owned(),
            grooves: picked,
        };
        let json = dm_groove::pack_to_json(&pack);
        std::fs::write(out, &json).unwrap_or_else(|e| {
            eprintln!("{out}: {e}");
            std::process::exit(1);
        });
        println!("wrote {out}: {} bytes", json.len());
    }
}

/// A style as people say it: `latin-brazilian-baiao` becomes `Baião`, `soul-motown` becomes
/// `Soul motown`. The words "latin" and "brazilian" only say where a style comes from.
fn style_label(style: &str) -> String {
    let words: Vec<&str> = style
        .split('-')
        .filter(|w| !matches!(*w, "latin" | "brazilian"))
        .map(|w| match w {
            "baiao" => "baião",
            "hiphop" => "hip hop",
            w => w,
        })
        .collect();
    let mut label = words.join(" ");
    if let Some(first) = label.get(..1) {
        label = first.to_uppercase() + &label[1..];
    }
    label
}
