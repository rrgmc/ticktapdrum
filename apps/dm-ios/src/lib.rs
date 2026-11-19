//! TickTapDrum on iPhone and iPad.
//!
//! The crate builds to `libticktapdrum.a`, which the app in `ports/ios` links. Its `main.m` calls
//! [`dm_ios_main`]. The crate is empty on every other target, so `cargo build` and `cargo clippy`
//! for the whole workspace still work on a desktop.

#[cfg(target_os = "ios")]
mod ios {
    use std::ffi::{CStr, OsStr, c_char};
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    use eframe::egui;

    unsafe extern "C" {
        /// In `main.m`: gives the UIWindow holding `view` to the app's scene.
        fn dm_ios_attach_view(view: *mut std::ffi::c_void);
        /// In `main.m`: the safe-area insets in points.
        fn dm_ios_safe_area(top: *mut f64, left: *mut f64, bottom: *mut f64, right: *mut f64);
        /// In `main.m`: 1 keeps the screen on, 0 lets it dim and lock again.
        fn dm_ios_keep_awake(on: i32);
        /// In `main.m`: 1 on an iPad, 0 on an iPhone.
        fn dm_ios_is_ipad() -> i32;
        /// The C library's, which Apple routes to the system log.
        fn syslog(priority: i32, format: *const c_char, ...);
    }

