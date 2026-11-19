# Building TickTapDrum

This file is for developers. To install and use the app, see [`README.md`](README.md).

The common commands are also tasks for [Task](https://taskfile.dev) (`winget install Task.Task`,
`brew install go-task`). `task` lists them; `task run`, `task check` and `task android` are the
usual three. Task is optional: every task forwards to a command shown below.

## Desktop

```sh
cargo run -p dm-desktop
```

On the first start the app offers to download its instrument bank. It keeps the bank, the saved
projects and the autosave in its data folder, which [`README.md`](README.md) lists for each
system.

### macOS

Install the Xcode command line tools (`xcode-select --install`), Rust through rustup, and
optionally Task (`brew install go-task`). Then `task run`, or the cargo command above.

The macOS build has been checked on Apple silicon: installing the bank, playing a preset, saving,
the autosave, and opening and saving files through the system dialogs. See "Interface" in
[`docs/DESIGN.md`](docs/DESIGN.md) for what was found.

Three environment variables help during development:

| Variable | Effect |
|---|---|
| `DM_DATA_DIR` | Uses this folder for the bank instead of the normal data folder. |
| `DM_SF2` | Plays this `.sf2` file directly and skips the bank. |
| `DM_BANK_ZIP` | Installs the bank from this local copy of its archive instead of downloading it. The copy must match the pinned digest. |

MIDI out (the "MIDI output" setting) is the Cargo feature `midi`, on by default in the desktop
and iOS apps and off on Android. `cargo run -p dm-desktop --no-default-features` builds without
it; the setting is then not offered, and neither `dm-midi` nor midir is built. To try MIDI out
without the interface, `cargo run --release -p dm-midi --example ports` lists the outputs, and
`DM_SF2=<bank.sf2> cargo run --release -p dm-midi --example engine -- "<output name>"` plays a
beat through the engine to that output and reports the timing.

To install the bank into a folder without starting the app:

```sh
cargo run --release -p dm-bank --example install -- <folder>
```

### Icons

The icon drawings are in `docs/icon/`. After changing one, run `cargo run -p dm-icons --release`
and commit the files it writes: the iOS app icon, the desktop icons in `apps/dm-desktop/assets/`,
and the Android launcher icon layers in `ports/android/app/src/main/res/`. It runs on any system
and needs no image tools.

### Checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The audio tests need a SoundFont. Set `DM_TEST_SF2` to any `.sf2` file; without it they are
skipped.

## Android

The Android app is a Rust library inside a small Android package. Cargo builds the library.
Gradle wraps it in an APK. The package has one small Java class, `MainActivity`, which extends
Android's built-in `NativeActivity`. `NativeActivity` loads the library and runs it, and
`MainActivity` only adds what it cannot do: opening the system's file picker and passing the
chosen file back to Rust.

### What you need

- **Android SDK.** Install it with Android Studio or the command-line tools. You need an SDK
  platform for API 36, the build tools and the platform tools (which contain `adb`).
- **Android NDK.** Install it from the SDK manager. Any recent version works.
- **JDK 17 or 21** for Gradle. Set `JAVA_HOME` to it. Newer JDKs can make Gradle fail.
- **Rust Android targets.** Run this from inside the repository, so the targets join the Rust
  version the repository pins:

  ```sh
  rustup target add aarch64-linux-android x86_64-linux-android
  ```

- **cargo-ndk**, which runs cargo with the NDK's compilers:

  ```sh
  cargo install cargo-ndk
  ```

- **Bash.** On Windows, use Git Bash.

The scripts look for the SDK in `ANDROID_HOME`, then `ANDROID_SDK_ROOT`. If neither is set, they
look where Android Studio installs it:

| System  | Default SDK folder                |
|---------|-----------------------------------|
| Windows | `%LOCALAPPDATA%\Android\Sdk`      |
| macOS   | `~/Library/Android/sdk`           |
| Linux   | `~/Android/Sdk`                   |

