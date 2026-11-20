//! Imports every Yamaha style under a folder and prints how many sections were accepted, why
//! the rest were refused, and which keyboard kits the accepted ones use. For checking the style
//! import and its key map on real styles.
//!
//! Usage: `cargo run --release -p dm-groove --example styles -- <folder>`
//!
//! A style is any file with a style's extension (`.sty`, `.prs`, `.sst`, `.scp`, `.fps`,
//! `.bcs`) in the folder or its subfolders. A file with such a name that is not a style, by its
//! content, is counted apart.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const EXTENSIONS: [&str; 6] = ["sty", "prs", "sst", "scp", "fps", "bcs"];

fn main() {
    let Some(folder) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: styles <folder>");
        std::process::exit(2);
    };
    let mut files = Vec::new();
    collect(&folder, &mut files);
    files.sort();

    let mut styles = 0;
    let mut not_styles = Vec::new();
    let mut with_accepted = 0;
    let mut accepted = 0;
    let mut refused: BTreeMap<String, usize> = BTreeMap::new();
    // Accepted and refused sections by the kind of section: Intro, Main, Fill, Break, Ending.
    let mut by_kind: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut kits: BTreeMap<String, usize> = BTreeMap::new();
    let mut similar: BTreeMap<String, usize> = BTreeMap::new();
    for file in &files {
        let shown = file.strip_prefix(&folder).unwrap_or(file).display();
        let Ok(bytes) = std::fs::read(file) else {
            not_styles.push(format!("{shown}: unreadable"));
            continue;
        };
        if !dm_groove::is_style(&bytes) {
            not_styles.push(format!("{shown}: not a style"));
            continue;
        }
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        let import = match dm_groove::import(&name, &bytes) {
            Ok(import) => import,
            Err(error) => {
                not_styles.push(format!("{shown}: {error:?}"));
                continue;
            }
        };
        styles += 1;
        if !import.grooves.is_empty() {
            with_accepted += 1;
        }
        accepted += import.grooves.len();
        for groove in &import.grooves {
            by_kind.entry(kind(&groove.name)).or_default().0 += 1;
            let kit = groove
                .kit
                .clone()
                .unwrap_or_else(|| "(not in the table)".into());
            *kits.entry(kit).or_default() += 1;
            let program = groove
                .similar_kit
                .map_or_else(|| "(none)".into(), |p| p.to_string());
            *similar.entry(program).or_default() += 1;
        }
        for (path, refusal) in &import.refused {
            by_kind.entry(kind(path)).or_default().1 += 1;
            *refused.entry(format!("{refusal:?}")).or_default() += 1;
        }
    }

    let refused_total: usize = refused.values().sum();
    println!(
        "{}: {} files with a style's extension",
        folder.display(),
        files.len()
    );
    println!("  styles read: {styles}");
    println!("  styles with at least one section accepted: {with_accepted}");
    println!(
        "  sections: {} in all, {accepted} accepted, {refused_total} refused",
        accepted + refused_total
    );
    for (reason, count) in &refused {
        println!("    refused, {reason}: {count}");
    }
    println!("  by kind of section (accepted / refused):");
    for (kind, (yes, no)) in &by_kind {
        println!("    {kind:8} {yes:5} / {no}");
    }
    println!("  accepted sections by keyboard kit:");
    let mut kits: Vec<_> = kits.into_iter().collect();
    kits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    for (kit, count) in &kits {
        println!("    {count:5}  {kit}");
    }
    println!("  accepted sections by similar General MIDI kit program:");
    for (program, count) in &similar {
        println!("    {count:5}  {program}");
    }
    if !not_styles.is_empty() {
        println!("  not read as styles: {}", not_styles.len());
        for line in &not_styles {
            println!("    {line}");
        }
    }
}

/// The kind of section a groove or refusal name ends in.
fn kind(name: &str) -> &'static str {
    if name.ends_with("Fill In BA") {
        "Break"
    } else if name.contains(" Fill In ") {
        "Fill"
    } else if name.contains(" Main ") {
        "Main"
    } else if name.contains(" Intro ") {
        "Intro"
    } else if name.contains(" Ending ") {
        "Ending"
    } else {
        "other"
    }
}

/// Every file with a style's extension at or under `path`.
fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            collect(&entry.path(), out);
        }
    } else if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
    {
        out.push(path.to_owned());
    }
}
