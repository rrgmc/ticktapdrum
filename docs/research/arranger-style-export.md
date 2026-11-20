# Research: exporting a beat as an arranger keyboard style

Findings from 2026-11-20 on whether the app can save a project as a style for an arranger
keyboard, starting with the Yamaha PSR-SX720. These are findings, not decisions. A finding becomes
binding only when [`DESIGN.md`](../DESIGN.md) records it.

Claims marked *unverified* could not be confirmed from a primary source. Nothing here has been
tried on a keyboard yet.

## Terms

- **Arranger keyboard:** a keyboard that plays a backing band (drums, bass, chords) that follows
  the chord the player holds with the left hand.
- **Style:** the backing for one kind of music on an arranger keyboard. It holds several
  **sections**: main variations, fills between them, intros and endings.
- **CASM:** a block of data in a Yamaha style that says how each part is transposed to the chord
  played. Drums are never transposed.

## Summary

| Brand | Open route | Effort | Verdict |
|---|---|---|---|
| Yamaha (PSR-SX, PSR-S, Genos, Tyros) | Write the `.sty` file directly; the format is described by the community | Low: the MIDI export already does most of it | Do first |
| Korg Pa | The keyboard imports a standard MIDI file "separated by markers" into a style | Low, once the marker names are confirmed | Later, if asked for |
| Roland (E-A7, BK) | None found | Unknown | Leave out |
| Casio (`.ac7`) | None found | Unknown | Leave out |

## Yamaha

The main source is Peter Wierzba's "Style Files - Introduction and Details", version 2.1 (2015).
Yamaha does not publish the format; the document comes from its authors' experiments.

### A style is a MIDI file

- A style file is a **format-0 standard MIDI file**, with one track. Optional blocks follow it:
  CASM, OTS (four keyboard setups), MDB (song-finder entries) and, rarely, MH. Only the MIDI part
  is required. The common order is MIDI, CASM, OTS, MDB.
- **A drum-only style needs no CASM block.** A style may leave it out when it uses only channels
  9 to 16, each in its standard role, written in the key of C major 7. Channel 10 is the main
  drums ("Rhythm") and channel 9 extra percussion ("Sub-Rhythm"). Drums ignore the key, so the
  app's beats meet these rules as they are.
- File extensions `.sty`, `.prs`, `.bcs` and others share one format; `.sty` is the safe choice.
- **Ticks per beat:** SFF2 styles appear to need 1920. SFF1 styles allow other values. The app's
  MIDI export uses 96 (`dm_project::TICKS_PER_QUARTER`), and 1920 is exactly 96 × 20.

### What the track holds

**Bar 1** sets the style up. Everything usually sits at tick 0, in this order:

1. Time signature and tempo. The tempo becomes the style's default tempo.
2. A marker, `SFF1` or `SFF2`, then the style's name as a copyright text event (meta event 3).
3. A marker, `SInt`, then the setup for each channel: the GM reset message
   (`F0 7E 7F 09 01 F7`), volume, reverb, then bank select (MSB, LSB) before the program change.

**Bar 2 onward** holds the sections. Each starts with a marker; the names are case sensitive and
must be spelled exactly, spaces included:

| Kind | Markers |
|---|---|
| Main | `Main A`, `Main B`, `Main C`, `Main D` |
| Fill | `Fill In AA`, `Fill In BB`, `Fill In CC`, `Fill In DD` |
| Break | `Fill In BA` |
| Intro | `Intro A`, `Intro B`, `Intro C` |
| Ending | `Ending A`, `Ending B`, `Ending C` |

- **Fills and the Break are exactly one bar.** Other sections may be any length, up to 255 bars.
- **Every style should have `Main A`;** some keyboards refuse one without it. Intro A, Ending A
  and Fill In AA are expected by players.
- If a Main has no fill, some keyboards play Ending A where the fill should be.
- Note-on events are not processed in the setup bar, and only some controllers are allowed in the
  sections (volume, pan, expression, bank select, program change and a few others).

### The PSR-SX720

- Yamaha lists the SX720 among the "SFF GE" models (with the PSR-SX600, PSR-SX920 and Genos2).
  SFF GE extends SFF2 with guitar-part data, which drums do not use. Whether the SX720 loads a
  minimal SFF1 or SFF2 file without CASM is *unverified*; forum users load older styles on SX
  models, which suggests it does.
