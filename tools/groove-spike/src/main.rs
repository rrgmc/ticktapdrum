//! Spike: how well groove-pack MIDI files import into TickTapDrum, and how they sound.
//!
//! This is throwaway research code, not the importer. It answers three questions on real files:
//! how far the notes sit from the 16-step grid, which drum keys they use, and how many patterns
//! a file becomes. It can also write the import as a `.drums` project and render both the
//! original performance and the imported grid to WAV files, for listening side by side.
//!
//! Usage:
//!
//! ```text
//! groove-spike analyze [--half] [--map auto|gm|td11] <file or folder>... [--csv out.csv]
//! groove-spike convert [--map ...] <in.mid> <out.drums>
//! groove-spike render  [--map ...] <in.mid> <bank.sf2> <out folder>
//! groove-spike tab <file.txt or folder>... [--csv out.csv]
//! groove-spike tab-render <tab.txt> <bank.sf2> <out folder>
//! ```

mod analysis;
mod build;
mod keymap;
mod read;
mod render;
mod tab;

use std::path::{Path, PathBuf};

use analysis::Analysis;
use keymap::KeyMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let map = take_option(&mut args, "--map")
        .map(|m| KeyMap::parse(&m))
        .transpose()?
        .unwrap_or(KeyMap::Auto);
    if let Some(i) = args.iter().position(|a| a == "--half") {
        args.remove(i);
        analysis::HALF_TIME.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let csv = take_option(&mut args, "--csv").map(PathBuf::from);
    let command = if args.is_empty() {
        String::new()
    } else {
        args.remove(0)
    };
    match command.as_str() {
        "analyze" => analyze(&args, map, csv.as_deref()),
        "convert" if args.len() == 2 => {
            let a = analysis_of(Path::new(&args[0]), map)?;
            let project = build::project(&a);
            std::fs::write(&args[1], dm_project::to_json(&project.project))?;
            print_report(&a, &project);
            Ok(())
        }
        "roundtrip" => roundtrip(),
        "tab" => tabs(&args, csv.as_deref()),
        "tab-render" if args.len() == 3 => render::tab(
            Path::new(&args[0]),
            Path::new(&args[1]),
            Path::new(&args[2]),
        ),
        "render" if args.len() == 3 => render::run(
            Path::new(&args[0]),
            Path::new(&args[1]),
            Path::new(&args[2]),
            map,
        ),
        _ => Err("usage: groove-spike analyze|convert|render ... (see the source)".into()),
    }
}

fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.remove(i);
    (i < args.len()).then(|| args.remove(i))
}

pub fn analysis_of(path: &Path, map: KeyMap) -> Result<Analysis, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let file = read::parse(&bytes)?;
    Ok(analysis::analyze(path, &file, map))
}

fn collect_ext(path: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for e in entries {
            collect_ext(&e, ext, out);
        }
    } else if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    {
        out.push(path.to_owned());
    }
}

/// Reads a tab file as text. Old tabs are often Latin-1, so bytes that are not UTF-8 are
/// replaced rather than refused.
pub fn read_text(path: &Path) -> std::io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

fn tabs(paths: &[String], csv: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for p in paths {
        collect_ext(Path::new(p), "txt", &mut files);
    }
    let mut results = Vec::new();
    let mut failed = Vec::new();
    for f in &files {
        match read_text(f) {
            Ok(text) => results.push(tab::parse(f, &text)),
            Err(e) => failed.push((f.clone(), e.to_string())),
        }
    }
    if let Some(csv) = csv {
        let mut text = String::from(tab::TabReport::CSV_HEADER);
        text.push('\n');
        for r in &results {
            text.push_str(&r.csv_row());
            text.push('\n');
        }
        std::fs::write(csv, text)?;
    }
    if results.len() == 1 {
        let r = &results[0];
        let built = build::from_grid(&r.grid, r.tempo.unwrap_or(100.0), 0.0);
        println!("{}\n{}", r.report(), built.report());
    } else {
        tab::print_summary(&results, &failed);
    }
    Ok(())
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for e in entries {
            collect(&e, out);
        }
    } else if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mid") || e.eq_ignore_ascii_case("midi"))
    {
        out.push(path.to_owned());
    }
}

fn analyze(
    paths: &[String],
    map: KeyMap,
    csv: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for p in paths {
        collect(Path::new(p), &mut files);
    }
    let mut results = Vec::new();
    let mut failed = Vec::new();
    for f in &files {
        match analysis_of(f, map) {
            Ok(a) => results.push(a),
            Err(e) => failed.push((f.clone(), e.to_string())),
        }
    }
    if let Some(csv) = csv {
        let mut text = String::from(Analysis::CSV_HEADER);
        text.push('\n');
        for a in &results {
            text.push_str(&a.csv_row());
            text.push('\n');
        }
        std::fs::write(csv, text)?;
    }
    if results.len() == 1 && failed.is_empty() {
        let project = build::project(&results[0]);
        print_report(&results[0], &project);
    } else {
        analysis::print_summary(&results, &failed);
    }
    Ok(())
}

/// Exports every preset with the app's MIDI writer, imports it again and compares the steps of
/// every audible row, ignoring row order.
fn roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::BTreeSet;
    let hits = |p: &dm_project::Project| -> BTreeSet<(usize, u8, usize, u8)> {
        let mut out = BTreeSet::new();
        let mut at = 0;
        for &slot in p.song() {
            let pattern = &p.patterns()[p.index_of_slot(slot).unwrap()];
            for (row, info) in p.rows().iter().enumerate() {
                for step in 0..pattern.length() {
                    let v = pattern.velocity(row, step);
                    if v > 0 {
                        out.insert((at + step / 16, info.key, step % 16, v));
                    }
                }
            }
            at += pattern.bars();
        }
        out
    };
    for preset in dm_project::presets() {
        let original = dm_project::preset_project(preset.name).unwrap();
        let file = read::parse(&dm_project::to_midi(&original))?;
        let a = analysis::analyze(Path::new(preset.name), &file, KeyMap::Gm);
        let built = build::project(&a);
        let same = hits(&original) == hits(&built.project);
        println!(
            "{}: {:?}, swing {:.2} -> {:.2}, tempo {} -> {}, steps {}",
            preset.name,
            a.verdict(),
            original.swing(),
            built.project.swing(),
            original.tempo,
            built.project.tempo,
            if same { "identical" } else { "DIFFER" }
        );
    }
    Ok(())
}

fn print_report(a: &Analysis, built: &build::Built) {
    println!("{}", a.report());
    println!("{}", built.report());
}
