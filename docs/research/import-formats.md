# Research: file formats the app could import

Findings from 2026-11-19 on which existing drum file formats the app could import, and which of
them have a large library of files to import. These are findings, not decisions. A finding becomes
binding only when [`DESIGN.md`](../DESIGN.md) records it.

Claims marked *unverified* could not be confirmed from a primary source.

## What an import must fit

The app's model, from `DESIGN.md`, limits what any import can keep:

- 4/4 only, 16 steps per bar. Triplets and finer timing have no place on the grid.
- Rows are General MIDI drum sounds. One kit applies to the whole project.
- A tap sets a step off, soft or loud, but a step stores any velocity from 1 to 127, so an
  import keeps velocities as they are. (Corrected by the
  [groove import spike](groove-import-spike.md); this line first said velocity has three levels.)
- A project holds named patterns (A, B, C) and a song, which is an ordered list of patterns.
- One swing value applies to the whole song.

So every import is lossy to some degree. An import should report what it rounded or dropped
rather than fail or change things silently.

## Summary

| Format | Library size | Fit to our model | Parser effort | Verdict |
|---|---|---|---|---|
| Standard MIDI file (`.mid`) | Very large | Good for groove loops, fair for songs | Low | Do first |
| Guitar Pro (`.gp3` to `.gp5`) | Large | Fair (whole songs) | Medium | Worth doing later |
| Hydrogen (`.h2song`, `.h2pattern`) | Small | Very good | Medium | Only if asked for |
| ASCII drum tabs | Large but inconsistent | Fair | Medium, with low success rate | Maybe, as paste-in |
| MuseScore / MusicXML | Large | Poor | High | No |
| LMMS, Renoise | Small | Fair | Medium | No |
| FL Studio, Ableton, Logic, Maschine, hardware SysEx | Varies | Varies | Very high or impossible | No |

## 1. Standard MIDI file

### Library

MIDI has the largest library by far. It comes in two kinds.

**Groove packs** are short drum loops made for drum machines and DAWs. They suit the app best.

