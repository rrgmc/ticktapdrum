//! TickTapDrum on Android.
//!
//! The crate builds to `libticktapdrum.so`, which the app's activity, a small subclass of
//! `android.app.NativeActivity` (see `ports/android`), loads. It is empty on every other target, so
//! `cargo build` and `cargo clippy` for the whole workspace still work on a desktop.

#[cfg(target_os = "android")]
mod picker;
#[cfg(target_os = "android")]
mod share;

#[cfg(target_os = "android")]
mod android {
    use std::path::PathBuf;

    use eframe::egui;
    use eframe::wgpu;
    // The same crate and version winit uses, so this is the type `NativeOptions::android_app` holds.
    use android_activity::AndroidApp;

    struct Host {
        app: dm_ui::DrumApp,
        android: AndroidApp,
        /// The screen size the insets were last read for, and when, in seconds.
        insets_for: Option<(egui::Vec2, f64)>,
        /// The activity's language tag the app was last given, such as `pt-BR`.
        locale: Option<String>,
    }

    /// The activity's language as a BCP 47 tag, such as `pt-BR`. `None` if nothing names one.
    ///
    /// The manifest lists `locale` in `configChanges`, so a change of language in the system
    /// settings, or of this app's own language on Android 13 and later, does not restart the
    /// activity. The language is read from the activity's resources through JNI: the copy of
    /// the configuration android-activity keeps comes from the activity's native asset
    /// manager, which on the emulator kept the old language after a change of this app's own
    /// language. That copy is the fallback.
    fn activity_locale(app: &AndroidApp) -> Option<String> {
        match resources_locale(app) {
            Ok(tag) if !tag.is_empty() && tag != "und" => return Some(tag),
            Ok(_) => {}
            Err(e) => log::warn!("could not read the language from the resources: {e}"),
        }
        let config = app.config();
        let language = config.language()?;
        Some(match config.country() {
            Some(country) => format!("{language}-{country}"),
            None => language,
        })
    }

    /// `getResources().getConfiguration().getLocales().get(0).toLanguageTag()`.
    fn resources_locale(app: &AndroidApp) -> jni::errors::Result<String> {
        with_activity(app, |env, activity| {
            let resources = env
                .call_method(
                    activity,
                    jni::jni_str!("getResources"),
                    jni::jni_sig!("()Landroid/content/res/Resources;"),
                    &[],
                )?
                .l()?;
            let config = env
                .call_method(
                    &resources,
                    jni::jni_str!("getConfiguration"),
                    jni::jni_sig!("()Landroid/content/res/Configuration;"),
                    &[],
                )?
                .l()?;
            let locales = env
                .call_method(
                    &config,
                    jni::jni_str!("getLocales"),
                    jni::jni_sig!("()Landroid/os/LocaleList;"),
                    &[],
                )?
                .l()?;
            let locale = env
                .call_method(
                    &locales,
                    jni::jni_str!("get"),
                    jni::jni_sig!("(I)Ljava/util/Locale;"),
                    &[jni::objects::JValue::Int(0)],
                )?
                .l()?;
            let tag = env
                .call_method(
                    &locale,
                    jni::jni_str!("toLanguageTag"),
                    jni::jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .l()?;
            let tag = env.cast_local::<jni::objects::JString>(tag)?;
            tag.try_to_string(env)
        })
    }

    /// How often the insets are read again while the size stays the same, in seconds. Turning
    /// the phone half a circle moves the camera cutout without changing the size.
    const INSETS_REFRESH: f64 = 1.0;
    /// Room kept at an edge that has no bar or cutout, in points.
    const EDGE: f32 = 8.0;

    impl eframe::App for Host {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let size = ui.ctx().content_rect().size();
            let now = ui.input(|i| i.time);
            let stale = match self.insets_for {
                Some((at_size, at_time)) => at_size != size || now - at_time > INSETS_REFRESH,
                None => true,
            };
            if stale
                && let Some(tag) = activity_locale(&self.android)
                && self.locale.as_ref() != Some(&tag)
            {
                log::info!("the system language changed to {tag}");
                self.app.set_system_locale(&tag);
                self.locale = Some(tag);
            }
            if stale && let Some(px) = system_insets(&self.android) {
                self.insets_for = Some((size, now));
                let points = ui.ctx().pixels_per_point();
                let side = |px: i32| (px as f32 / points).max(EDGE).round() as i8;
                self.app.set_padding(egui::Margin {
                    left: side(px[0]),
                    top: side(px[1]),
                    right: side(px[2]),
                    bottom: side(px[3]),
                });
            }
            self.app.ui(ui);
        }
    }