- **Drum kit numbers:** older Yamaha keyboards put drum kits on bank MSB 127, LSB 0, with the
  kit as the program number. The SX720's own numbers, including its Revo kits, are in its Data
  List ("Drum&SFX Kit List"). The General MIDI note numbers for each drum should still hold
  (*unverified* for every note).
- **Loading:** copy the `.sty` file to a USB stick, then on the keyboard choose a style, User,
  USB. To keep it without the stick, copy it to the User drive.
- One user saw "content incomplete" when loading a style they had edited, so a malformed file
  is refused rather than played wrongly.
- Audio styles do not load on the SX720. They are not needed here.

### Yamaha's own converter

Yamaha's free **MIDI Song to Style** app (version 1.2.0, Windows 11 and macOS 15/26) turns a MIDI
song into a style, and supports the SX720 in the SFF GE format. It offers a way to test without
writing code: export MIDI from TickTapDrum, convert it, load it. How it divides a file into
sections, and whether it handles a drum-only file, is *unverified*: its manual refused to
download.

### What the installed converter shows

Findings from 2026-11-20 on an installed copy of MIDI Song to Style, version 1.1.1 for Windows.
They come from reading its files and the text inside the program, without running it. Its manual
was not available.

- **It is a Qt 6 program,** one 70 MB executable. Beside it are a licence file and
  `MarkerTemplate.mid`, which the menu item "Export Marker Template" also writes.
- **The template shows how a person marks sections in a MIDI file** before converting it:
  - It is a format-1 file at 960 ticks per beat, 4/4, 120 BPM, with 9 tracks.
  - The first track holds the section names as **lyric events** (meta event 6), not markers,
    each at the start of its first bar: `Intro I`, `Fill A`, `Main A`, `Fill B`, `Main B`,
    `Fill C`, `Main C`, `Fill D`, `Main D`, `Break`, `Ending I`, `Intro II`, `Ending II`,
    `Intro III`, `Ending III`, then `End of Section` after the last one. Bar 1 is left for setup.
  - The program also accepts other spellings, such as `Fill AA`, `Intro 1`, `Ending 2` and
    `Fill BA`.
  - The other eight tracks are named Rhythm1, Rhythm2, Bass, Chord1, Chord2, Pad, Phrase1 and
    Phrase2, on channels 9 to 16. Rhythm1 (channel 9) and Rhythm2 (channel 10) select bank
    MSB 127, LSB 0, program 0, the standard drum kit.
- **The program can do more than convert a song:**
  - It opens an existing style ("Open SFF") and can save its music as a plain MIDI file
    ("ExportSEQ"). This is a way to inspect the SX720's own styles.
  - It can send a style straight to a connected keyboard over MIDI ("TransferSTY").
  - It refuses to save a style that has no Main section.
  - It has a size limit ("style file size over!").
- **It writes three versions of the setup marker:** `SFF1`, `SFF2` and `SFF4`. `SFF4` is not in
  Wierzba's document. It is probably the marker of the SFF GE format the SX720 uses (*unverified*).
- **It writes the CASM block itself,** with Ctab or Ctb2 entries for the eight parts. Drum parts
  are assigned as "Main Drum" (Rhythm2) and "Add Drum" (Rhythm1).
- The only voice list built in is the Genos2's. Other keyboards' lists are added with "Import
  Voice List".

**What this means for the app:**

- **A low-effort first step:** export MIDI with the section names as lyric events, in the
  template's layout, and let MIDI Song to Style make the style for each keyboard model.
- **A style the converter saves for the SX720 is the best reference** for a direct export. Its
  bytes show the SX720's setup marker, ticks per beat, the setup bar and the CASM block.

## Korg Pa

- The Pa keyboards (Pa300, Pa800, Pa3X, Pa4X and others) import a **format-0 MIDI file** into a
  style on the keyboard itself, from the Style Record import menu.
- A whole style imports from one file "separated by markers". The markers name each part in a
  pattern like `EnCVn`: the element (`v` variation, `i` intro, `f` fill, `e` ending), its number,
  and the chord variation number. The exact spelling is *unverified*; it is in the Pa4X manual's
  "Importing a Standard MIDI File to a Style".
