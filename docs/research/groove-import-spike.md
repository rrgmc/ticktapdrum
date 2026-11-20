# Spike: importing groove packs

Findings from 2026-11-19 on how well drum groove packs import into the app and play there. This
follows up [`import-formats.md`](import-formats.md), which recommended MIDI import aimed at groove
packs. These are findings, not decisions. A finding becomes binding only when
[`DESIGN.md`](../DESIGN.md) records it.

## Summary

- **Groove packs in General MIDI (GM) mapping import well.** Of the 4/4 files in Groove Monkee's
  free GM pack, 97% import cleanly or with small losses. Every one fits in a single pattern, and
  notes move 4 ms on average when snapped to the grid.
- **The biggest risk is the drum mapping, not the timing.** Vendors ship each pack in many
  mappings, one per drum program. A file made for another program imports with clean timing but
  plays the wrong drums, for example congas instead of hi-hats. The timing checks cannot see this.
- **Live performances import poorly as grooves.** The Groove MIDI Dataset (GMD) is recorded
  drumming. Only 37% of its 4/4 files import cleanly or with small losses. Its long takes turn into
  many near-identical patterns, and some run out of patterns.
- **Swing and tempo come through.** An export and re-import of every built-in preset gives back
  identical steps, with swing within 0.01.
- **Only GMD can be bundled with the app.** The free commercial packs forbid use in apps.

## What was done

### Test files

