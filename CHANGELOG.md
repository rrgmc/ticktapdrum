# Changelog

All notable changes to TickTapDrum are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version numbers follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Changes not yet released wait as separate files in [`changelog.d/`](changelog.d/README.md). They
move into this file when a version is released.

## [Unreleased]

## [1.1.0] - 2026-11-20

### Added

- Add drum grooves from MIDI files or zip packs, from a link, or shared from another app on
  Android. The Grooves list keeps the grooves you added.
- 359 beats and fills played by real drummers, from Magenta's Groove MIDI Dataset, in 24 styles.
  They appear in Ready-made styles and in the Grooves list.
- Preview a groove before adding it.
- Paste a few bars of a drum tab, behind an experimental setting.
- Links to free grooves and drum tabs from their dialogs.
- The bar line can play from a chosen bar and follow playback, and can show bar thumbnails.
- The window title shows the project's name.
- The desktop app can use the clipboard.

### Changed

- The menu is folded into Grooves and Settings.
- Empty bars at either end of an imported groove are dropped.

### Removed

- Basic mode.

### Fixed

- The Android app no longer hangs when playback starts or stops.

## [1.0.0] - 2026-11-19

### Fixed

- The Debian package's file name no longer holds a tilde.

## [1.0.0-rc.1] - 2026-11-19

### Added

- The first release, for Windows, macOS, Linux, Android, iPhone and iPad: a step grid, ready-made
  beats, patterns and songs, kits, tempo, swing and per-row volume, undo and redo, MIDI and WAV
  export, MIDI out on desktops and iOS, pictures for the drums, and English and Brazilian
  Portuguese.

[Unreleased]: https://github.com/rrgmc/ticktapdrum/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.0.0-rc.1...v1.0.0
[1.0.0-rc.1]: https://github.com/rrgmc/ticktapdrum/releases/tag/v1.0.0-rc.1