They take the NDK from `ANDROID_NDK_HOME`. If it is not set, they use the newest version in the
SDK's `ndk` folder.

### Build the APK

```sh
tools/android/build.sh
```

This builds the library for 64-bit ARM phones (`arm64-v8a`) and for the 64-bit emulator
(`x86_64`). Then it builds the APK. The APK is at
`ports/android/app/build/outputs/apk/debug/app-debug.apk`.

The library is a debug build by default. Add `--release` for an optimised library. The APK is
signed with the debug key either way.

### Install and run it

Start an emulator or connect a phone with USB debugging turned on. Then run:

```sh
tools/android/run.sh
```

This installs the APK, starts the app and shows its log. Press Ctrl+C to stop the log. The app
keeps running. If more than one device is connected, set `ANDROID_SERIAL` to the one you want.
`adb devices` lists them.

Every computer has its own debug key. Android refuses to update an app signed with a different
key, so an APK from another computer, or a release APK, installs only after the old app is
uninstalled. Uninstalling deletes the app's data, including the bank.

### Build a release APK

```sh
tools/android/build.sh --apk-release
```

This builds the optimised library and `ports/android/app/build/outputs/apk/release/app-release.apk`.
`run.sh` installs only the debug APK; install this one with `adb install`.

Without a signing key the release APK is signed with the debug key. To sign it with your own key,
create `ports/android/keystore.properties`. Git ignores this file. A relative `storeFile` is
relative to `ports/android`; a blank `keyPassword` uses the store password.

```properties
storeFile=release.jks
storePassword=...
keyAlias=...
keyPassword=...
```

Or set `DM_KEYSTORE_FILE`, `DM_KEYSTORE_PASSWORD`, `DM_KEY_ALIAS` and `DM_KEY_PASSWORD`, which win
over the file. `DM_VERSION_NAME` and `DM_VERSION_CODE` set the version; they default to `0.1.0`
and `1`.

## iOS

The iOS app is a Rust static library inside a small Xcode project. Cargo builds the library
(`apps/dm-ios`) for devices and for the simulator, and packs both into an xcframework. XcodeGen
writes the Xcode project from `ports/ios/project.yml`; the project and its `Info.plist` are
generated and git-ignored, so edit `project.yml`, never the project in Xcode. A short `main.m`
sets up the audio session, finds the app's folders and calls the library.

**macOS only**, and it needs the full Xcode from the App Store, not only the command line tools.

### What you need

- **Xcode**, with the iOS platform installed. If it is missing:

  ```sh
  xcodebuild -downloadPlatform iOS
  ```

  The scripts use Xcode even when `xcode-select` points at the command line tools: they set
  `DEVELOPER_DIR` to `/Applications/Xcode.app/Contents/Developer` if `xcodebuild` cannot run.
  Set `DEVELOPER_DIR` yourself for an Xcode installed elsewhere.

- **Rust iOS targets.** Run this from inside the repository, so the targets join the Rust version
  the repository pins in `rust-toolchain.toml`:

  ```sh
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim
  ```

  `aarch64-apple-ios` is for real devices; `aarch64-apple-ios-sim` is for the simulator on Apple
  silicon Macs. There is no Intel simulator build.

- **XcodeGen**, which writes the Xcode project from a `project.yml` file:

  ```sh
  brew install xcodegen
  ```

- **Task**, optionally: `brew install go-task`.

### Build the app

```sh
tools/ios/build.sh            # or: task build:ios
```

This builds the library for both targets, generates the project and builds the app twice: for the
simulator, and for devices. The device app is signed when `DM_IOS_TEAM` is set (see "Signing");
without it, it is built unsigned, which still checks that it compiles and links. Add `--release`
for an optimised library. xcodebuild's full output goes to `ports/ios/build/xcodebuild-*.log`;
the script shows only the errors.

### Run it on a simulator

```sh
tools/ios/run.sh              # or: task run:ios
```

