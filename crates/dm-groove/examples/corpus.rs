//! Imports groove files and packs from disk and prints how many were accepted and why the rest
//! were refused. For checking the rule on real packs.
//!
//! Usage: `cargo run --release -p dm-groove --example corpus -- <file or folder>...`
//!
//! Each argument is counted on its own. A folder is searched for files in all its subfolders;
//! each file found is imported as if opened on its own, so a folder holding both a zip and its
//! unpacked files counts them twice.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.is_empty() {
        eprintln!("usage: corpus <file or folder>...");
        std::process::exit(2);
    }
    for path in args {
        let mut files = Vec::new();
        collect(&path, &mut files);
        files.sort();
        let mut accepted = 0;
        let mut refused: BTreeMap<String, usize> = BTreeMap::new();
        let mut skipped = 0;
        let mut errors: BTreeMap<String, usize> = BTreeMap::new();
        for file in &files {
            let Ok(bytes) = std::fs::read(file) else {
                *errors.entry("unreadable".into()).or_default() += 1;
                continue;
            };
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            match dm_groove::import(&name, &bytes) {
                Ok(import) => {
                    accepted += import.grooves.len();
                    skipped += import.other_mappings_skipped;
                    for (_, why) in import.refused {
                        *refused.entry(format!("{why:?}")).or_default() += 1;
                    }
                }
                Err(e) => *errors.entry(format!("{e:?}")).or_default() += 1,
            }
        }
        let total = accepted + refused.values().sum::<usize>();
        println!("{}", path.display());
        println!("  {accepted} of {total} MIDI files accepted; {skipped} other mappings skipped");
        println!("  refused: {refused:?}");
        println!("  files not imported: {errors:?}");
    }
}

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            collect(&entry.path(), out);
        }
    } else if path.is_file() {
        out.push(path.to_owned());
    }
}
