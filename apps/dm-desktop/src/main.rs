//! TickTapDrum on Windows, macOS and Linux.

// On Windows the app is a windowed program in every build, so starting it opens no console
// window. The log then has nowhere to go when the app is started from Explorer.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;

/// Where the app keeps its bank and other data. `DM_DATA_DIR` overrides it, so a development run
/// never touches a real install's data. Otherwise it is the platform's usual place for app data:
/// `%LOCALAPPDATA%` on Windows, `~/Library/Application Support` on macOS, and the XDG data folder
/// (`~/.local/share` by default) on Linux.
fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("DM_DATA_DIR") {
        return PathBuf::from(dir);
    }
    platform_data_base()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ticktapdrum")
}

#[cfg(windows)]
fn platform_data_base() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn platform_data_base() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_data_base() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

struct Host {
    app: dm_ui::DrumApp,
}

impl eframe::App for Host {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            // Shown only until the first frame, when DrumApp sets the title in its language.
            .with_title("TickTapDrum")
            .with_inner_size([960.0, 640.0])
            // Matches the Linux desktop file's name, so the dock shows the window under its icon.
            .with_app_id("ticktapdrum")
            // Groove files and packs can be dropped on the window to add them.
            .with_drag_and_drop(true)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png"))
                    .unwrap_or_default(),
            ),
        ..Default::default()
    };
    let mut app = dm_ui::DrumApp::new(data_dir());
    // Development shortcuts. `DM_SF2` plays any SoundFont without installing the bank;
    // `DM_BANK_ZIP` installs the bank from a local copy of its archive instead of downloading it.
    if let Some(path) = std::env::var_os("DM_BANK_ZIP") {
        app = app.with_local_archive(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("DM_SF2") {
        app = app.with_soundfont_file(PathBuf::from(path));
    }
    eframe::run_native(
        "TickTapDrum",
        options,
        Box::new(move |_cc| Ok(Box::new(Host { app }))),
    )
}
