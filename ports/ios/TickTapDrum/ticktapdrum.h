// The one function the Rust library (apps/dm-ios) exports to the app.
#pragma once

// Runs TickTapDrum. `data_dir` holds the bank, the projects and the autosave; `export_dir`
// receives MIDI and WAV exports. Both are copied before use. Call it on the main thread, outside
// UIApplicationMain: winit starts UIApplicationMain itself, so this never returns.
int dm_ios_main(const char *data_dir, const char *export_dir);

// Defined in main.m and called by Rust on its first frame: gives the UIWindow holding `view`, a
// UIView, to the app's scene.
void dm_ios_attach_view(void *view);

// Defined in main.m: the screen's safe-area insets in points (notch, home indicator), which the
// app keeps clear itself because main.m hides them from winit.
void dm_ios_safe_area(double *top, double *left, double *bottom, double *right);

// Defined in main.m: 1 keeps the screen on, 0 lets it dim and lock again.
void dm_ios_keep_awake(int on);

// Defined in main.m: 1 on an iPad, 0 on an iPhone.
int dm_ios_is_ipad(void);
