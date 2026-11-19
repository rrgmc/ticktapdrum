# Fixes we could make upstream

The iOS port (milestone 7) and the Android port work around several problems in the libraries
they use. Each would
be better fixed in the library itself, so that the workaround here could go. This document lists
them, with what a fix would look like. The versions are the ones this repository pins.

Each entry says:

- **Where:** the library and version, and the code involved.
- **Problem:** what goes wrong, and how it shows.
- **Our workaround:** what this repository does instead, and where.
- **Upstream fix:** what the library could change.
- **Upstream status:** existing issues and pull requests, as of 2026-11-19.

## 1. winit: no scene lifecycle on iOS

- **Where:** winit 0.30.13, `src/platform_impl/ios/event_loop.rs` and `window.rs`.
- **Problem:** apps built with the iOS 27 SDK must adopt the UIScene life cycle. Without a scene
  manifest, UIKit refuses to launch them: "UIScene life cycle is required for apps built with
  this SDK". winit 0.30 calls `UIApplicationMain` with no delegate and creates its `UIWindow` with
  `initWithFrame:`, never giving it a `windowScene`. A window without a scene is never shown.
- **Our workaround:** the `Info.plist` declares one scene with a delegate class defined in
  `ports/ios/TickTapDrum/main.m`. Rust hands winit's view to `main.m` as soon as eframe has made
  the window (`dm_ios_attach_view`), and `main.m` sets `window.windowScene` and makes it key.
- **Upstream fix:** winit creates its windows inside a `UIWindowScene`: a scene delegate class
  of its own, windows created with `initWithWindowScene:`, and `Resumed` or
  `can_create_surfaces` sent once a scene connects. winit's next major version (0.31) is
  reworking the iOS backend; check whether it already does this before writing a patch. A
  backport to 0.30 would let eframe 0.37 apps run unchanged.
