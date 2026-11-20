//! Gathers the grooves the app ships from Google Magenta's Groove MIDI Dataset (CC BY 4.0):
//! every file the app accepts, beats and fills, grouped by style. Writes them as a pack in the
//! app's JSON form.
//!
//! Usage: `cargo run --release -p dm-groove --example curate_magenta -- <groove-v1.0.0-midionly.zip>
//! [<out.json>]`. Without an output file it only counts what it would write.
//!
//! The dataset's files are named like `149_latin-brazilian-baiao_95_beat_4-4`: a take number,
//! the style, the tempo, `beat` or `fill`, and the time signature. Styles such as `funk-groove5`
//! (ten set grooves several drummers played) count as their style, `funk`. Each groove's folder
//! is its style, which the app's lists group by. Within a style the grooves of up to
//! [`SHORT`] bars come first, as they are the easiest to start from; then, in each group, beats
//! before fills, each by tempo and then take number, so the order is the same every run. Several
//! drummers' takes may share a file name; each take is kept, as each is played differently.

use std::collections::BTreeMap;

use dm_groove::{Groove, Pack};
use dm_project::BarHit;

/// Grooves of up to this many bars come first in their style.
const SHORT: usize = 4;

/// One take: whether it is a fill, its tempo and take number for sorting, and the groove.
struct Take {
    fill: bool,
    take: u32,
    groove: Groove,
}

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
    println!(
        "{} files accepted, {} refused",
        import.grooves.len(),
        import.refused.len()
    );

    // By the style's name in lower case, so `Funk Purdie shuffle` sorts after `Funk fast`.
    let mut styles: BTreeMap<String, (String, Vec<Take>)> = BTreeMap::new();
    let mut unnamed = 0;
    for groove in import.grooves {
        let parts: Vec<&str> = groove.name.split('_').collect();
        let [take, style, _bpm, kind, _signature] = parts[..] else {
            unnamed += 1;
            continue;
        };
        // `funk-groove5` is a funk groove.
        let style = style.split_once("-groove").map_or(style, |(base, _)| base);
        let label = style_label(style);
        styles
            .entry(label.to_lowercase())
            .or_insert_with(|| (label, Vec::new()))
            .1
            .push(Take {
                fill: kind == "fill",
                take: take.parse().unwrap_or(u32::MAX),
                groove,
            });
    }
    if unnamed > 0 {
        println!("{unnamed} files left out: their names are not in the dataset's form");
    }

    let mut grooves = Vec::new();
    for (label, mut takes) in styles.into_values() {
        takes.sort_by_key(|t| {
            let bars = dm_project::repeat_unit(&t.groove.bars);
            (bars > SHORT, t.fill, t.groove.bpm as u32, t.take)
        });
        let (mut beats, mut fills) = (0, 0);
        for Take { fill, groove, .. } in takes {
            // The whole take, as the import gives it: at most 16 bars, exact repeats collapsed.
            let unit = dm_project::repeat_unit(&groove.bars);
            let bars: Vec<Vec<BarHit>> = groove.bars[..unit].to_vec();
            let name = if fill {
                fills += 1;
                format!("{label} {fills} (fill)")
            } else {
                beats += 1;
                format!("{label} {beats}")
            };
            grooves.push(Groove {
                name,
                folder: label.clone(),
                bpm: groove.bpm,
                bars,
            });
        }
        println!("{label}: {beats} beats, {fills} fills");
    }
    println!("{} grooves", grooves.len());

    if let Some(out) = args.get(1) {
        let pack = Pack {
            name: "Groove MIDI Dataset".to_owned(),
            grooves,
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
            "afrocuban" => "afro-Cuban",
            "neworleans" => "New Orleans",
            "purdieshuffle" => "Purdie shuffle",
            w => w,
        })
        .collect();
    let mut label = words.join(" ");
    if let Some(first) = label.get(..1) {
        label = first.to_uppercase() + &label[1..];
    }
    label
}