This boots the simulator, installs the app and starts it with its log on screen. Ctrl+C stops the
app. `DM_IOS_SIM` names the simulator, `iPhone 17` unless set; `xcrun simctl list devices
available` lists them. For an iPad, for example:

```sh
DM_IOS_SIM="iPad Air 11-inch (M4)" tools/ios/run.sh
```

### Run it on an iPhone or iPad

Set `DM_IOS_TEAM`, build, connect the device with developer mode on, and run:

```sh
tools/ios/run.sh --device     # or: task run:ios DEVICE=1
```

`DM_IOS_DEVICE` names the device when more than one is connected; `xcrun devicectl list devices`
lists them. A new development build may need the network on its first start, to verify its
certificate. You can also open `ports/ios/TickTapDrum.xcodeproj` in Xcode after a build and press
Run, provided the project was generated with `DM_IOS_TEAM` set.

### Drive it without touching the screen

`tools/ios/drive.sh` taps, rotates, presses Home and takes screenshots through a UI test, with
no window on the Mac. The app draws everything itself, so taps go by position, from 0 to 1
across the window:

```sh
tools/ios/drive.sh 'launch; wait 4; shot start; tap 0.5 0.39; wait 30; shot after'
```

The screenshots land in `ports/ios/build/shots`. `--sim NAME` picks a simulator, `--device
NAME` a connected device (signed with `DM_IOS_TEAM`), and `tools/ios/drive.sh --help` lists the
steps. On an iPadOS 17 device the UI test lost its connection to the device's test service, so
it is reliable on simulators only for now.

### Signing

The simulator needs no signing. A real iPhone or iPad needs your Apple development team ID.
**Keep it in `CLAUDE.local.md`, not in a committed file**, and pass it to the build through the
`DM_IOS_TEAM` environment variable. `project.yml` reads it from there.

### Package a release

```sh
tools/ios/package.sh 0.2.0
```

This builds a release and writes two files into `dist/`: `ticktapdrum-<v>-ios.ipa`, the device
app, and `ticktapdrum-<v>-ios-simulator.zip`, the simulator app for Apple silicon Macs. The
version sets the app's version and its build number (`1.2.3` gives `10203`). The device app is
unsigned unless `DM_IOS_TEAM` is set, and iOS installs no unsigned app: a sideloading tool must
sign it first.

## Continuous integration and releases

Two GitHub Actions workflows live in `.github/workflows`.

**`ci.yml`** runs on every push to `master` and every pull request. It checks the format and
runs clippy on Linux, runs the tests on Linux, Windows and macOS, and builds the Android and iOS
apps in debug. A change to documentation only skips the build jobs.

**`release.yml`** runs when a version tag is pushed. It builds every package and publishes them,
with a `SHA256SUMS.txt`, as a GitHub release named after the tag:

| System | Files |
|---|---|
| Windows (x64) | an installer and a zip |
| macOS (Apple silicon) | a disk image and a zip |
| Linux (x86_64) | a Debian package and a tarball |
| Android | `ticktapdrum-<v>-android.apk`, signed with the release key |
| iOS | an unsigned IPA and a simulator app |

To release:

1. Set `version` under `[workspace.package]` in `Cargo.toml` to the new version, and commit it.
   The workflow stops if the tag and this version differ.
2. Tag the commit and push the tag:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

A tag with a suffix, such as `v0.2.0-rc.1`, becomes a pre-release.

The release needs the Android signing key in the repository's secrets (Settings > Secrets and
variables > Actions): `ANDROID_KEYSTORE_BASE64` (the keystore file in base64),
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and, if it differs from the keystore password,
`ANDROID_KEY_PASSWORD`. Without them the Android job fails and nothing is published. The macOS
signing and notarization secrets are optional; see
[`apps/dm-desktop/packaging/README.md`](apps/dm-desktop/packaging/README.md), which also explains
how to build each desktop package locally and what each system asks on the first start.

While the repository is private, the runners use the account's Actions minutes. macOS minutes count
ten times and Windows minutes twice.