- **Upstream status:** open. Issue
  [winit#4224](https://github.com/rust-windowing/winit/issues/4224) tracks it, and a comment
  there reports that the iOS 27.2 SDK refuses to launch such apps. PR
  [winit#4723](https://github.com/rust-windowing/winit/pull/4723), against `master`, adds
  support: an app that declares a scene manifest gets `can_create_surfaces` once the first
  scene connects, and its windows are attached to that scene, with no scene delegate needed.
  The author keeps a 0.30.13 backport on a branch but has not opened a PR against `v0.30.x`.
  Once eframe uses a winit with this change, the scene delegate in `main.m` and
  `dm_ios_attach_view` may no longer be needed.

## 2. eframe and winit: drawing surface the size of the safe area

- **Where:** winit 0.30.13, `Window::inner_size` on iOS (`src/platform_impl/ios/window.rs`),
  which returns the safe area; eframe 0.37.0, `src/native/wgpu_integration.rs`, which sizes the
  wgpu surface from `window.inner_size()`; egui-winit 0.37.0, `screen_size_in_pixels`, which
  uses `outer_size()` on iOS.
- **Problem:** egui lays out the whole window, but the surface is only as tall as the safe area.
  The Metal layer stretches the smaller picture over the whole window, so nothing is drawn where
  egui thinks it is. On an iPhone 17 the picture was stretched by 874 / 778, and every tap landed
  about an eighth of the screen height too low. Any eframe app on a device with a notch, a
  Dynamic Island or a home indicator has this problem.
- **Our workaround:** `main.m` overrides `safeAreaInsets` on winit's own `UIWindow` subclass to
  return zero, so `inner_size` and `outer_size` agree. The app then keeps clear of the screen
  edges itself: `apps/dm-ios` reads the real insets every frame (`dm_ios_safe_area`) and passes
  them to `DrumApp::set_padding`.
- **Upstream fix:** eframe sizes the surface from the same size egui-winit uses for the screen
  rectangle, `outer_size()` on iOS. That is a one-line change in the `Resized` handler and in
  surface creation. In the longer term, winit 0.31's separate `surface_size` and `safe_area`
  remove the ambiguity.
- **Upstream status:** fixed, not yet released. PR
  [egui#8840](https://github.com/emilk/egui/pull/8840), merged on 2026-11-09, sizes the surface
  from `outer_size()` on iOS, both when it is created and on `Resized`. The latest release,
  0.37.0, predates it. After upgrading to a release that includes it, drop the `safeAreaInsets`
  override in `main.m`; keep passing the insets to `DrumApp::set_padding`.

## 3. egui-wgpu: device limits the iOS simulator does not offer

- **Where:** egui-wgpu 0.37.0, `WgpuSetupCreateNew::without_display_handle` in `src/setup.rs`.
- **Problem:** the default device descriptor asks for `wgpu::Limits::default()`, the full
  WebGPU limits. The iOS simulator's Metal device offers 15 inter-stage shader variables where
  WebGPU asks for 16. The device request fails, and eframe exits before the first frame with
  "Limit 'max_inter_stage_shader_variables' value 16 is better than allowed 15".
- **Our workaround:** `apps/dm-ios` passes its own device descriptor, which asks for the
  adapter's own limits (`adapter.limits()`).
- **Upstream fix:** egui-wgpu asks for `wgpu::Limits::downlevel_defaults()` raised to what egui
  needs (a large `max_texture_dimension_2d`), or for the adapter's limits. egui itself needs far
  less than full WebGPU.
- **Upstream status:** issue open, no PR.
  [egui#8651](https://github.com/emilk/egui/issues/8651) reports the same
  `max_inter_stage_shader_variables` failure on a Raspberry Pi 5, and a comment there proposes
  using `adapter.limits()`. The iOS simulator is not mentioned yet; adding it to that issue
  would show the problem is not specific to one device.

## 4. egui-winit: safe area read from the key window only

- **Where:** egui-winit 0.37.0, `src/safe_area.rs`.
- **Problem:** the insets come from the key window of the first foreground scene, and they
  ignore the window controls that iPadOS 26 and later draws in the top-left corner of an app
  running in a resizable window. Those controls covered the app's Play button.
- **Our workaround:** `dm_ios_safe_area` in `main.m` also takes the corner-adapted safe area
  (`edgeInsetsForLayoutRegion:` with `UIViewLayoutRegion
  safeAreaLayoutRegionWithCornerAdaptation:`, iOS 26).
- **Upstream fix:** egui-winit reads the insets from the view that egui draws into, using the
  corner-adapted region where the system has it.
- **Upstream status:** nothing filed. The current code came from
  [egui#7578](https://github.com/emilk/egui/pull/7578), merged in 2025.

## 5. winit: the Android activity's destruction is ignored

- **Where:** winit 0.30.13, `src/platform_impl/android/mod.rs` (`MainEvent::Destroy`), with
  android-activity 0.6.1, `src/native_activity/glue.rs` (`notify_destroyed`).
- **Problem:** when Android destroys the activity but keeps the process, winit only logs
  "TODO: forward onDestroy notification to application" and keeps its loop running.
  android-activity's `onDestroy` waits for the Rust thread to stop, which it never does, so
  Android's main thread is held and the next launch in the same process hangs on the start-up
  screen.
- **Our workaround:** `Logger` in `apps/dm-android/src/lib.rs` ends the process when that
  message is logged. winit logs through `tracing`, so `dm-android` turns on tracing's `log`
  feature to see it. Back moves the app to the background instead of letting Android destroy
  the activity (`enableOnBackInvokedCallback="false"` in the manifest, and `with_back_leaves`).
  Matching a log message is fragile: a winit update that changes the text silently brings the
  hang back.
- **Upstream fix:** winit ends its event loop on `MainEvent::Destroy`, or sends the application
  an event it can end the loop on, so `android_main` returns and android-activity can finish.
  eframe then needs to return from `run_native` cleanly, so the host can exit or wait for the
  next activity.
- **Upstream status:** not confirmed as of 2026-11-19. One search found no winit issue for this
  case. A reproduction of a related reopen bug, with GameActivity, is at
  [SnowyCoder/winit-bug-reopen-example](https://github.com/SnowyCoder/winit-bug-reopen-example).

## Not upstream, but related

- **cpal's CoreAudio backend sets up no audio session** on iOS. That is a choice rather than a
  defect: the app decides the category. `main.m` sets `AVAudioSessionCategoryPlayback` and
  activates the session before the audio starts, as karaokemachine does. A note in cpal's iOS
  documentation would save the next person the search. cpal 0.18.2 activates the session but
  never sets its category. No issue asks for such a note.