| Source | Files | Mapping | Licence | Bundle in the app? |
|---|---|---|---|---|
| [GMD](https://magenta.tensorflow.org/datasets/groove), MIDI-only zip | 1,150 | Roland TD-11 (not GM) | CC BY 4.0 | Yes, with attribution |
| Groove Monkee "Beat Farm" free pack | 241 | GM | Freebie licence | No |
| Groove Monkee free samples, GetGood Drums mapping | 274 | GetGood Drums | Freebie licence | No |
| Groove Monkee free samples, Perfect Drums mapping | 382 | Perfect Drums | Freebie licence | No |

- The GMD licence is now confirmed on the Magenta page itself: "The dataset is made available by
  Google LLC under a Creative Commons Attribution 4.0 International (CC BY 4.0) License." The
  Expanded GMD (E-GMD) page says the same. E-GMD was not downloaded: its MIDI-only archive is
  103 MB and holds the same performances played back on other kits.
- Groove Monkee's freebie licence allows "ONLY ... noncommercial compositions/releases". It forbids
  "the creation of any competitive product; including ... App Contents (Mobile or otherwise)" and
  "uploading, file sharing". So these packs are test data only.
- Packs that needed an account, an email address or a $0 checkout were skipped: Toontrack,
  GetGood Drums' own free grooves, Groove Monkee's "Drum Freebie Pak" and Samples From Mars.
- No commercial-style groove pack under an open licence was found.

### The spike tool

[`tools/groove-spike`](../../tools/groove-spike) is throwaway research code, not the importer. For
each file it:

1. reads the notes on channel 10, or on the only channel the file uses;
2. maps the file's drum keys to GM keys (as they are, or from the TD-11 layout);
3. finds the swing (0 to 0.5, as the app defines it) and a constant time offset (up to a quarter
   step either way) that best fit the notes;
4. snaps every note to the nearest step of that swung grid, and measures how far it moved;
5. merges notes of the same drum that land on the same step, keeping the loudest ("collisions");
6. builds a project through `dm-project`'s public API. A file of up to 16 bars becomes one pattern.
   A longer file is cut into 4-bar patterns, and a pattern is reused when a 4-bar chunk repeats
   exactly.

It can also render a file twice with the same SoundFont: the original notes at their recorded
times, and the imported project through the app's own offline renderer, `dm_audio::OfflineRender`.

Each file gets a verdict:

- **Clean:** 4/4, at most 2% of notes moved more than a quarter step, at most 2% of notes lost, and
  no triplet feel.
- **Usable:** the same limits at 10%, or a triplet feel.
- **Poor:** anything else, or not 4/4.

A quarter step is 31 ms at 120 BPM. "Triplet feel" means the notes fit an 8th-note triplet grid
clearly better than any swung 16th grid.

## Results

All 2,047 files parsed; none failed. The table counts only files in 4/4.

| Source | 4/4 files | Clean | Usable | Poor | Fit one pattern |
|---|---|---|---|---|---|
| Beat Farm (GM) | 189 | 144 (76%) | 39 (21%) | 6 (3%) | 189 |
| Perfect Drums mapping | 342 | 202 (59%) | 54 (16%) | 86 (25%) | 341 |
| GetGood Drums mapping | 264 | 91 (34%) | 37 (14%) | 136 (52%) | 263 |
| GMD, all | 1,138 | 164 (14%) | 266 (23%) | 708 (62%) | 851 |
| GMD, beats | 491 | 40 (8%) | 164 (33%) | 287 (58%) | 204 |
| GMD, fills | 647 | 124 (19%) | 102 (16%) | 421 (65%) | 647 |

The verdicts for the Perfect Drums and GetGood Drums files describe timing only. Their drums are
wrong, as the next section explains.

### 1. The mapping decides whether a file sounds right

Each drum program expects its sounds on different keys, so vendors sell each pack in up to 29
mappings. Users who pick the wrong folder get the wrong sounds:

- **GetGood Drums mapping:** the hi-hats are on keys 54 and 70, which are the tambourine and
  maracas in GM, and on key 10, which GM does not define. The spike dropped key 10 (1,509 notes);
  37% of the files lost notes this way.
- **Perfect Drums mapping:** the hi-hat is on key 63, the open high conga in GM. 4,412 hi-hat
  notes would play as congas, and no note was dropped, so nothing looks wrong.
- **GMD (TD-11):** keys 22 and 26 are hi-hat edge hits and are not GM. Keys 40, 52, 55, 58 and 59
  mean different drums than in GM (for example 58 is a tom rim, but the vibraslap in GM). Guessing
  the TD-11 layout from keys 22 and 26 caught only 519 of the 1,150 files, because many takes never
  hit a hi-hat edge. The layout has to come from the source, not from each file.

Channel use also varies. 19 Beat Farm files and every GetGood Drums file put the drums on
channel 1. Taking the only channel a file uses handled all of them.

### 2. Groove-pack timing fits the grid

Beat Farm files are made on a grid with light humanising. The median file has every note within a
tenth of a step, and notes move 4 ms on average. The 6 Poor files hold 32nd-note hi-hat or ride
"ratchets", which merge into 16ths.

Collisions are rare in packs: 1.8% to 2.3% of notes. They come from 32nd notes and flams.

### 3. Live playing does not fit the grid well

GMD takes are human performances, not loops:

- In the median file, notes move 18 ms on average and 14% of notes move more than a quarter step.
  These are ghost notes, flams and drags, which the grid has no place for.
- 60% of files have collisions, and 5.2% of all notes are lost to them.
- Drummers play ahead of or behind the beat. The best constant offset was zero for only 311 files.
  Fitting the offset raised the Clean count from 110 to 164 files. The offset has to stay within a
  quarter step. A wider search let it shift notes by half a step, which could move a backbeat onto
  the wrong step.
- Beats are long: the median beat is 24 bars, and 133 files run past 64 bars. Exact repeats almost
  never happen in human playing, so pattern reuse does not help. A 29-bar funk take became 8
  patterns for 8 chunks. A 251-bar jazz take used all 26 patterns, and 37 of its 63 chunks were
  left out of the song.
- Many takes start with silence. In 91 of 503 GMD beats (18%) the first note comes a beat or more
  after the start. Usually the drummer comes in with a pickup that leads into bar 2. In the 80 BPM
  funk sample, bar 1 holds only a soft snare pickup on its last five steps. The silence is in the
  recording, not caused by the import. No file in the three packs starts late.
- Merging bars that differ by up to two hits barely helps: the funk take went from 27 distinct bars
  to 23, the jazz take from 111 to 95.
- GMD velocities are low: 65% of notes are below 80. The app keeps every velocity from 1 to 127 per
  step, so nothing is lost, but the grid draws every note of velocity 60 or less as soft.

GMD is a good stress test. As a library for users it needs a way to pick a few bars out of a take.

### 4. Swing and tempo survive a round trip

Every built-in preset was exported with the app's MIDI export (`dm_project::to_midi`) and imported
again. All seven gave back identical steps and velocities. Swing came back within 0.01: Hip-hop
went from 0.16 to 0.17 because the export rounds swing to whole ticks. Tempo came back as, for
example, 109.99991 instead of 110, so an importer should round it.

### 5. Triplets, shuffles and odd meters

The app's swing delays every second 16th. It cannot express 8th-note swing or triplet fills.

- Files with a triplet feel: 6.3% of GMD, 8.4% of GetGood Drums, 10.5% of Perfect Drums and none
  of Beat Farm.
- One workaround was tried: import in half time, where one step holds an 8th note and the tempo
  halves, so the app's swing becomes 8th-note swing. It cut the timing moved by at least a fifth in
  only 13 of 1,150 GMD files and 58 of 382 Perfect Drums files. It is not worth making a default.
- Odd meters are common in packs: 21.6% of Beat Farm (7/4 and 3/4) and 10.5% of Perfect Drums
  (12/8 and 6/8), against 1% of GMD. Every file had a time signature, so they are easy to detect.

### 6. Accepting only groove files

A later question was whether the import can accept groove loops and refuse every other MIDI file.
A rule of six checks was tried on the same files: notes on one channel only; 4/4 with no change;
one tempo; at most 16 bars; at most 10% of notes more than a quarter step off the grid; and at least
90% of notes on drum-kit keys (General MIDI 35 to 59, so hand percussion keys count against it).

| Source | Accepted | Refused mainly for |
|---|---|---|
| Beat Farm (GM) | 189 of 241 | 52 not in 4/4, 8 off the grid (32nd-note hi-hat rolls) |
| Perfect Drums mapping | 141 of 382 | 164 not mostly kit keys, 79 off the grid, 40 not in 4/4 |
| GetGood Drums mapping | 86 of 274 | 140 not mostly kit keys, 101 with keys outside GM, 67 off the grid |
| GMD | 286 of 1,150 | 689 off the grid, 297 longer than 16 bars |

- Every Beat Farm file in 4/4 that sits on the grid was accepted. The kit-key check refused no GM
  file: every Beat Farm file has all its notes on kit keys.
- The kit-key check refuses most files made for other drum programs. Some of them still pass,
  probably grooves without a hi-hat whose keys happen to match GM; how they sound was not checked.
- GMD's TD-11 hi-hat keys 22 and 26 are outside the kit keys, so many GMD files fail that check.
  GMD files would need to be recognised and mapped first.
- Full-song MIDI files were not tested. The one-channel and 16-bar checks should refuse them, but
  that is reasoning, not measurement.

## What was checked and what was not

- **Checked:** every file parsed. Seven sample files, chosen to cover clean, swung, triplet, live
  and long cases, were converted to projects. Each loaded back through the app's `from_json`
  unchanged. Each was rendered both ways through the same SoundFont.
- **Listened:** the user compared the original and imported renders of all seven samples, plus the
  jazz take in half time. Each pair sounded very similar. Both renders of a file use the same key
  mapping, so this says nothing about whether a mapping is right. The Perfect Drums sample plays
  congas in both.
- **Opened in the app:** the user opened all eight sample projects in the desktop app. Seven
  looked fine. The 80 BPM funk take showed a mostly silent first bar, which is in the recording
  (see section 3).
- **Not checked:** nothing was tried on Android.

## Recommendations for MIDI import

1. **Target GM groove loops first.** One file becomes one pattern of up to 16 bars. Take the
   tempo from the file, rounded, and estimate swing and offset as the spike does.
2. **Check the mapping before importing.** If a file has a kick and a snare but no GM hi-hat or
   ride key, or uses keys outside GM, say that it seems made for another drum program and suggest
   its GM version. Offer GMD's TD-11 layout as a named choice.
3. **Refuse files that are not in 4/4,** and say why.
4. **Report what changed:** notes moved more than a quarter step, notes merged, keys dropped.
5. **For files longer than 16 bars, ask which bars to import** into one pattern instead of
   building dozens of patterns. Pattern reuse by exact match does not work on live playing.
   Start the suggested range at the first full bar, so a count-in or pickup does not become a
   silent first bar in a loop.
6. **Demo grooves:** GMD is the only source that can be bundled. Clean GMD fills and short beats
   would work, with attribution under CC BY 4.0.

## Reproducing

The test files are not in the repository. Download them from the sources above, then:

```text
cargo run --release -p groove-spike -- analyze --map td11 <GMD folder> --csv gmd.csv
cargo run --release -p groove-spike -- analyze <pack folder> --csv pack.csv
cargo run --release -p groove-spike -- render --map gm <file.mid> <bank.sf2> <out folder>
cargo run --release -p groove-spike -- roundtrip
```

`--half` imports in half time. `analyze` on a single file prints a full report for it.
