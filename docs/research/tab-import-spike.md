# Spike: importing ASCII drum tabs

Findings from 2026-11-19 on which conventions plain-text (ASCII) drum tabs follow, whether a parser
can trust them, and how well real tabs import into the app. This follows up
[`import-formats.md`](import-formats.md), which listed ASCII tabs as "maybe, as paste-in". These are
findings, not decisions. A finding becomes binding only when [`DESIGN.md`](../DESIGN.md) records it.

## Summary

- **There is no standard.** Published guides describe a common core and disagree on the details.
  Every one tells the reader to trust each tab's own legend.
- **The common core is reliable:** one drum per row with a label on the left, `-` for a rest, `|`
  for a bar line, one character per time slot, `x` and `o` for hits, capitals for accents.
- **Of 256 real tabs, 37% parse cleanly, 47% parse with guesses, and 16% fail.** Snare hits land
  on beats 2 and 4 in 73% of parsed bars that have a snare, which suggests the columns are read
  correctly.
- **Labels, bar widths and repeats need guessing,** and some tabs give common labels unusual
  meanings. An import must show the user what it guessed and let them correct it.
- **No source can be bundled.** Every site's terms or missing licence limit tabs to personal use.

## What the published conventions say

| Source | What it is |
|---|---|
| [Wikipedia, "Drum tablature"](https://en.wikipedia.org/wiki/Drum_tablature) | A legend: labels CC, HH, Rd, SN, T1, T2, FT, BD, Hf; symbols x, X, o, O, g, f, d, #, @. Says symbols "may vary". No bar length or repeats. |
| [Wikibooks, "Tablature/Drums"](https://en.wikibooks.org/wiki/Tablature/Drums) | An unfinished stub that conflicts with the others: HH is the *open* hi-hat, T and t are low and high toms. |
| [TigerBill drum tabs](https://tigerbill.com/drumtabs.htm) | A teaching legend. Says conventions "differ by site or author". |
| [Aerodrums tab import syntax](https://aerodrums.com/aered/aered-tab-import-syntax) | The only machine-readable spec found. Bar width sets the grid (16 = 16ths, 8 = 8ths, 12 = triplets, 32 = 32nds), a count line overrides it, and repeats are `x2`, `2x` or "Repeat 5x". In its reading, "Repeat 5x" means 6 plays. |
| Converters (drumtabconv, TAB2XML, MIDI::Tab) | Make labels and notes per bar configurable, which confirms there is no fixed standard. |
| Ultimate Guitar | No public drum-tab legend or submission rule was found. |

They agree on the common core listed in the summary. They conflict on:

- **`o`**: an open hi-hat on a hi-hat row, but a normal hit on a drum row.
- **`F`**: floor tom in most tabs, hi-hat foot or flam in others.
- **`Ch`**: china cymbal in Aerodrums, but it can also read as closed hi-hat.
- **`HH`**: hi-hat almost everywhere, open hi-hat in Wikibooks.
- **`b`**: ride bell or a buzz roll. **`R`**: ride or rimshot.
- **Bar width**: only Aerodrums ties it to the grid. Many guide examples use 8-slot bars.

On accuracy, no study of drum tabs was found. A study of about 4.5 million guitar tabs (Macrae and
Dixon, ISMIR 2011) found that search rank and user ratings predict accuracy poorly. Agreement
between several tabs of the same song predicted it better. Its error rates could not be read here.

## What was done

### Test tabs

256 tabs by about 135 transcribers, for 199 artists:

| Source | Tabs | Terms |
|---|---|---|
| Ultimate Guitar drum tabs | 105 | Terms of service 6.2: reproduction only "for your personal, non-commercial use" |
| An open directory of tabs from MyGuitarTabs.com, late 1990s to 2000s | 85 | No terms visible |
| Seven GitHub repositories of drum tabs | 52 | No licence in any of them |
| Tabcrawler | 14 | Terms not read |

Genres are mostly rock, metal and pop. Jazz and blues shuffles are almost absent. Nothing here can
be bundled or committed; the tabs also contain transcribers' names and email addresses.

### The parser

The [spike tool](../../tools/groove-spike) gained a `tab` command, still research code. It:

1. finds "systems": runs of lines that start with a short label and hold mostly rests and hits;
2. undoes common damage first: `¦`, `│` and a capital `I` between hits become bar lines, and long
   dashes from web typography become rests;
3. skips guitar systems (string names as labels) and count lines;
4. cuts each row into bars at `|`, or into bars of 16 if the row has no bar lines;
5. replaces a bar holding only `%` with the bar before it;
6. fixes a bar one column wider than 4, 8, 16 or 32 by dropping a column that is empty in every
   row (a spacer), and pads a bar one column short;
7. reads the bar's width as its grid and scales anything else onto 16 steps, counting hits that
   move more than a sixth of a step;
8. maps the row label to a General MIDI drum, using the tab's own legend first. Some symbols
   depend on the row: `o` on a hi-hat row is an open hat, `b` on a ride row is the bell, `C`, `r`,
   `s` and `c` on a cymbal row are crash, ride, splash and china;
9. reads repeat marks such as `x4` at the end of a row or on the line after a system, as total
   plays.

## Results

| Measure | Tabs |
|---|---|
| Clean: every bar has a standard width as written, every label known | 94 (37%) |
| Partial: something was guessed, fixed or dropped | 121 (47%) |
| Failed: no drum system, most bars of odd width, or most labels unknown | 41 (16%) |

By source, the GitHub tabs did best (48% clean) and Tabcrawler worst (21% clean, 36% failed).

### Where tabs fail

- **Odd bar widths** cause most failures. 16 tabs are mostly 12 or 24 columns wide (triplets, or
  something else, see below). 18 more use other widths. 7 tabs had no recognisable drum system.
- **Bar widths as written:** 16 columns in 68% of 19,448 bars, then 17, 12, 8, 32 and 15. Fixing
  one-column spacers and short bars raised readable widths from 76% to 88% of bars. 20% of tabs
  needed such a fix.
- **Count lines** (`1 e + a 2 e + a`) appear in 31% of tabs. They can explain odd widths, but they
  are ambiguous themselves. One tab writes 12-column bars counted `1+a2+a3+a4+a`, which can mean
  triplets ("1-and-a") or 16ths with the unused "e" column left out. A later part of the same tab
  is 15 columns wide, counted `1 + 2 + 3+a4 +`: uneven 16ths only the count line explains. Of 102
  tabs with odd-width bars left after fixing, 42 have a count line.

### Labels and legends

- 28% of tabs have a legend. Most restate the usual meanings; a legend explained an otherwise
  unknown label in 5 tabs.
- **6 tabs redefine a common label** in their legend: `T` as tambourine (twice) or closed hi-hat,
  `H` as high tom, `HF` as floor tom and `Cs` as crash. Tabs without a legend may do the same with
  no way to tell.
- After adding the labels seen here, 10% of tabs still have a row the parser cannot name, such as
  `Fc` (finger clicks), `E1` (an electronic pad) or `Tm`.

### Other findings

- **Repeats** were applied in 16% of tabs and added 847 bars. Whether `x4` means 4 plays or 4
  repeats is not settled.
- **Tempo** is given in only 11% of tabs, so the user must usually set it.
- **Merged hits** (two on one step, from 32nd notes) were 1.2% of hits; hits moved by scaling odd
  widths were 8.5%.
- **Checking the columns:** of bars with a snare hit, 73% have one on beat 2 or 4. Bars whose width
  was fixed score 70%, close enough to suggest the fixes are right. A one-column error would put
  the snare on the wrong step and drop that share sharply.

## What was checked and what was not

- **Checked:** all 256 tabs were read; every result above is counted, not estimated. Four tabs,
  chosen as clean, fixed-width, repeated and triplet-width cases, were rendered through the app's
  offline renderer and added to the desktop app's project list.
- **Not checked:** nobody has listened to the renders or compared them with the songs. Whether a
  parsed tab matches the real drum part was not tested; the backbeat check is the only measure of
  correctness. Nothing was tried on Android.

## Recommendations for tab import

1. **Offer tab import as a paste box for short grooves,** not as a song importer. A user pastes a
   few bars and sees them on the grid before accepting.
2. **Show every guess and let the user fix it,** with the tab's labels next to the drums chosen
   for them. Labels have no reliable meaning on their own.
3. **Read the tab's legend first,** then a built-in label table modelled on Aerodrums' syntax.
4. **Ask about odd widths,** with the count line shown, rather than scaling silently. Offer
   "triplets" and "16ths" for 12-column bars.
5. **Ask for the tempo** unless the tab states one.
6. **Do not bundle tabs.** None of the sources allow it.

## Reproducing

The test tabs are not in the repository. Collect tabs as plain-text files, then:

```text
cargo run --release -p groove-spike -- tab <tabs folder> --csv tabs.csv
cargo run --release -p groove-spike -- tab <one tab.txt>
cargo run --release -p groove-spike -- tab-render <tab.txt> <bank.sf2> <out folder>
```
