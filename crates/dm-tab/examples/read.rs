//! Reads a drum tab from a text file and prints what a paste of it would give: the refusal, or
//! the bars, the bars left once exact repeats collapse, and what was dropped. For trying tabs
//! without the app.
//!
//! Usage: `cargo run -p dm-tab --example read -- <tab.txt>`

use dm_project::{BarHit, repeat_unit};

fn main() {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: read <tab.txt>");
        std::process::exit(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("{}: {e}", path.to_string_lossy());
            std::process::exit(1);
        }
    };
    let tab = match dm_tab::parse(&text) {
        Ok(tab) => tab,
        Err(refusal) => {
            println!("refused: {refusal:?}");
            return;
        }
    };
    let bars: Vec<Vec<BarHit>> = tab
        .bars
        .iter()
        .map(|bar| {
            bar.iter()
                .map(|h| BarHit {
                    key: h.key,
                    step: h.step,
                    velocity: h.velocity,
                })
                .collect()
        })
        .collect();
    let unit = repeat_unit(&bars);
    let mut distinct: Vec<&Vec<BarHit>> = Vec::new();
    for bar in &bars {
        if !distinct.iter().any(|d| same(d, bar)) {
            distinct.push(bar);
        }
    }
    println!("bars after repeats: {}", tab.bars.len());
    println!("bars kept after collapsing exact repeats: {unit}");
    println!("different bars: {}", distinct.len());
    println!("dropped rows: {:?}", tab.dropped_labels);
    println!("merged hits: {}", tab.merged);
}

/// Whether two bars have the same drums on the same steps, velocity ignored.
fn same(a: &[BarHit], b: &[BarHit]) -> bool {
    let cells = |bar: &[BarHit]| {
        let mut c: Vec<(u8, usize)> = bar.iter().map(|h| (h.key, h.step)).collect();
        c.sort_unstable();
        c
    };
    cells(a) == cells(b)
}