    /// eframe's wgpu defaults, except on the Android emulator.
    ///
    /// By default wgpu tries Vulkan first and OpenGL ES second, and phones render with Vulkan.
    /// The emulator's Vulkan driver (`gfxstream`) crashes inside the driver before the first
    /// frame, in `vkSetDebugUtilsObjectNameEXT` and then in `vkQueueSubmit`. Its OpenGL ES works.
    /// Choosing OpenGL ES after the fact is not enough: wgpu has by then attached the window to
    /// Vulkan, and EGL cannot attach to it again. So the emulator gets an OpenGL-only instance.
    fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
        let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
        if has_emulator_vulkan() {
            log::info!("the Vulkan driver is the emulator's, rendering with OpenGL ES");
            setup.instance_descriptor.backends = wgpu::Backends::GL;
        }
        eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: setup.into(),
            ..Default::default()
        }
    }

    /// Whether the Vulkan driver is the emulator's. Looks with a throwaway instance that has no
    /// window, so nothing is attached to the window yet.
    fn has_emulator_vulkan() -> bool {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::VULKAN;
        let instance = wgpu::Instance::new(desc);
        let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
        adapters.iter().any(|a| a.get_info().driver == "gfxstream")
    }

    /// Runs a future that native wgpu completes without waiting.
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        loop {
            if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
            std::thread::yield_now();
        }
    }

    /// Runs `f` with the JNI environment and the activity.
    pub(crate) fn with_activity<T>(
        app: &AndroidApp,
        f: impl FnOnce(&mut jni::Env<'_>, &jni::objects::JObject<'_>) -> jni::errors::Result<T>,
    ) -> jni::errors::Result<T> {
        // SAFETY: both pointers come from android-activity and stay valid while the app runs.
        let vm = unsafe { jni::JavaVM::from_raw(app.vm_as_ptr().cast()) };
        let activity = app.activity_as_ptr() as jni::sys::jobject;
        vm.attach_current_thread(|env: &mut jni::Env<'_>| {
            let activity = unsafe { jni::objects::JObject::from_raw(env, activity) };
            f(env, &activity)
        })
    }

    /// Calls a method that takes no arguments and returns an `int`.
    fn int_of(
        env: &mut jni::Env<'_>,
        obj: &jni::objects::JObject<'_>,
        name: &jni::strings::JNIStr,
    ) -> jni::errors::Result<i32> {
        env.call_method(obj, name, jni::jni_sig!("()I"), &[])?.i()
    }

    /// The room the status bar, the navigation bar and the camera cutout take at each edge, in
    /// pixels: left, top, right, bottom. `None` until the window is attached, or on an error.
    ///
    /// eframe does not report these on Android, and the app draws under the bars. They change
    /// with the rotation: in landscape the cutout sits on the left or the right.
    fn system_insets(app: &AndroidApp) -> Option<[i32; 4]> {
        let result = with_activity(app, |env, activity| {
            let window = env
                .call_method(
                    activity,
                    jni::jni_str!("getWindow"),
                    jni::jni_sig!("()Landroid/view/Window;"),
                    &[],
                )?
                .l()?;
            let view = env
                .call_method(
                    &window,
                    jni::jni_str!("getDecorView"),
                    jni::jni_sig!("()Landroid/view/View;"),
                    &[],
                )?
                .l()?;
            let insets = env
                .call_method(
                    &view,
                    jni::jni_str!("getRootWindowInsets"),
                    jni::jni_sig!("()Landroid/view/WindowInsets;"),
                    &[],
                )?
                .l()?;
            if insets.as_raw().is_null() {
                return Ok(None);
            }
            let sdk = env
                .get_static_field(
                    jni::jni_str!("android/os/Build$VERSION"),
                    jni::jni_str!("SDK_INT"),
                    jni::jni_sig!("I"),
                )?
                .i()?;
            let mut px = [0; 4];
            if sdk >= 30 {
                // WindowInsets.getInsets(Type.systemBars() | Type.displayCutout()).
                let mut kinds = 0;
                for name in [jni::jni_str!("systemBars"), jni::jni_str!("displayCutout")] {
                    kinds |= env
                        .call_static_method(
                            jni::jni_str!("android/view/WindowInsets$Type"),
                            name,
                            jni::jni_sig!("()I"),
                            &[],
                        )?
                        .i()?;
                }
                let sides = env
                    .call_method(
                        &insets,
                        jni::jni_str!("getInsets"),
                        jni::jni_sig!("(I)Landroid/graphics/Insets;"),
                        &[jni::JValue::Int(kinds)],
                    )?
                    .l()?;
                let names = [
                    jni::jni_str!("left"),
                    jni::jni_str!("top"),
                    jni::jni_str!("right"),
                    jni::jni_str!("bottom"),
                ];
                for (side, name) in px.iter_mut().zip(names) {
                    *side = env.get_field(&sides, name, jni::jni_sig!("I"))?.i()?;
                }
                return Ok(Some(px));
            }
            // Before Android 11 the system window insets cover the bars. On Android 9 and 10
            // the cutout is read as well, and the larger of the two kept.
            let names = [
                jni::jni_str!("getSystemWindowInsetLeft"),
                jni::jni_str!("getSystemWindowInsetTop"),
                jni::jni_str!("getSystemWindowInsetRight"),
                jni::jni_str!("getSystemWindowInsetBottom"),
            ];
            for (side, name) in px.iter_mut().zip(names) {
                *side = int_of(env, &insets, name)?;
            }
            if sdk >= 28 {
                let cutout = env
                    .call_method(
                        &insets,
                        jni::jni_str!("getDisplayCutout"),
                        jni::jni_sig!("()Landroid/view/DisplayCutout;"),
                        &[],
                    )?
                    .l()?;
                if !cutout.as_raw().is_null() {
                    let names = [
                        jni::jni_str!("getSafeInsetLeft"),
                        jni::jni_str!("getSafeInsetTop"),
                        jni::jni_str!("getSafeInsetRight"),
                        jni::jni_str!("getSafeInsetBottom"),
                    ];
                    for (side, name) in px.iter_mut().zip(names) {
                        *side = (*side).max(int_of(env, &cutout, name)?);
                    }
                }
            }
            Ok(Some(px))
        });
        match result {
            Ok(px) => px,
            Err(e) => {
                log::warn!("could not read the screen insets: {e}");
                None
            }
        }
    }

    /// Keeps the activity in landscape (either way up, by the sensor) or lets it turn freely,
    /// through `Activity.setRequestedOrientation`.
    fn set_landscape(app: &AndroidApp, on: bool) {
        // ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE and SCREEN_ORIENTATION_UNSPECIFIED.
        let orientation = if on { 6 } else { -1 };
        let result = with_activity(app, |env, activity| {
            env.call_method(
                activity,
                jni::jni_str!("setRequestedOrientation"),
                jni::jni_sig!("(I)V"),
                &[jni::JValue::Int(orientation)],
            )?;
            Ok(())
        });
        match result {
            Ok(()) => log::info!(
                "orientation: landscape lock {}",
                if on { "on" } else { "off" }
            ),
            Err(e) => log::warn!("could not set the orientation: {e}"),
        }
    }

    /// Keeps the screen on while the app plays, through the window's `FLAG_KEEP_SCREEN_ON`.
    /// android-activity passes the change to the activity's UI thread, where Android needs it.
    fn keep_awake(app: &AndroidApp, on: bool) {
        use android_activity::WindowManagerFlags;
        let flag = WindowManagerFlags::KEEP_SCREEN_ON;
        let none = WindowManagerFlags::empty();
        log::info!("keep the screen on: {on}");
        if on {
            app.set_window_flags(flag, none);
        } else {
            app.set_window_flags(none, flag);
        }
    }

    /// Sends the app to the background, as Back does at the top of an Android 12 or later app.
    /// The activity is not destroyed, so the app carries on where it was when it comes back.
    fn move_to_background(app: &AndroidApp) {
        let result = with_activity(app, |env, activity| {
            env.call_method(
                activity,
                jni::jni_str!("moveTaskToBack"),
                jni::jni_sig!("(Z)Z"),
                &[jni::JValue::Bool(jni::sys::JNI_TRUE)],
            )?;
            Ok(())
        });
        if let Err(e) = result {
            log::warn!("could not move to the background: {e}");
        }
    }

    /// Sends `log` output to logcat, and ends the process when Android destroys the activity.
    ///
    /// winit 0.30 ignores the activity's destruction: it only logs "TODO: forward onDestroy
    /// notification to application". Meanwhile android-activity holds Android's main thread
    /// until the Rust thread stops, which it never does, so the next launch in the same process
    /// hangs on the start-up screen. Ending the process when that message comes makes the next
    /// launch start a fresh one. Nothing is lost: the app autosaves as soon as it loses focus,
    /// which comes before the activity is destroyed. See `docs/upstream-fixes.md`.
    struct Logger(android_logger::AndroidLogger);

    impl log::Log for Logger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            self.0.enabled(metadata)
        }

        fn log(&self, record: &log::Record<'_>) {
            self.0.log(record);
            if record.target().starts_with("winit")
                && record.args().to_string().contains("onDestroy")
            {
                self.0.log(
                    &log::Record::builder()
                        .args(format_args!(
                            "the activity was destroyed, ending the process"
                        ))
                        .level(log::Level::Info)
                        .target(module_path!())
                        .module_path(Some(module_path!()))
                        .build(),
                );
                std::process::exit(0);
            }
        }

        fn flush(&self) {
            self.0.flush();
        }
    }

    /// The entry point `android-activity` calls on its own thread once the activity starts.
    #[unsafe(no_mangle)]
    fn android_main(app: AndroidApp) {
        let logger = android_logger::AndroidLogger::new(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("ticktapdrum"),
        );
        if log::set_boxed_logger(Box::new(Logger(logger))).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
        // A panic on this thread would otherwise vanish: stderr goes nowhere on Android.
        std::panic::set_hook(Box::new(|info| log::error!("panic: {info}")));

        // The app's private files folder. It always exists for an installed app; the fallback only
        // keeps the type simple.
        let data_dir = app
            .internal_data_path()
            .unwrap_or_else(|| PathBuf::from("."));
        log::info!("starting, data folder {}", data_dir.display());
        // Exports go to the app's external files folder, which a file manager or a USB
        // connection can reach, unlike the private data folder. Falls back to the private one.
        let export_dir = app
            .external_data_path()
            .unwrap_or_else(|| data_dir.clone())
            .join("exports");

        // The interface follows the activity's language. sys-locale cannot ask Android for it.
        let locale = activity_locale(&app);
        log::info!("language {}", locale.as_deref().unwrap_or("unknown"));

        let orientation_app = app.clone();
        let options = eframe::NativeOptions {
            android_app: Some(app),
            wgpu_options: wgpu_options(),
            ..Default::default()
        };
        let result = eframe::run_native(
            "TickTapDrum",
            options,
            Box::new(move |cc| {
                if let Some(render_state) = &cc.wgpu_render_state {
                    let info = render_state.adapter.get_info();
                    log::info!("rendering with {:?} on {}", info.backend, info.name);
                }
                // Room for a typical status bar on top and gesture bar below, until the real
                // insets can be read (see `system_insets`).
                let padding = egui::Margin {
                    left: 8,
                    right: 8,
                    top: 36,
                    bottom: 24,
                };
                let lock_app = orientation_app.clone();
                let back_app = orientation_app.clone();
                let awake_app = orientation_app.clone();
                // The export folder stays as the fallback; the interface prefers the picker.
                let mut app = dm_ui::DrumApp::new(data_dir)
                    .with_padding(padding)
                    .with_export_dir(export_dir)
                    .with_file_picker(crate::picker::pick)
                    .with_orientation_lock(move |on| set_landscape(&lock_app, on))
                    .with_back_leaves(move || move_to_background(&back_app))
                    .with_keep_awake(move |on| keep_awake(&awake_app, on));
                if let Some(tag) = &locale {
                    app = app.with_system_locale(tag);
                }
                crate::picker::install(orientation_app.clone(), app.pick_answers());
                // Last, once the interface exists: a share that started the app is handed on now.
                crate::share::install(orientation_app.clone(), app.incoming_files());
                Ok(Box::new(Host {
                    app,
                    android: orientation_app,
                    insets_for: None,
                    locale,
                }))
            }),
        );
        if let Err(err) = result {
            log::error!("eframe stopped with an error: {err}");
        }
    }
}