- Tracks and channels must follow Korg's style layout; others are ignored on import.
- A single file can also be imported as one chord variation.

## Roland and Casio

No documented style format, and no way to import a MIDI file as a style, was found for the
Roland E-A7 or BK series or for Casio's `.ac7` rhythms. Conversion to Roland is sold as a paid
service. These are left out.

## How an export could map a project

The app's project has a tempo, a kit, named patterns of whole bars, and a song (an ordered list
of patterns). A Yamaha style needs up to four Mains, one-bar fills, intros and endings. A
possible mapping:

- **Each pattern can fill one section.** By default the first four patterns of the song become
  Main A to D. A one-bar pattern may serve as a fill or the Break; an intro or ending may be any
  pattern.
- **The export dialog shows the mapping and lets the person change it.** This is an export, so
  asking is fine, unlike imports (see "Groove import"). The dialog says, in plain words, which
  section is missing and what the keyboard does without it.
- **Hidden bars are not exported**, as in the MIDI export. Mute, solo, row volume and swing are
  applied to the notes, as in the MIDI export.
- **The kit:** the app's General MIDI kit number becomes a bank select and program change in
  `SInt`, using the target keyboard's numbers.
- **One writer, several targets.** A Yamaha `.sty` and a Korg "separated by markers" `.mid` are
  both one MIDI track with markers. Only the marker names, the setup bar and the channel layout
  differ, so one function with a target parameter can write both.

## Suggested next steps

1. **Make one style with the converter and try it on the SX720.** This shows whether the keyboard
   accepts a drum-only style. The `.sty` file it produces is the reference for a direct export.
   1. Export a beat from the app as MIDI.
   2. Add the section names as lyric events, in the layout of `MarkerTemplate.mid` (see "What the
      installed converter shows"). A small script can do this from the exported file.
   3. Open the result in MIDI Song to Style, check the sections it finds, and save the style for
      the SX720.
   4. Load the style on the SX720 from a USB stick and play it.
   5. Analyse the saved `.sty` file byte by byte. It shows what a direct export must write: the
      setup marker (`SFF1`, `SFF2` or `SFF4`), ticks per beat, the setup bar, the drum kit's bank
      and program, and the CASM block.
2. **Write a test file by hand or with a small example program:** SFF1, no CASM, 1920 ticks per
   beat, Main A and Fill In AA only. Load it on the SX720. If it is refused, try SFF2.
3. **Read the SX720 Data List** for its drum kit bank and program numbers.
4. **Only then** design the export option in `DESIGN.md`.

## Sources

- Peter Wierzba, Michael P. Bedesem: [Style Files - Introduction and Details, v2.1](https://wierzba.hier-im-netz.de/StyleFileDescription_v21.pdf)
- Yamaha: [MIDI Song to Style for Windows](https://usa.yamaha.com/support/updates/midi-song-win.html),
  [for macOS](https://usa.yamaha.com/support/updates/midi-song-mac.html),
  [supplementary manual v1.2.0](https://ru.yamaha.com/files/download/other_assets/3/2928633/MIDI_song_to_style_v120_SM_En_A0.pdf)
- Yamaha: [PSR-SX720+ downloads](https://usa.yamaha.com/products/musical_instruments/keyboards/arranger_workstations/psr-sx720plus/downloads.html)
- StylePlayer: [loading styles from USB](https://styleplayer.io/community/d/58/2)
- Yamaha Musicians forum: ["Empty" style from SX920/720](https://yamahamusicians.com/forum/threads/request-empty-style-from-sx920-720.22400/post-132463)
- PSR Tutorial forum: [copying to the User drive](https://forum.psrtutorial.com/index.php?msg=493518),
  [Revo kits on the SX920/720](https://forum.psrtutorial.com/index.php?msg=525486),
  [converting to Roland BK](https://forum.psrtutorial.com/index.php?msg=361302)
- Korg: [Pa4X manual, importing a MIDI file](https://www.manualslib.com/manual/1354785/Korg-Pa4x.html?page=311),
  [Pa300 manual, a file separated by markers](https://www.manualsdir.com/manuals/857881/korg-pa300-arranger.html?page=226)
