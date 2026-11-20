# Research: exporting a beat as an arranger keyboard style

Findings from 2026-11-20 on whether the app can save a project as a style for an arranger
keyboard, starting with the Yamaha PSR-SX720. These are findings, not decisions. A finding becomes
binding only when [`DESIGN.md`](../DESIGN.md) records it.

Claims marked *unverified* could not be confirmed from a primary source. Nothing here has been
tried on a keyboard, except where "Tried on the SX720" says so.

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
2. A marker, `SFF1` or `SFF2`, then the style's name as meta event 3. In the MIDI standard that
   event is the sequence or track name, not the copyright notice (meta event 2).
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
  SFF GE extends SFF2 with guitar-part data, which drums do not use. The SX720 loads and plays a
  minimal drum-only file without CASM, with either the `SFF1` or the `SFF2` marker (see "Tried
  on the SX720").
- **Drum kit numbers:** older Yamaha keyboards put drum kits on bank MSB 127, LSB 0, with the
  kit as the program number. The SX720's own numbers, including its Revo kits, are in its Data
  List ("Drum&SFX Kit List"). Yamaha publishes one Data List for the SX920 and SX720 together,
  also as a spreadsheet (`.xlsx`) whose sheets include the style lists of both models. The
  General MIDI note numbers for each drum should still hold (*unverified* for every note).
- **Loading:** copy the `.sty` file to a USB stick, then on the keyboard choose a style, User,
  USB. To keep it without the stick, copy it to the User drive.
- One user saw "content incomplete" when loading a style they had edited, so a malformed file
  is refused rather than played wrongly.
- Audio styles do not load on the SX720. They are not needed here.

### Tried on the SX720

Findings from 2026-11-20. The example program `yamaha_style` wrote two test styles: one with the
`SFF1` marker and one with `SFF2`. Each is 541 bytes, at 1920 ticks per beat, with no CASM, OTS
or MDB block. Each holds a two-bar `Main A` and a one-bar `Fill In AA` on channel 10, with the
standard kit (bank MSB 127, LSB 0, program 0).

- **Both files were sent to the keyboard over MIDI,** with the file transfer described in
  "Sending a style over MIDI". The link was Bluetooth MIDI, through an adapter on the keyboard's
  MIDI ports, not the USB cable.
- **Both appear in the style list under User and both play.** So the SX720 accepts a drum-only
  style without CASM, and the first-bar setup described above is enough.
- Not checked: whether each drum sound and the fill are exactly right, how the keyboard treats a
  missing Intro or Ending, and the SX720's own drum kit numbers.

### Sending a style over MIDI

A Yamaha arranger accepts files over MIDI, so a style can go straight into its User drive without
a USB stick. StylePlayer.io does this from the browser with Web MIDI. The messages below come from
reading its page script. Yamaha does not document them. The example program `style_send` sends
the same messages, and it worked on the SX720 (see "Tried on the SX720"):

- `cargo run -p dm-midi --example style_send -- ports` lists the MIDI inputs and outputs.
- `... -- probe <port>` shows the model and the drives and lists the User drive's `STYLE` folder.
  It changes nothing.
- `... -- send <port> <file.sty>` writes the file into that folder. Add `--verbose` to print
  every message.

**Encoding.** All messages are SysEx starting `F0 43 50`. Texts end with a zero byte. Paths and
file data use a 7-bit packing: each group of up to 7 bytes becomes one byte of high bits followed
by the 7 low-bit bytes. In the first byte, bit 6 is the high bit of the group's first byte, bit 5
of the second, and so on. Lengths and sizes are 7-bit numbers, most significant first.

**Every request gets one reply.** The sender waits for it, up to 3 seconds, before the next one.
The reply should be ignored only if it is active sensing or a channel message.

| Step | Sent | Reply |
|---|---|---|
| Model | `F0 43 50 00 00 07 01 F7` | `F0 43 50 00 00 07 02`, packed name, `F7`. The SX720 answers `PSR-SX720`. |
| Start | `F0 43 50 00 00 00 01 F7` | `F0 43 50 00 00 00 02 01 nn F7`; `nn` is the number of drives. |
| | `F0 43 50 00 00 02 01 F7` | Any. |
| Reset | `F0 43 50 00 00 01 01 F7` | `F0 43 50 00 00 01 02 00 F7` |
| Connect on | `F0 43 50 00 00 01 00 01 F7` | `F0 43 50 00 00 01 02 01 F7` |
| | `F0 43 50 00 05 0B 00 00 F7`, then `F0 43 50 00 05 06 00 00 F7` | Any. |
| Drive name | `F0 43 50 00 05 0B 00 dd F7` | `F0 43 50 00 05 0B 01`, 3 bytes, packed name, `F7`. Drive 0 of the SX720 is `USER`. |
| List files | `F0 43 50 00 05 04 00 3F`, length (2 bytes), packed path such as `0:\STYLE\*.*`, `F7` | One entry per reply; ask for the next with `F0 43 50 00 05 05 00 F7`. A reply of 27 bytes or fewer ends the list. |
| Open a file | `F0 43 50 00 06 01 00 05`, file size (5 bytes), `05 00 00 00 00 00`, length (2 bytes), packed path such as `0:\STYLE\Name.sty`, `F7` | `F0 43 50 00 03 00 F7` |
| Data | `F0 43 50 01`, length of the block (2 bytes), block, checksum, `F7`. The block is `02 02`, the packet number (counting from 0, modulo 128), then up to 210 bytes of the file, packed. The checksum makes the sum of the block's bytes a multiple of 128. | `F0 43 50 00 03 00 F7` after each packet. |
| Close | `F0 43 50 00 03 02 F7` | `F0 43 50 00 03 03 F7` |
| Delete a file | `F0 43 50 00 05 01 00 00`, length (2 bytes), packed path such as `0:\STYLE\Name.sty`, `F7` | `F0 43 50 00 05 7F 01 00 01 00 00 00 F7`. Tried on the SX720: the file is gone from the next listing. |
| Connect off | `F0 43 50 00 00 01 00 00 F7` | `F0 43 50 00 00 01 02 00 F7` |

- **The SX720's User drive** holds the folders `SONG`, `STYLE`, `VOICE`, `REGIST`, `MULTI PAD`,
  `SYSTEM`, `TEXT`, `PLAYLIST`, `CHORDLOOPER`, `MIDI` and `Expansion`. A style goes in `STYLE`.
- **StylePlayer avoids overwriting** by listing the folder first and adding `_1`, `_2` and so on
  to a name already in use.
- **StylePlayer's "Preview"** loads a style without keeping it. It writes the style, then a
  registration file that selects it, loads that registration with
  `F0 43 73 01 52 26 03 00 00 00`, path, `00 F7`, and deletes both files. The registration
  format was not studied.
- **StylePlayer says the transfer works on the PSR-SX models and later,** through the USB "To
  Host" port. It also worked over Bluetooth MIDI. On Android, Web MIDI reaches the keyboard over
  USB; iOS has no Web MIDI.

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

### Section numbers over MIDI

A keyboard can also be told which section to play, live, by a SysEx message. A style file does
not need this; it would matter only if the app ever drove a keyboard's style over MIDI. The Tyros2
and Tyros3 Data Lists document the message, and a Genos 2 capture shows the same format on current
models (the SX720's own Data List was not checked).

- **Section Control:** `F0 43 7E 00 ss dd F7`, where `dd` is `7F` for on and `00` for off.
- **Section numbers (`ss`):** Intro A to D are `00` to `03`, Main A to D `08` to `0B`, Fill In AA
  to DD `10` to `13`, the Break `18`, and Ending A to D `20` to `23`. Registration files (`.rgt`)
  use the same numbers for the current section.
- The same group of messages sets the tempo (`43 7E 01`) and the chord (`43 7E 02`).

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

1. **Done: load the hand-made test file on the SX720.** Both the SFF1 and the SFF2 file load and
   play (see "Tried on the SX720"). The example program `yamaha_style` writes them:
   - `cargo run -p dm-project --example yamaha_style -- test1.sty` writes the SFF1 file.
   - Add `--sff2` for the SFF2 file, and `--name <name>` to change the style's name.
2. **Read the SX720 Data List** for its drum kit bank and program numbers, then try a style with
   another kit and listen for the right sounds.
3. **Try a fuller style:** all four Mains, their fills, the Break, an Intro and an Ending, made
   from a real project. This shows how the keyboard moves between sections.
4. **Done: the export is planned** in `DESIGN.md`, "Export as a keyboard style". The app will
   both save a style file and send a style over MIDI.

The converter route (MIDI Song to Style) is no longer needed to prove the format. It stays useful
as a reference if a later keyboard refuses a file without CASM.

## Sources

- Peter Wierzba, Michael P. Bedesem: [Style Files - Introduction and Details, v2.1](https://wierzba.hier-im-netz.de/StyleFileDescription_v21.pdf)
- Yamaha: [MIDI Song to Style for Windows](https://usa.yamaha.com/support/updates/midi-song-win.html),
  [for macOS](https://usa.yamaha.com/support/updates/midi-song-mac.html),
  [supplementary manual v1.2.0](https://ru.yamaha.com/files/download/other_assets/3/2928633/MIDI_song_to_style_v120_SM_En_A0.pdf)
- Yamaha: [PSR-SX720+ downloads](https://usa.yamaha.com/products/musical_instruments/keyboards/arranger_workstations/psr-sx720plus/downloads.html)
- Yamaha: Tyros2 Data List, [Section Control](https://www.manualslib.com/manual/477783/Yamaha-Tyros2.html?page=69);
  Tyros3 Data List, [p.75](https://www.manualslib.com/manual/477788/Yamaha-Tyros3.html?page=75)
- Yamaha Musicians forum: [Genos 2 Section Control capture](https://yamahamusicians.com/forum/viewtopic.php?t=21601)
- StylePlayer: [loading styles from USB and over a USB cable](https://styleplayer.io/community/d/58/2);
  its page script for the file transfer messages
- Yamaha Musicians forum: ["Empty" style from SX920/720](https://yamahamusicians.com/forum/threads/request-empty-style-from-sx920-720.22400/post-132463)
- PSR Tutorial forum: [copying to the User drive](https://forum.psrtutorial.com/index.php?msg=493518),
  [Revo kits on the SX920/720](https://forum.psrtutorial.com/index.php?msg=525486),
  [converting to Roland BK](https://forum.psrtutorial.com/index.php?msg=361302)
- Korg: [Pa4X manual, importing a MIDI file](https://www.manualslib.com/manual/1354785/Korg-Pa4x.html?page=311),
  [Pa300 manual, a file separated by markers](https://www.manualsdir.com/manuals/857881/korg-pa300-arranger.html?page=226)