    /// Hands winit's view to `main.m` as soon as eframe has made the window. winit 0.30 makes its
    /// UIWindow without a scene, and apps built with the iOS 27 SDK show only windows that have
    /// one. It cannot wait for the first frame: winit draws only once the window shows.
    fn attach(window: &impl raw_window_handle::HasWindowHandle) {
        use raw_window_handle::RawWindowHandle;
        match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::UiKit(handle)) => {
                // SAFETY: the view is winit's live UIView, and this runs on the main thread.
                unsafe { dm_ios_attach_view(handle.ui_view.as_ptr()) }
            }
            other => log::error!("no UIKit view to attach: {other:?}"),
        }
    }

    struct Host {
        app: dm_ui::DrumApp,
        /// The padding last used, so a change is logged once.
        padding: Option<egui::Margin>,
    }

    impl eframe::App for Host {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let padding = padding();
            if self.padding != Some(padding) {
                log::info!("padding {padding:?}");
                self.padding = Some(padding);
            }
            self.app.set_padding(padding);
            self.app.ui(ui);
        }
    }

    /// Room around the app: the safe area (the notch, the home indicator, the rounded corners),
    /// plus a small margin. Read every frame, because it changes when the device rotates.
    ///
    /// main.m hides the safe area from winit, so that eframe's drawing surface covers the whole
    /// window as egui's layout does, and egui-winit's own safe-area handling sees none either.
    fn padding() -> egui::Margin {
        let (mut top, mut left, mut bottom, mut right) = (0.0, 0.0, 0.0, 0.0);
        // SAFETY: four valid pointers, on the main thread.
        unsafe { dm_ios_safe_area(&mut top, &mut left, &mut bottom, &mut right) };
        // Margin holds whole points up to 127; insets are at most about 60.
        let side = |inset: f64| (inset.ceil() as i32 + 8).clamp(0, i8::MAX as i32) as i8;
        egui::Margin {
            left: side(left),
            right: side(right),
            top: side(top),
            bottom: side(bottom),
        }
    }

    /// Turns the idle timer off while the app plays, so the screen stays on.
    fn keep_awake(on: bool) {
        log::info!("keep the screen on: {on}");
        // SAFETY: an int, on the main thread, where eframe draws.
        unsafe { dm_ios_keep_awake(i32::from(on)) }
    }

    /// eframe's wgpu defaults, except that the device asks for the adapter's own limits.
    ///
    /// By default egui-wgpu asks for wgpu's full WebGPU limits. The iOS simulator's Metal device
    /// offers 15 inter-stage shader variables where WebGPU asks for 16, so the device request
    /// fails and eframe exits before the first frame. egui needs far less than either.
    fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
        let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
        setup.device_descriptor = std::sync::Arc::new(|adapter| eframe::wgpu::DeviceDescriptor {
            label: Some("egui wgpu device"),
            required_limits: adapter.limits(),
            ..Default::default()
        });
        eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup.into(),
            ..Default::default()
        }
    }

    struct Stderr;

    impl log::Log for Stderr {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        // stderr shows in Xcode's console and in `simctl launch --console`. syslog reaches the
        // system log, which `log show` and Console.app read, also when nothing captures stderr,
        // as under a UI test or on a device away from Xcode.
        fn log(&self, record: &log::Record) {
            let line = format!(
                "ticktapdrum {} {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
            eprintln!("{line}");
            let priority = match record.level() {
                log::Level::Error => 3,
                log::Level::Warn => 4,
                _ => 5,
            };
            if let Ok(line) = std::ffi::CString::new(line) {
                // SAFETY: "%s" with one NUL-terminated string.
                unsafe { syslog(priority, c"%s".as_ptr(), line.as_ptr()) };
            }
        }
        fn flush(&self) {}
    }

    /// Copies a path `main.m` passed. Its memory belongs to an autorelease pool, so nothing may
    /// keep the pointer.
    ///
    /// # Safety
    ///
    /// `path` is null or a NUL-terminated string.
    unsafe fn path_from(path: *const c_char) -> Option<PathBuf> {
        if path.is_null() {
            return None;
        }
        // SAFETY: the caller promises a NUL-terminated string.
        let bytes = unsafe { CStr::from_ptr(path) }.to_bytes();
        (!bytes.is_empty()).then(|| PathBuf::from(OsStr::from_bytes(bytes)))
    }

    fn run(data_dir: PathBuf, export_dir: PathBuf) {
        log::info!(
            "starting, data folder {}, exports {}",
            data_dir.display(),
            export_dir.display()
        );
        let result = eframe::run_native(
            "TickTapDrum",
            eframe::NativeOptions {
                wgpu_options: wgpu_options(),
                ..Default::default()
            },
            Box::new(move |cc| {
                attach(cc);
                if let Some(render_state) = &cc.wgpu_render_state {
                    let info = render_state.adapter.get_info();
                    log::info!("rendering with {:?} on {}", info.backend, info.name);
                }
                // The Files app shows Documents as this, because the Info.plist sets
                // UIFileSharingEnabled; the folder's path means nothing to people.
                // SAFETY: no arguments, on the main thread.
                let place = if unsafe { dm_ios_is_ipad() } != 0 {
                    "app-export-place-ipad"
                } else {
                    "app-export-place-iphone"
                };
                // The padding follows the safe area; Host::ui sets it every frame.
                Ok(Box::new(Host {
                    app: dm_ui::DrumApp::new(data_dir)
                        .with_export_dir(export_dir)
                        .with_export_place(place)
                        .with_keep_awake(keep_awake),
                    padding: None,
                }))
            }),
        );
        if let Err(err) = result {
            log::error!("eframe stopped with an error: {err}");
        }
    }

    /// The entry point `main.m` calls on the main thread. winit calls `UIApplicationMain` inside
    /// it, so it does not return while the app runs.
    ///
    /// # Safety
    ///
    /// Both arguments are null or NUL-terminated paths.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn dm_ios_main(
        data_dir: *const c_char,
        export_dir: *const c_char,
    ) -> i32 {
        log::set_logger(&Stderr).ok();
        log::set_max_level(log::LevelFilter::Info);
        // SAFETY: passed on from the caller.
        let (data_dir, export_dir) = unsafe { (path_from(data_dir), path_from(export_dir)) };
        let (Some(data_dir), Some(export_dir)) = (data_dir, export_dir) else {
            log::error!("main.m passed no data or export folder");
            return 1;
        };
        // A panic must not unwind into Objective-C: that aborts with no message.
        match std::panic::catch_unwind(move || run(data_dir, export_dir)) {
            Ok(()) => 0,
            Err(_) => {
                log::error!("the app panicked");
                1
            }
        }
    }
}
