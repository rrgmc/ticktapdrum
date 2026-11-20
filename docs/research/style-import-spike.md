# Research: reading the drum parts of keyboard styles

Findings from 2026-11-20 on whether the app can import the drums of a Yamaha style file, the
reverse of "Export as a keyboard style" in [`DESIGN.md`](../DESIGN.md). These are findings, not
decisions. Nothing in the app reads styles yet. The programs used are throwaway and not in the
repository.

## What was tried

A throwaway program read 172 style files from a PSR-SX720 owner's backup (the local notes say
where). They are factory, StylePlayer and user styles, with the extensions `.prs`, `.sty`, `.sst`,
`.scp` and `.fps`, which share one format.

For each file it read the MIDI track and stopped before the CASM, OTS and MDB blocks. It then
cut the track at the section markers (`Intro A`, `Main A`, `Fill In AA`, `Ending A` and so on).
For each section it built a small MIDI file with only the drum notes, shifted to start at 0, with
the style's tempo and time signature. Each of those went through `dm_groove::import`, the app's
own groove import, unchanged. Three versions were tried:

1. Channel 10 only (the main drums, "Rhythm 2").
2. Channels 9 and 10 merged (adding "Rhythm 1", the extra percussion).
3. Channels 9 and 10 merged, with notes on keys outside General MIDI (35 to 81) left out.

## Results

**Reading the files is easy.** All 172 parsed. 161 are SFF2 and 11 SFF1. Most use 1920 ticks
per beat, and 20 use 480. 167 are in 4/4 and 5 in 3/4, which the grid cannot hold. The 2,451
sections are mostly 1, 2, 4 or 8 bars long; only a few are longer than 16.

**Timing is rarely the problem.** Only 64 of the 2,451 sections had too many notes off the
16-step grid. Style drums are mostly on the grid, including swung ones.

**The drum keys are the problem.**

| Version | Accepted | Refused as "made for another drum program" | Off the grid | Not 4/4 |
|---|---|---|---|---|
| Channel 10 only | 496 | 1,745 | 95 | 73 |
| Channels 9 and 10 | 177 | 2,120 | 64 | 75 |
| Both, non-GM keys left out | 612 | 1,522 | 192 | 74 |

- **About a third of channel 10's notes are on keys General MIDI does not use:** keys 13 to 26
  above all, and 88 to 91. The app's import refuses a groove when fewer than 90% of its notes
  are on drum-kit keys, so most sections fail.
- **It depends on the kit.** The setup bar chooses each part's kit with bank select and a
  program change. The Standard kit (bank MSB 127, program 0) has 4% of its notes outside General
  MIDI. The kits most styles use (bank MSB 127, programs 73, 17, 69 and 72, probably Yamaha's
  newer kits) have 33% to 66%. Channel 9 often uses SFX kits (bank MSB 126).
- **Leaving those notes out is not a fix.** In most sections it removes 20% to 60% of the notes,
  which would change the beat, and many sections are still refused.
- **Channel 9 is mostly percussion** (keys 60 to 81: congas, shakers, cowbells), which the app's
  rows and its 90% drum-kit rule do not expect.

## The kits' own keys

The SX720 Data List names the sound on every key of its 56 drum and SFX kits (sheet "Drum&SFX
Kit List SX720" of the spreadsheet on Yamaha's PSR-SX720 support page, under Manuals &
Documents; it downloads as a `.zip` holding `psrsx920_sx720_en_dl_a0.xlsx`). In the list,
program numbers count from 1, so program 73 in a style file is kit 74, PopDrumKit.

- **The extra keys are mostly ordinary drums.** In the kits most styles use (PopDrumKit,
  RockDrumKit), keys 13 to 22 are hi-hat articulations ("Hi-Hat Tip 00" to "Hi-Hat Edge 99",
  where the number is how open the hi-hat is, and the pedal). Keys 25 to 34 are other snares, a
  snare roll and a second kick. Keys 23, 24 and 32 are clicks and a count-in.
- **Kits from the older standard keep General MIDI's extras there:** surdo, Hi Q, scratches,
  metronome clicks, brushes, castanets.
- **Some kits are made of loops and effects:** SchlagerKit's low keys are "Loop Element" slices,
  EDM Kit has glitches, impacts and reverses. Those have no place on the app's rows.
- **The SFX kits on channel 9** (bank MSB 126) are hand percussion: djembe, cajon, congas, bongos,
  udu, triangles.

## With the keys mapped

A second program gave every key of every kit a General MIDI key from its sound's name: a hi-hat
from "Tip"/"Edge" 00 to 25 becomes the closed hi-hat (42), 50 and over the open one (46), and the
pedal 44; snares become 38, kicks 36, congas 63, shakers 70, and so on. A sound already on a
General MIDI key of its own family keeps that key, so toms keep their pitch. Clicks, loops,
glitches, noises and effects are left out. The test then ran again with each channel's notes
mapped through the kit its setup bar chose.

| Version | Accepted | Refused as "made for another drum program" | Off the grid | Not 4/4 |
|---|---|---|---|---|
| Channel 10, mapped | 1,596 | 381 | 354 | 71 |
| Channels 9 and 10, mapped | 705 | 1,440 | 213 | 75 |

- **Mapping is the fix.** With channel 10 mapped, 65% of the sections import, against 20%
  before, and 152 of the 172 styles have at least one section that imports.
- **Little is lost.** With both channels mapped, 2,140 of 2,451 sections lose less than 10% of
  their notes, and only 37 lose more than half.
- **What is still refused:** kits that are mostly percussion or effects; sections whose hi-hats
  are played off the grid (more of them now that the hi-hat notes count); and the styles in 3/4.
- **Channel 9 needs its own rule.** Its congas and shakers fail the 90% drum-kit rule when
  merged, though they are on General MIDI keys after mapping.
- **Not checked:** how the imported grooves sound. Nobody has listened to one next to the
  keyboard's own playing.

## What an import would need

- **The key map, shipped with the app.** A table from (bank, program, key) to a General MIDI key
  or "leave out", made once from the Data List as above, with Yamaha's kits from other models
  added the same way. Notes from a kit not in the table keep their key if it is a General MIDI
  one.
- **A rule for channel 9.** Either count General MIDI percussion keys (60 to 81) as drum keys for
  styles, or offer channel 9 as a separate groove ("Main A, percussion").
- **No other new parts.** Splitting by markers, the tempo, the time signature and the grid fit
  all work with what `dm-groove` already does. Styles in 3/4 stay refused.
- **Where it would go:** "Add grooves from a file" would take a style as a pack, one groove per
  section, named after the style and the section, as the groove list shows packs now.

## Suggested next steps

1. Listen to a few imported sections next to the keyboard playing the same style, to check the
   hi-hat openness threshold and the snare choices.
2. Decide the channel 9 rule.
3. Then design the import in `DESIGN.md`.