- Google Magenta's **Groove MIDI Dataset (GMD)** holds 1,150 MIDI files, about 13.6 hours and
  22,000 bars, played by real drummers on an electronic kit. The MIDI-only download is about
  3 MB. Its licence is CC BY 4.0, confirmed on the Magenta page by the
  [groove import spike](groove-import-spike.md). Its drum keys follow a Roland TD-11 layout, not
  General MIDI.
  ([Magenta](https://magenta.tensorflow.org/datasets/groove),
  [TensorFlow catalogue](https://www.tensorflow.org/datasets/catalog/groove))
- The **Expanded Groove MIDI Dataset (E-GMD)** has 444 hours of audio over 43 kits, with
  matching MIDI, under CC BY 4.0. A MIDI-only archive exists.
  ([Magenta](https://magenta.tensorflow.org/datasets/e-gmd),
  [paper](https://arxiv.org/abs/2004.00188))
- Commercial packs from Toontrack, Groove Monkee, GetGood Drums and others hold thousands of
  grooves. Their licences allow use in music but are expected to forbid redistribution, so the
  app could not bundle them. Users could import packs they own. *Unverified*: licence terms were
  not read.

**Full-song MIDI files** are whole songs with a drum track on channel 10.

- The **Lakh MIDI Dataset** collects 176,581 deduplicated MIDI files, a few thousand of them
  corrupt. The compilation is CC BY 4.0, but the songs inside are third-party works with unknown
  authors. ([Lakh MIDI Dataset](https://colinraffel.com/projects/lmd/))
- Sites such as BitMidi and FreeMIDI host tens of thousands of songs. *Unverified*: counts not
  checked.

Both GMD and E-GMD are free to use as test data under CC BY 4.0, with attribution.

### Mapping

- Notes on channel 10 use General MIDI drum numbers, which are already the app's rows. The app's
  MIDI export uses the same mapping in reverse.
- Velocity is kept as it is.
- A program change on channel 10 picks the kit. Without one, keep the current kit.
- Tempo comes from the first tempo event. Later tempo changes are dropped with a warning.
- Notes snap to the nearest 16th. Notes that land far from any 16th (triplets, flams, rolls) are
  counted and reported.
- Swing can be estimated from how late the second 16th of each pair sits on average. This is
  optional.
- Notes outside General MIDI, such as the GS extras 27 to 34 and 82 to 87, need a row if the bank
  has the sound, or are dropped with a warning.
- A time signature other than 4/4 is refused, or imported with a warning that bars will not line
  up.

### Building patterns from a timeline

A MIDI file is one timeline with no patterns. Import would cut it into bars, then:

1. Give each distinct bar a pattern, reusing a pattern when a bar repeats exactly.
2. Build the song as the list of those patterns.
3. Optionally group runs of bars (for example 4 bars) into one pattern when they repeat as a
   group, which keeps the pattern count low for songs.

A groove pack loop of 1 to 4 bars becomes a single pattern. A full song often becomes many
patterns, because fills make nearby bars differ.

### Effort

Low. The Rust crate [`midly`](https://docs.rs/midly) reads format 0 and 1 files. The app already
writes MIDI, so a round trip (export, then import) is an easy first test.

## 2. Guitar Pro

### Library

Tab sites such as Ultimate Guitar host very large numbers of Guitar Pro files, and many rock and
pop transcriptions include a drum track. *Unverified*: no counts were found. Quality depends on the
person who transcribed each song, and the songs themselves are third-party works.

### Format and parsers

- Versions 3 to 5 (`.gp3`, `.gp4`, `.gp5`) are a binary format with no official specification.
  [PyGuitarPro](https://pyguitarpro.readthedocs.io/en/latest/pyguitarpro/format.html) reads and
  writes them and documents the layout.
- The Rust crate [`guitarpro`](https://docs.rs/crate/guitarpro/latest) is based on PyGuitarPro,
  TuxGuitar and MuseScore. It reads versions 3 to 5. Versions 6 and 7 (`.gpx`, `.gp`) are on its
  roadmap, not done.
- Version 6 and later use different, container-based formats. Supporting them means more work.
  alphaTab (JavaScript) is said to read them. *Unverified*.

### Mapping

Drum tracks store General MIDI percussion numbers, so the row mapping matches MIDI. The rest has
the same problems as full-song MIDI: whole songs, triplets, and time-signature changes. Guitar Pro
does store bars explicitly, which makes the bar split easier than with MIDI.

### Effort

Medium for versions 3 to 5, if the `guitarpro` crate proves good enough. High for 6 and later.

## 3. Hydrogen

### Library

Small. Hydrogen ships demo songs, and users have shared drum kits and some patterns. *Unverified*:
no count of shared songs or patterns was found.

### Format

- `.h2song` holds a whole song; `.h2pattern` holds one pattern. Both are XML.
  ([manual](https://hydrogen-music.org/documentation/manual/manual_en_chunked/ch21.html))
- There is no complete specification. The wiki describes the format as "well-formed, but
  non-standard XML", gives a rough sketch, and warns it may be out of date. The loader in
  Hydrogen's source is the real reference.
  ([wiki](https://github.com/hydrogen-music/hydrogen/wiki/Song-format))

### Mapping

Structurally it is the closest match: named patterns, a pattern order, swing and per-note
velocity. The difficulty is instruments. Hydrogen instruments are samples with free-form names,
not General MIDI numbers, so rows must map by name ("Kick", "Snare", "Hat Closed"), with a
fallback for names that do not match.

## 4. ASCII drum tabs

Plain-text tabs such as `HH|x-x-x-x-x-x-x-x-|` are widely posted online and would suit a paste-in
box for users with little music knowledge. There is no standard: row labels, hit symbols, bar
widths and repeat marks all vary. A forgiving parser could handle the common cases, but a large
share of real tabs would not parse cleanly. The [tab import spike](tab-import-spike.md) surveyed
256 tabs: 37% parse cleanly, 47% need guesses and 16% fail.

## 5. Formats not worth importing

- **MuseScore (`.mscz`) and MusicXML:** large catalogues on musescore.com, but drum notation
  (voices, tuplets, notehead shapes) maps poorly to steps, and most downloads there need an
  account. *Unverified*: download terms not checked.
- **LMMS (`.mmp`, `.mmpz`) and Renoise (`.xrns`):** open XML formats with step sequencers, but
  small libraries.
- **FL Studio (`.flp`), Ableton (`.als`), Logic, GarageBand, Maschine:** closed or undocumented.
  Users of these programs can export MIDI, which covers them.
- **Hardware drum machines** (Roland TR series, Elektron and others): SysEx dumps that differ per
  model. Newer Akai MPC `.mpcpattern` files are JSON and readable, but few users would bring them.

## Recommendation

1. **MIDI import, aimed at groove packs first.** Short 4/4 loops in 16ths fit the grid as they
   are, the library is the largest, and the note mapping already exists in export. Use GMD as test
   data. The [groove import spike](groove-import-spike.md) tested this on about 2,000 files.
2. **Then song MIDI**, with the bar split and pattern reuse above, and a report of what was
   rounded or dropped.
3. **Guitar Pro 3 to 5 later**, if users want to play along with songs. It reuses the same
   pattern-building code.
4. **Hydrogen only on request.** Good fit, small library.

## Open questions

- Whether quiet files, such as GMD with 65% of notes below velocity 80, should be scaled up so
  the grid does not draw most of their notes as soft.
- Whether a song import with dozens of near-identical patterns is usable, or whether import should
  merge bars that differ by only a few steps.
- How import is offered on Android, which has the file picker from milestone 10.
