# Research: reading the drum parts of keyboard styles

Findings from 2026-11-20 on whether the app can import the drums of a Yamaha style file, the
reverse of "Export as a keyboard style" in [`DESIGN.md`](../DESIGN.md). These are findings, not
decisions. Nothing in the app reads styles yet.

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

## What an import would need

- **A key map for each Yamaha kit.** The SX720 Data List ("Drum&SFX Kit List") gives, for each
  kit, the sound on each key. With it, a key such as 13 in kit 73 could become the app's closest
  row (a closed hi-hat, say), and the 90% rule could count mapped keys. This is the deciding piece
  of work. The Data List is a PDF and a spreadsheet on Yamaha's PSR-SX720 downloads page.
- **A rule for channel 9.** Either merge it with its own key map, or offer it as a separate
  groove ("Main A, percussion").
- **No other new parts.** Splitting by markers, the tempo, the time signature and the grid fit
  all work with what `dm-groove` already does. Styles in 3/4 stay refused.
- **Where it would go:** "Add grooves from a file" would take a style as a pack, one groove per
  section, named after the style and the section, as the groove list shows packs now.

## Suggested next steps

1. Get the SX720 Data List and check what keys 13 to 26 and 88 to 91 play in the kits above.
2. Rerun this test with those keys mapped, and see how many sections import and how they sound.
3. Only then design the import in `DESIGN.md`.
