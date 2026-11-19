//! Installs the bank into a folder from the network, as the app does, and prints progress.
//!
//! `cargo run --release -p dm-bank --example install -- <folder>`

use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let dir = std::env::args_os().nth(1).expect("usage: install <folder>");
    let started = Instant::now();
    let cancel = AtomicBool::new(false);
    let mut last = String::new();
    let last = std::cell::RefCell::new(&mut last);
    let report = |p: dm_bank::Progress| {
        let line = match p {
            dm_bank::Progress::Downloading { done, total } => {
                format!("downloading {}%", done * 100 / total.max(1))
            }
            dm_bank::Progress::Extracting { done, total } => {
                format!("extracting {}%", done * 100 / total.max(1))
            }
            other => format!("{other:?}"),
        };
        let mut last = last.borrow_mut();
        if **last != line && (line.ends_with("0%") || !line.contains('%')) {
            println!("{:>6.1}s {line}", started.elapsed().as_secs_f64());
            **last = line;
        }
    };
    match dm_bank::install(dir.as_ref(), &dm_bank::COLOMBO, &report, &cancel) {
        Ok(path) => println!(
            "installed {} in {:.1}s",
            path.display(),
            started.elapsed().as_secs_f64()
        ),
        Err(e) => {
            eprintln!("failed: {e}");
            std::process::exit(1);
        }
    }
}
