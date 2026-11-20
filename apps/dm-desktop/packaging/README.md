# Packaging the desktop app

Each script builds the release program (`ticktapdrum`) and writes its packages into `dist/` at the
repository root. Run them from the repository root, with the version as the only argument. The
release workflow runs them when a version tag is pushed (`v0.2.0` gives `0.2.0`).

| System | Command | Needs |
|---|---|---|
| Windows (x64) | `pwsh apps/dm-desktop/packaging/windows/package.ps1 0.2.0` | Inno Setup 6 |
| macOS (Apple silicon) | `bash apps/dm-desktop/packaging/macos/package.sh 0.2.0` | Xcode command line tools, the Rust target `aarch64-apple-darwin` |
| Linux (x86_64) | `bash apps/dm-desktop/packaging/linux/package.sh 0.2.0` | The ALSA headers (`libasound2-dev`), `dpkg-deb`, and `dpkg-shlibdeps` (`dpkg-dev`) if possible |

## What a release holds

| System | File | How to install |
|---|---|---|
| Windows (x64) | `ticktapdrum-<v>-windows-x64-setup.exe` | Run it. It installs for the current user, so it needs no administrator rights, and adds a Start menu entry. |
| Windows (x64) | `ticktapdrum-<v>-windows-x64.zip` | Unpack it and run `TickTapDrum.exe`. |
| macOS (Apple silicon) | `ticktapdrum-<v>-macos-arm64.dmg` | Open it and drag TickTapDrum to Applications. |
| macOS (Apple silicon) | `ticktapdrum-<v>-macos-arm64.zip` | The app alone. |
| Linux (x86_64) | `ticktapdrum_<v>_amd64.deb` | `sudo apt install ./ticktapdrum_<v>_amd64.deb` on Debian, or Ubuntu 22.04 and later. |
| Linux (x86_64) | `ticktapdrum-<v>-linux-x86_64.tar.gz` | The program, its icon and a desktop file. Unpack it and run `ticktapdrum`. |

Inside the Debian package, a pre-release such as `0.2.0-rc.1` takes the Debian version `0.2.0~rc.1`,
which sorts before `0.2.0`. The file name keeps `0.2.0-rc.1`, because GitHub changes `~` in the
names of uploaded files. The Linux packages are built on Ubuntu 22.04, because the program
links against the C library of the system it is built on and runs on that system or newer.

## The first start

The Windows packages are not signed with a code-signing certificate, and the macOS app is signed
only when the release has the signing secrets below. Without them each system warns once:

- **Windows:** SmartScreen says "Windows protected your PC". Choose **More info**, then **Run anyway**.
- **macOS, signed ad hoc:** right-click the app and choose **Open**, then **Open** again. On recent
  macOS versions, open it once, then go to **System Settings > Privacy & Security** and choose
  **Open Anyway**. A notarized app starts without either step.
- **Linux:** the program uses the system's sound (ALSA), graphics (Vulkan or OpenGL ES) and keyboard
  (xkbcommon) libraries. The Debian package installs them; with the tarball, every desktop has them.

## Signing and notarizing the macOS app

The macOS script reads these environment variables. Without them it signs the app ad hoc and skips
notarization, as above.

| Variable | Meaning |
|---|---|
| `DM_MACOS_SIGN_IDENTITY` | The signing identity in the keychain, such as `Developer ID Application: Some Name (TEAMID)`. |
| `DM_MACOS_NOTARY_KEY` | The path of an App Store Connect API key file (`.p8`). |
| `DM_MACOS_NOTARY_KEY_ID` | That key's ID. |
| `DM_MACOS_NOTARY_ISSUER` | That key's issuer ID. |

- With an identity, the script signs the program and then the app with the hardened runtime and a
  secure timestamp, and signs the disk image. The app needs no entitlements: it plays sound but
  records none.
- With an identity and all three notary variables, it also sends the app, then the disk image, to
  Apple's notary service, and staples the tickets to both. The zip holds the stapled app. If Apple
  rejects either, the script prints Apple's log and fails.
- Notary variables without an identity are an error, because Apple notarizes only signed apps.

The release workflow fills these variables from the repository's secrets (Settings > Secrets and
variables > Actions). Each one is optional; see the start of
[`release.yml`](../../../.github/workflows/release.yml) for what happens when some are missing.

| Secret | Holds |
|---|---|
| `MACOS_CERTIFICATE_P12` | The Developer ID Application certificate and its private key, exported as a `.p12` file, in base64. The workflow finds the identity name in it. |
| `MACOS_CERTIFICATE_PASSWORD` | The password of that `.p12` file. |
| `NOTARY_API_KEY` | The App Store Connect API key (`.p8`), either its text or the file in base64. |
| `NOTARY_API_KEY_ID` | The key's ID, for `DM_MACOS_NOTARY_KEY_ID`. |
| `NOTARY_API_ISSUER` | The issuer ID, for `DM_MACOS_NOTARY_ISSUER`. |

## The icons

The icon drawings are in `docs/icon/`. The `dm-icons` tool (`tools/icons`) renders them into the
icon files of every platform. Run it on any system after a drawing changes, and commit the files it
writes:

```sh
cargo run -p dm-icons --release
```

It writes these desktop files in `apps/dm-desktop/assets/`, each with rounded corners:

- `icon-256.png`: the window icon and the Linux icon.
- `icon.ico`: the Windows program and installer, from 16 to 256 pixels.
- `icon-1024.png`: the source of the macOS `.icns` file, which the macOS script makes each time it
  runs.
