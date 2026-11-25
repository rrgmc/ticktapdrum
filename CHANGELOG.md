# Changelog

All notable changes to TickTapDrum are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version numbers follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Changes not yet released wait as separate files in [`changelog.d/`](changelog.d/README.md). They
move into this file when a version is released.

## [Unreleased]

## [1.6.0] - 2026-11-25

### Changed

- Building from source needs Rust 1.98.1.

## [1.5.0] - 2026-11-21

### Added

- In Add instrument and when changing a row's drum, a right click or long press on a drum plays it
  without choosing it.
- A double click on a bar's thumbnail plays the pattern from that bar.
- A right click or long press on a bar's thumbnail plays only that bar, over and over. Do it
  again to play the whole pattern.
- Settings, Sound bank chooses which SoundFont plays: ColomboGMGS2 or a `.sf2` file of your own.
- A beat remembers which SoundFont and kit it was made with. Opened with another SoundFont, it
  plays the nearest kit that SoundFont has, and a notice says so.
- A row whose drum has no sound in the kit playing shows a struck-through speaker.
- On desktop, drag a row's name up or down to move the row.

### Changed

- In Add instrument, drums that already have a row show faint and cannot be added again.
- Playing from a bar no longer turns Follow on.
- The app is dark on every platform, including the window's title bar, whatever the system's
  setting. Windows used to show it light.
- The page that offers to download the drum sounds shows the app's logo, name and a short tagline,
  with the offer in a card and an orange Download button.
- While the chosen MIDI output is not connected, Play shows its keyboard mark struck through, and
  the drums play on this device.
- The download page says in simpler words why the drum sounds are needed.
- Remove bar in the bar pop-up removes the bar shown in the grid, not the last one. Later bars
  move back to close the gap.
- On desktop, a left click on a row's name only plays its drum. A right click opens the row's
  settings.

## [1.4.0] - 2026-11-21

### Added

- A right click on a step gives it the level the last left click gave a step, such as soft, or
  turns it off if it already has that level. A right-button drag paints that level.
- A Sound kits button in the top bar, before Undo, to change the drum kit in one tap.

### Changed

- On desktop, the window opens larger, at 1280 by 820 points, or the screen's size if that is smaller.
- The menu's Sounds item is now two items: Sound kits, and Add instrument, which shows the drums
  to add straight away.

### Fixed

- Stop silences every sound at once, also the long ones some kits have.

## [1.3.0] - 2026-11-20

### Added

- The README explains how to use MIDI out, and how to connect a Bluetooth MIDI device on macOS
  and Windows.
- Export, Keyboard style: saves the beat as a style for a Yamaha arranger keyboard, such as the PSR-SX720. You choose which pattern plays each part of the style: Intro, Main A to D with their fills, Break and Ending.
- Export, Send to keyboard: puts the style straight into a Yamaha keyboard's User styles through the MIDI output, with no USB stick. Sending again replaces the style of the same name.
- With MIDI in on and a Yamaha keyboard, choosing a drum kit on the keyboard makes the app read
  each key the way that kit plays it. A key whose sound the app has no row for is ignored. The
  app names the keyboard's kit and offers its nearest kit with one tap.
- Record a beat by playing it on the keyboard chosen as MIDI output. While the keyboard is
  connected, a MIDI in button shows next to Play. Turn it on, play a pattern and play along: each
  hit lands on the nearest step, soft or loud as you played it. Undo takes back everything
  recorded from Play to Stop. A key lights up the name of its row. A drum with no row shows as a
  faded row under the others; click it to add it, with what you recorded on it. For drum pads,
  choose them under Settings, MIDI input; the app then sounds their hits. Desktop and iPhone/iPad
  only.
- A small piano-keys badge on Play while the drums go to a MIDI output. Its hover text names the output.
- A Volume slider in the MIDI output dialog sets how loud the chosen device plays the drums.
- Add grooves from a file also opens Yamaha keyboard styles (`.sty`, `.prs`, `.sst` and others): each part of the style, such as Main A or Fill In AA, becomes a groove in the groove list.
- A groove from a keyboard style names the keyboard's drum kit, and after it is added the app offers the nearest kit with one tap.

### Changed

- The line of bars at the bottom of the screen marks the bar being played. While the pattern is
  stopped, it marks the bars on screen.

### Fixed

- A keyboard switched off while it is the MIDI output is noticed within a second or two, and the drums play on this device again.

## [1.2.0] - 2026-11-20

### Added

- Rename in the pattern pop-up, to choose a pattern's letter.

### Changed

- The kits are shown as a list, one per line, with a few words on each.
- A thin line at the bottom of the pattern shows where the grid is.
- In song mode, "Play song" is shortened to "Play" so the strip's words fit.

### Fixed

- Each MIDI out hit now ends with its own note-off, 100 ms after it starts.

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

[Unreleased]: https://github.com/rrgmc/ticktapdrum/compare/v1.6.0...HEAD
[1.6.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.5.0...v1.6.0
[1.5.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/rrgmc/ticktapdrum/compare/v1.0.0-rc.1...v1.0.0
[1.0.0-rc.1]: https://github.com/rrgmc/ticktapdrum/releases/tag/v1.0.0-rc.1
