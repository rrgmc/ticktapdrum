# TickTapDrum: design

TickTapDrum is a drum machine for people with little music knowledge. It makes drum patterns
only. The main view is a grid: instruments are listed on the left, and each box on the right is a
moment in time where that instrument can play.

The app started as a **proof of concept**, which checked that the technology works and that the
core ideas feel right. Its steps and findings are kept below, under "Proof-of-concept
milestones".

## Product name

The application is called **TickTapDrum**. The name says what you do: tap a box, and a drum
ticks or booms. It is an English word made up for the app, and it is not translated.

During the proof of concept the app had working names, "Drum machine" and "Bateria eletrônica".
Older findings below still use them. The identifiers that carry the name are:

| Where | Identifier |
|---|---|
| Desktop program, Linux package, data folder | `ticktapdrum` |
| Windows and macOS | `TickTapDrum.exe`, `TickTapDrum.app` |
| Android application ID | `dev.rrgmc.ticktapdrum` |
| iOS and macOS bundle IDs | `io.github.rrgmc.ticktapdrum`, `io.github.rrgmc.ticktapdrum.desktop` |
| Native library for Android and iOS | `libticktapdrum` |

The crates keep their `dm-` prefix, and the development variables keep their `DM_` prefix.

A search on 2026-11-19 found no app, product or US trademark with this name. The nearest US
trademark is TAP DRUMS, for drum practice pads and fidget toys. The domains `ticktapdrum.com` and
`ticktapdrum.app` had no DNS records, so they are probably unregistered. Before release, check the
USPTO and INPI (Brazil) records directly and register both domains.

### Icon

The icon is a drum seen from above, on a deep indigo background. Its head carries a 4 × 4 grid
of steps, some lit, like the app's main screen. One column is highlighted as the playhead, and a
yellow drumstick comes in from the corner to tap the top-right step. The sketch is in
[`icon/ticktapdrum.svg`](icon/ticktapdrum.svg), with a single-colour version for themed and
tinted icons in [`icon/ticktapdrum-mono.svg`](icon/ticktapdrum-mono.svg).

Both are drawn on the 108 × 108 Android adaptive-icon canvas, where launchers show the middle
72 × 72. They are sketches that settle the design, not final art.

The `dm-icons` tool (`tools/icons`) renders both drawings into every platform's files: an
Android adaptive icon (background colour, foreground and monochrome layers), the iOS icon, and
the rounded desktop icons (the window icon, the Windows `.ico`, and the source of the macOS
`.icns`). The iOS and desktop icons show the middle 72 × 72 of the canvas. At 16 px the grid is
lost, at 24 px it is barely readable, and from 32 px it reads clearly, so the smallest sizes may
later need a simpler drawing. Checked on Windows: the window's title bar, the taskbar and the
program file show the icon. Checked on the Android emulator: the app drawer shows the icon with
the name TickTapDrum. Not checked: the themed (monochrome) Android icon, iOS and macOS.

## Terms

The app uses these words. Each is defined once here.

| Term | Meaning |
|---|---|
| **Step** | One box in the grid. A step is either on (the drum plays) or off. |
| **Bar** | A group of 16 steps, which is 4 beats of 4 steps each. |
| **Pattern** | A grid of bars for all instruments, played as a loop. Patterns are named A, B, C and so on. |
| **Song** | An ordered list of patterns, such as A A B A, played one after another. |
| **Row** | One instrument line in the grid, such as "Kick" or "Snare". |
| **Kit** | A set of drum sounds that belong together, such as "Standard" or "808". |
| **Tempo** | The speed, in beats per minute (BPM). |
| **Swing** | A delay on every second step that gives a relaxed, "shuffle" feel. |
| **Velocity** | How hard a drum is hit, which sets how loud it sounds. |

## Requirements

### Grid and timing

- The time signature is fixed at 4/4 with 16 steps per bar. Other time signatures are out of scope.
- A new project starts with one pattern of one bar.
- One tempo applies to the whole song.
- One swing control applies to the whole song. Zero means straight timing.

### Patterns and bars

A pattern has a bar count. Adding a bar is one click.

**Removing the last bar deletes it with its notes**, after asking ("Remove bar 2? Its notes go
too."). Nothing is kept hidden for later: bars that vanished and came back with their notes when
a bar was added again confused people, so the interface no longer hides bars. The file format
still holds steps beyond the bar count, so a file written that way loads unchanged, but the app
no longer makes them.

The same rule applies to patterns. Removing a pattern from the song does not delete the pattern.
Deleting a pattern is a separate, explicit command.

### Song

- A song is an ordered list of pattern references. The same pattern may appear more than once.
- The song plays its list in order and can loop.
- A pattern can also be played on its own, on a loop, while it is being edited.

### Steps and loudness

- **The step levels button in the strip chooses what a tap on a step does.** Each tap on the
  button moves to the next of three choices, saved with the project:
  - **On and off** (one bar lit, the default): a tap turns a step on or off.
  - **Soft** (two bars): a tap goes on, soft, off. A loud step becomes soft.
  - **Loud** (three bars): a tap goes on, loud, soft, off.
- The icon is three rising bars, painted rather than taken from the icon font, whose signal
  bars leave the empty ones out. With room for words it also reads "On/off", "Soft" or "Loud".
- A soft step fills the lower half of its cell and is fainter; a loud one fills the cell and
  carries a light top edge, so the level reads by shape as well as colour. A drag paints the
  level the first step touched received.
- Steps keep their loudness whichever choice is made, so changing it loses nothing.
- A beat to start from picks the fewest levels that reach its notes.

### Rows

- A new project starts with a default set of rows: kick, snare, closed hi-hat, open hi-hat, clap,
  low tom, mid tom, high tom, crash and ride.
- Users can add rows from the General MIDI drum sounds. They can also remove, reorder and rename
  rows, and change a row's drum.
- **Choosing a drum shows 12 common ones first:** kick, snare, side stick, clap, closed, pedal
  and open hi-hat, low and high tom, crash, ride and cowbell. "More drums" opens the other 35.
  It opens by itself when the row's drum is not a common one.
- **Each drum family has a colour:** kicks red, snares and claps orange, hi-hats yellow, toms
  green, cymbals blue, everything else purple. A row's lit steps take its colour, and so do a
  stripe beside its name and one on each drum in the drum list. Loud and soft steps still differ
  in shape as well, so the grid reads without colour vision.
- **Rows can show pictures**, for children who cannot read yet. ☰ → "Rows: …" cycles through
  three choices, a device setting:
  - **Names** (the default): the name with the family's stripe.
  - **Pictures and names**: a picture with the name, the name under it on a tall row and beside
    it on a short one. With the name under the picture, the column is as wide as the longest
    name on one line needs, between 64 and 112 points, so short names leave more room for the
    steps. With the name beside the picture it is 112 points. With names alone the column also
    fits the longest name, from 96 points up to a quarter of the grid's width, so a desktop
    window shows whole names and a phone keeps room for its steps.
  - **Pictures**: a larger picture alone. The column of names narrows from 96 to 60 points, so
    the steps get the room.
  - With either picture choice, the drum list shows each drum's picture beside its name.
- **The pictures are drawn in code** from simple shapes (`crates/dm-ui/src/pictures.rs`), so they
  are sharp at any size, take the family's colour and need no image files. Related drums get
  related pictures that still differ: toms shrink as they get higher and floor toms stand on legs;
  a closed hi-hat has its cymbals together, an open one apart, and a pedal hi-hat has a foot
  pedal; a crash cymbal is tilted and a ride has a bell. Kick, snare, side stick, clap,
  tambourine, cowbell, maracas, bongos, conga, timbale, whistle, guiro, claves, wood block and
  triangle each have their own; anything else is a plain drum.
  - **Checked on the Android phone by screenshot:** pictures and names, and pictures alone, in
    portrait; the drum list with pictures, common and "More drums"; the ☰ item cycling back to
    names. Later: in landscape on the phone, pictures and names put the picture beside the name
    and read well; pictures alone are small there, because the short rows limit their height.
    In the light theme, on the Windows desktop, the pictures take the darker family shades and
    read well. Long names, such as Portuguese "Chimbal fechado", are cut short under the
    picture.
- Each row has mute, solo and volume controls.
- Clicking a row's name plays its sound, so users can hear what it is.
- **A row's settings open from its name in the grid** (see "One control strip"). The grid shows a
  muted row faded with an M, and a soloed row with an S; while any row is soloed, the others are
  faded too.
- **Removing a row removes its notes in every pattern**, so it asks again when the row has notes.
  Moving a row moves its notes with it.
- **Volume scales each note's velocity.** The synthesizer turns velocity into loudness on a
  decibel scale, so half the slider sounds about half as loud. In kits with layered samples, a
  quieter row may also use its softer sample. Mute and solo are a volume of 0 to the engine.
- **Kits come from the bank.** The short list is Standard 1, Room, Hiphop, Power 1, Electro,
  TR-808, Dance, TR-909, Jazz 1, Brush and Orchestral, where the bank has them; "All kits" lists
  every kit the bank holds (63 in ColomboGMGS2).

### Sounds

- The app plays sounds from a SoundFont (an `.sf2` file holding sampled instruments).
- The app uses **ColomboGMGS2 Vanilla 17.11**, the bank karaokemachine recommends. It was
  chosen there by ear over every other bank measured, and it holds 63 drum kits.
- **It is downloaded, not bundled.** Its licence terms are doubtful for bundling: the file claims
  CC-BY-SA 4.0, but its credits name other banks among its sample sources.
- Users can load their own `.sf2` file.
- One kit applies to the whole project.
- **The kit picker shows a short list** of about 8 to 12 named kits, such as Standard, Room, Power,
  Electronic, 808, Jazz and Brush. An "All kits" entry opens the rest of the bank's 63.

#### How the bank is downloaded

The app downloads an unchanged copy of the author's archive, hosted on a release of this
repository. It is byte for byte the copy karaokemachine uses. The author replaces the file at its
original SourceForge address with each new version, which would break a pinned digest. The copy
keeps one version at one address.

| | |
|---|---|
| Release | `bank-colombogmgs2-17.11` on `rrgmc/ticktapdrum` |
| URL | `https://github.com/rrgmc/ticktapdrum/releases/download/bank-colombogmgs2-17.11/ColomboGMGS2_VANILLA.zip` |
| Archive | `ColomboGMGS2_VANILLA.zip`, 237,967,501 bytes |
| Archive SHA-256 | `5ed5ba3516bcc139d4c9e3854338c892510e8d6f34a40fb2a9b423692eff411a` |
| Member | `ColomboGMGS2_Vanilla.sf2`, 274,474,922 bytes (261.8 MiB) |
| Member SHA-256 | `6a3197ad7d6d9528f204c683316aaed2b9b09db562833653bfe21a7ab1521dbb` |
| Licence | CC-BY-SA 4.0, claimed in the file; its credits name other banks among its sample sources |

- The app verifies the archive and the extracted member separately. A mismatch discards the file.
- The offer to download states the size and the licence line above, so the user knows what they
  are accepting.
- With no bank yet, the app shows the download offer and the option to load an `.sf2` instead. It
  makes no sound until one of them is done.
- The app depends on that release staying in place. Its tag does not start with `v`, so it does
  not start the release workflow, and it is not marked as the latest release.
- Risk: 262 MiB is large for a phone. A drums-only extract would be far smaller; CC-BY-SA allows
  it, provided the extract carries the same licence.

### Help for beginners

- **Preset beats.** A user can start from a ready-made beat, such as Rock, Pop, Hip-hop or Bossa nova.
- **Undo and redo** for every edit. This is the second safety net beside hidden bars.

### Saving, loading and export

- Projects save to and load from a JSON file with the extension `.drums`. The file carries a
  format number; a file from a newer version is refused with a message saying so, and a damaged
  one with a message saying what is wrong.
- **The ☰ menu** saves the project under a name, saves a copy under a new name, starts a new
  project, and opens or deletes saved ones. Starting or opening
  another project asks first when there are unsaved changes; deleting always asks.
- **Projects live in the app's own folder on every platform**, so phones need no file picker. On
  a desktop, two more buttons open and save `.drums` files anywhere, through the system's
  dialogs. Android's own file picker (the Storage Access Framework) is not used yet.
- **The app autosaves** the open project a second after each change, so nothing is lost if the
  app closes or a phone stops it in the background. Reopening the app restores it, with its name.
  Save and Save As still write named files.
- **Undo and Redo** buttons sit in the strip (Redo moves into ☰ on a narrow screen); Ctrl+Z, Ctrl+Y and Ctrl+S work with a keyboard.
  One drag that paints, one slide of a volume slider, or a run of tempo changes undo as one step.
  Undo keeps the last 100 steps; opening or starting a project clears them.
- **MIDI export** writes a standard `.mid` file for use in other music software: format 0, 96
  ticks per beat, the kit as a program change on channel 10 (no bank select, which players treat
  differently), mute, solo and volume applied, and swing in the note times. It plays the song
  once; an empty song exports the first pattern.
- **WAV export** renders the song once at 44.1 kHz, 16-bit stereo, plus 2 seconds for the last
  hits to ring out. It uses the same synthesizer and bank as the live sound, on a worker thread,
  about 200 times faster than real time, with a progress bar and a Stop button.
- **Where exports go:** a desktop asks with the system's save dialog. Android saves them in the
  app's external files folder (`Android/data/<app>/files/exports`), which a file manager or a USB
  connection can reach; the Export dialog shows the path. Sharing straight to another app is a
  later step.
- **Preset beats:** Rock, Pop, Hip-hop, Funk, Bossa nova, House and Reggae, each with a fitting
  tempo, a main pattern and usually a fill. Starting from one replaces the project, asking first
  when there are unsaved changes.
- **Swing** lives on the Sounds screen, under Feel, as a slider from 0 to 100%: 100% delays every
  second step by half a step, and about 66% is a triplet shuffle.

### Platforms

- Desktop (Windows, macOS, Linux) and mobile (Android, iOS, iPad) from the start.
- The layout must work with touch. Boxes are at least 44 pt (iOS) or 48 dp (Android) where the
  screen allows.
- **Nothing depends on hover or a right click.** Every menu opens with a tap, so a finger, a mouse
  and an iPad pencil all work. Hover text only repeats what the button already says.
- **A keyboard is optional.** The space bar starts and stops play, but only as a shortcut.
- **The screen stays on while the app plays**, so a pattern or song left playing is not cut
  short by the screen locking. After Stop the system's usual timeout applies again. Hosts that
  can do this pass a callback to `DrumApp::with_keep_awake`: iOS turns off the idle timer
  (`UIApplication.idleTimerDisabled`), Android sets the window's `FLAG_KEEP_SCREEN_ON` through
  android-activity, which applies it on the UI thread. Desktops do nothing.

#### Grid layout by screen

**The layout follows the width the app has, never the kind of device.** An iPad in Split View or
Slide Over can be as narrow as a phone, and gets the phone layout.

- **Narrower than 16 steps of 44 points** (a phone in portrait): the grid shows half a bar, 8
  steps. The bar button's arrows move by half a bar, and two dots under its label show which
  half is on screen and light the half the playhead is in. Cells may be taller than wide.
- **16 steps fit** (an iPad in portrait at about 820 points): one bar. On a short screen (under
  560 points tall, such as a phone in landscape) steps may be as narrow as 32 points, so a whole
  bar fits there too and the rows are not split again.
- **Room for more** (a large tablet in landscape, a desktop window): up to 4 bars side by side,
  each labelled "Bar n", with cells still 44 points or more.
- **Above the grid is one strip of buttons**, whatever the screen. See "One control strip".
- **Rows always fit the height.** The grid never scrolls, so on a short landscape phone in song
  mode, rows get shorter than a fingertip. Scrolling rows is a later step.
- The label column on the left stays fixed. The grid is never rotated. No orientation is forced
  by default; "Keep in landscape" in the ☰ menu locks it, where the host can
  (Android so far). The setting belongs to the device, not to a project.
- **Gestures:** one finger on the grid toggles a step, or paints across several; the first cell
  decides whether the drag turns cells on or off. Scrolling happens only inside pop-ups and
  dialogs. While one is open, a tap on the grid only closes it and changes no step.
- **Visuals:** alternate groups of 4 steps are shaded, beats are numbered 1 to 4, and the playhead
  column is highlighted.

#### One control strip

The app is meant for people who know little about technology, children included. From
milestone 8 the screen has **one row of buttons at the top, and the grid fills the rest**. Every
other control opens in a pop-up over the grid. Nothing replaces the grid, so the user always sees
the beat.

```
[▶ Play] [A ▾] [◀ Bar 1 of 4 ▶] [♩ 120] [▂▄▆] [↶] [↷] [☰]
```

| Button | What a tap does |
|---|---|
| **Play** | Starts and stops. In song mode it reads "Play song". |
| **A ▾** | Opens the pattern pop-up, which edits the patterns and the song: the switch between playing the pattern and playing the song; one large tile per pattern, "New", "Copy", "Rename" and "Delete"; then the song's play order, "Add A" and Repeat. Choosing a pattern leaves it open. |
| **◀ Bar 1 of 4 ▶** | The arrows move one screen of steps back or forward. The label opens the bar pop-up: the bar thumbnails, add a bar, remove the last bar (it asks first), copy the bar, and Follow. |
| **♩ 120** | Opens the tempo pop-up: large − and + buttons, a slider, and Swing. |
| **▂▄▆** | Step levels: moves to the next choice of what a tap on a step does (see "Steps and loudness"). |
| **↶** | Undo. Redo sits beside it when there is room. Either is in ☰ when it does not fit. |
| **☰** | Opens the menu: Undo, Redo and the tempo on one line at the top when the strip has no room for them, then Save, Save as, Open, New, Load default style, Sounds, Export MIDI and Export WAV. Last, under "Settings": Rows (names or pictures), Keep in landscape, and Language. Sounds opens the Sounds dialog: the drum kit and "Add a row". |

- **The strip never wraps and never scrolls.** Each button shows an icon and a word when there is
  room, and only the icon when there is not. If the icons still do not fit, buttons move into ☰
  in this order: Redo, the tempo, Undo. The step levels button stays. On a phone in portrait
  that leaves Play, the pattern, the bar button, step levels and ☰, with Undo in ☰.
- **Sounds is in ☰, never in the strip** (changed after milestone 8). It is changed rarely, and
  the strip has more room for the grid's own buttons without it.
- **The bar button replaces the bar strip and the half switch.** When the grid shows half a bar,
  the label reads "Bar 2 of 4" ("2/4" with icons only) with two dots under it. The dots show which half is on screen, and the
  half the playhead is in is lit, as the half switch did. The arrows move by half a bar there.
- **The bar arrows wrap round:** forward from the last screen goes to the first, and back from
  the first goes to the last, so 1/2, 2/2, 1/2 with two halves. An arrow about to wrap keeps
  its shape and turns the highlight colour, the orange of the playhead, except in a pattern of
  one bar, whose two halves just take turns. (A skip icon was tried
  first and found confusing.) With a single screen of steps both arrows are off. Checked on the Android phone, half a bar at a time.
- **Settings for one row open from its name.** Tapping a row's name plays the drum, as now, and
  opens a pop-up beside that row: play, rename, mute, solo, volume, drum, move up or down, remove, and
  the fills "every beat", "every 2 steps" and "clear". A fill makes the row play on exactly those
  steps in every bar of the pattern being edited, and is one undo step. The fills replace the
  long press planned earlier. The Sounds dialog no longer lists the rows. A drum is chosen from a
  list that replaces the pop-up's contents, because a drop-down list inside a pop-up closes the
  pop-up when an item is tapped.
- **The pop-up's ▶ button plays the drum again** at the row's volume, once per tap, and leaves
  the pop-up open. Checked on the Android phone by screenshot: three taps kept the pop-up open
  and changed no step. Nobody has listened to it yet.
- **Remove row always asks first.** For a row with notes the question says that its notes in
  every pattern go too.
- **Basic mode was removed** (2026-11-20). It hid the pattern button, Undo, Redo and the song
  for someone who wanted one beat, but it kept getting in the way of new features, and the app
  is simple enough without it. A `basic.mode` file left in the data folder is ignored.
- **The song lives in the pattern pop-up** (changed from a Song dialog in ☰): the switch between
  playing the pattern and playing the song at the top, and the play order with move and take
  out, "Add" for the pattern being edited, and Repeat at the end. It is small enough to share
  the pop-up with the patterns. While the song plays, the pattern button shows the pattern being
  heard. Checked on the Android phone by screenshot, in Portuguese.
- **A pattern's letter can be changed** (2026-11-20). "Rename" in the pattern pop-up opens a
  window with a button per letter, A to Z. Choosing a letter another pattern has swaps the two
  letters. The letter is kept apart from the pattern's engine slot, so renaming changes nothing
  that plays and the song keeps its patterns under their new letters. New and Copy take the
  first letter no pattern has. A saved file still names patterns by letter, and loading puts
  each pattern in its letter's slot. Checked on the Android emulator by screenshot: renaming A
  to B swapped the two in the tiles and in the song, and Undo brought them back.
- **Two kinds of pop-up, both from egui 0.37 itself.** A small pop-up opens under its button
  (`egui::Popup`): patterns, bars, tempo, a row's settings. A larger dialog sits in the middle
  of the screen over a dimmed grid (`egui::Modal`): Sounds, and the menu's Save as, Open,
  Load default style, Export and (when the strip has no room) Tempo. Both kinds close with a tap outside them, scroll when
  they are taller than the screen, and stay clear of the system bars. A dialog leaves some of
  the grid visible around it.
- **Every "are you sure" question opens in a small window of its own** over everything else:
  the question, then a button for each answer: the action, such as "Load", "New", "Delete" or
  "Remove", and "Cancel", the same word wherever nothing is to change. A tap outside
  it or Back counts as the answer that changes nothing. This covers Open, Open a file, Delete a
  saved beat, Load default style, New, Remove row, Delete pattern and Remove last bar. A question
  from a dialog leaves the dialog open behind it; one from a pop-up closes the pop-up first,
  because a tap on the window would otherwise count as a tap outside the pop-up. Checked on
  the Android phone: Load default style and Remove row, answered with Keep and with Back;
  later Delete pattern, Remove last bar and Delete a saved beat, each answered with yes. The
  Open question (opening another saved beat over unsaved changes) is not checked yet.
- **Icons come from the Phosphor font** (MIT licence), bundled in `dm-ui` with the code points the
  app uses. The `egui-phosphor` and `egui_phosphor_icons` crates need egui 0.36 as of
  2026-11-19; the app moves to one of them once it supports the egui version in use.

What milestone 8 found on the Android phone (1080 by 2340 pixels, 360 by 780 points):

- **In portrait the strip shows icons only**, and Redo, Sounds and Tempo are in ☰. The row
  fits with about 30 points to spare. Every pop-up and dialog opened, fitted the width and
  scrolled where needed.
- **In landscape the strip shows every button as an icon**, Tempo, Sounds and Redo included, and
  the grid shows a whole bar with rows about 20 points tall. The old layout gave them under 10.
- **The desktop window** at its default 960 points shows an icon and a word on every button.
- **Two faults were found and fixed.** Dialogs and pop-ups first scrolled at about 340 points
  even with room for twice that; they now take the height there is. A tall dialog first ran
  under the status and navigation bars; dialogs and pop-ups now stay inside the area the app
  draws in.
- **Checked by screenshot:** a fill, its undo, paging by half a bar, Play with the playhead,
  the half dots and the pattern dot, the Song dialog, Open with nothing saved, Beats to start
  from, the tempo pop-up, and that a tap closing a pop-up changes no step.
- **Checked later on the same phone:** renaming a row, choosing a drum from the short list and
  "More drums", Save as, deleting a saved beat, and both exports (a 86-byte MIDI file, and a
  776,204-byte WAV file: one bar at 100 BPM plus the 2-second tail). Five faults were found and
  fixed:
  - The on-screen keyboard flashed open and closed while a name was typed. Both name boxes
    asked for focus on every frame, and in egui that also interrupts the input method, which
    egui-winit does by turning it off and on. They now ask once.
  - The keyboard covered the Save as dialog's buttons. That dialog now sits at the top of the
    screen.
  - The Export dialog said nothing after an export; the result only reached the menu's status
    line. The dialog now shows it.
  - Remove row's question appeared below the visible part of the row pop-up. Its answers now
    scroll into view.
  - Descriptions and questions in the Export and Open dialogs wrapped around their buttons.
    They now sit on lines of their own.
- **Enter on the on-screen keyboard did nothing** at first. It reaches egui as a key press, but
  the press and the release come in one frame, and a text box reports losing focus only on a
  later one, so "lost focus and Enter pressed" never held. Enter now confirms a name whenever its
  box is open; checked by renaming a row on the phone. The app does not move anything out of the
  keyboard's way; it relies on the boxes sitting high on the screen.
- **Not checked:** the iOS build. Nobody has listened to this build, and nobody has used it who
  was not testing it.
- **The known Android restart fault appeared again.** After the activity was destroyed without
  the process ending, the next launch hung on the start-up screen until the app was stopped.
  This is the fault described under "Interface", fixed after milestone 8.

### Languages

- **Every text the app shows is translated.** The languages are English and Brazilian
  Portuguese (pt-BR).
- **The app follows the system's language** unless the person picks one under ☰, Idioma or
  Language: "System (…)", English, or Português (Brasil). The choice is a device setting, kept
  in the data folder as `language`, and takes effect at once. Language names are always shown
  in their own language.
- Any Portuguese system language gives Brazilian Portuguese; any language without a
  translation gives English.
- **Drum names are translated; names people type are not.** A row whose name is still its
  drum's default name, in any language, shows that drum's name in the current language. A
  renamed row keeps its name. Project names and preset style names (Rock, Bossa nova) stay as
  they are.
- The app's own name is not translated: it is TickTapDrum in every language, in the window
  title, the Android launcher, and the iOS and macOS bundles (see "Product name").
- Error text from the system itself (a file or network error) stays in whatever language the
  system gives it, inside a translated sentence.

#### How translations work

- Messages live in Fluent files, one per area, under `crates/dm-ui/locales/<tag>/`. Code asks
  for them with `tr!("menu-undo")` or `tr!("strip-bar", first = 1, total = 4)`. Numbers stay
  numbers, so Fluent picks plural forms, and the Portuguese bundle writes a decimal comma.
- Tests check that every language has the same messages with the same variables, that every
  message the code asks for exists, and that every message is used.
- To add a language: add it to `i18n::Language`, copy `locales/en` to a folder named after its
  tag, translate the files, list it in `i18n::resources`, and add the hosts' translated app
  name (Android `res/values-<tag>`, `locales_config.xml`, and the iOS and macOS `.lproj`
  folders).
- **Where the system language comes from.** Desktops and iOS ask with the sys-locale crate,
  which reads the preferred languages, so iOS's per-app language setting works. Android reads
  `getResources().getConfiguration().getLocales()` through JNI, at start and once a second, so
  a change of the system's or the app's own language (Android 13 and later) shows without a
  restart. android-activity's own copy of the configuration kept the old language on the
  emulator after a per-app change; it is only the fallback.

## Technology

### What comes from karaokemachine

karaokemachine already plays SoundFonts on every target platform. This app reuses its **building
blocks**, not its crates.

| Reused | Purpose |
|---|---|
| `rustysynth` (the fork karaokemachine uses) | Turns notes into sound using a SoundFont. |
| `cpal` | Sends sound to the audio device on every platform. |
| `rtrb` | A lock-free queue that carries commands from the interface to the audio thread. |
| The audio-thread rules from `km-audio` | Render in 64-sample blocks, never allocate memory in the audio callback, take commands only through the queue. |

The `km-audio` crate itself is not reused. It depends on karaokemachine's song, queue and fix
crates, and its sequencer plays MIDI files: seeking, lyrics, transpose, tempo maps. A step
sequencer is much smaller, so this app has its own.

The `km-display` crate is not reused either. It is built for a television and a remote control,
not for mouse and touch editing.

### How drum sounds are addressed

General MIDI puts drums on channel 10 and gives each sound a fixed key number. For example, 36 is
the kick, 38 the snare and 42 the closed hi-hat. A row therefore stores a key number, and a kit is a
program number on channel 10. Many SoundFonts hold several kits, such as Standard, Room, Power,
Electronic, 808, Jazz and Brush.

### Interface

The interface uses **egui**. egui draws its own widgets, so the same code runs on every
platform. Two parts still differ per platform:

- **File dialogs.** Desktop uses native dialogs. Android uses the Storage Access Framework, and iOS
  uses the document picker.
- **Audio session setup** on iOS.

**The host is eframe**, egui's own, with wgpu and Android's `NativeActivity`. It is the shortest
path to the mobile spike. `NativeActivity` replaced the planned GameActivity: it needs no Java
code and no library version kept in step with Rust, and the app needs little text input, which is
GameActivity's main advantage.

What the milestone 1 spike found on Android:

- **The emulator's Vulkan driver crashes** inside the driver before the first frame. When the
  Vulkan driver is the emulator's (`gfxstream`), the app limits wgpu to OpenGL ES. Other devices
  keep wgpu's default, Vulkan first. A real phone renders through Vulkan.
- **A destroyed activity left the app unable to start again.** winit ignores the activity being
  destroyed, and android-activity holds Android's main thread until the Rust thread stops, which
  it never does. The next launch in the same process hung on the start-up screen. Seen with
  Back in milestone 1, rotating the emulator during start-up in milestone 5, and once on the
  phone in milestone 8. Two changes deal with it:
  - **Back never destroys the activity.** It reaches the app as a key press. With a pop-up or
    dialog open it closes it, as a tap outside does. With nothing open the app moves itself to
    the background (`moveTaskToBack`), as Back does in Android 12 and later; the activity stays,
    and the app comes back where it was.
  - **When the activity is destroyed anyway, the app ends its process.** The host's logger
    watches for winit's "TODO: forward onDestroy" message and exits, so the next launch starts
    a fresh process. The app writes its autosave as soon as it loses focus, which comes first,
    so nothing is lost. See [`upstream-fixes.md`](upstream-fixes.md), entry 5.
  - **Checked on the phone:** Back closed the pattern pop-up, the Sounds dialog and a row's
    pop-up; with nothing open it sent the app to the background, and it came back in the same
    process. Relaunching with the "clear task" flag destroyed the activity: the process ended,
    and the next launch started a new one with the project restored. That relaunch itself did
    not show the app, because its new activity went with the old process; a second launch was
    needed. A real user meets destruction only when Android reclaims the app in the
    background, where this does not matter.
- **System-bar insets come from Android.** eframe does not report them, so the host reads the
  window insets through JNI (the system bars and the camera cutout) and keeps that much room on
  each side. It reads them again when the size changes and once a second, because turning the
  phone half a circle moves the cutout to the other side without changing the size.
- **Home and return, and rotation, work.** The steps survive both.
- **Audio delay.** `play:` lines in the log report the measured output latency each time Play is
  pressed. On Android the app opens the output with AAudio directly, not through cpal
  (`crates/dm-audio/src/aaudio.rs`).
  - cpal's `realtime` feature asks AAudio for low-latency mode, always, with no fallback. Once,
    a real phone granted it (the MMAP path), then its audio driver failed to route the stream,
    and every start failed with "Disconnected". So Android builds leave `realtime` off.
  - The app tries, in order: low latency and exclusive; low latency and shared; the default
    mode. An attempt fails if the stream does not open or start, reports an error, or makes no
    audio callbacks within a second. A failed stream is closed before the next attempt. The log
    says which mode runs (`audio output: AAudio ... running`).
  - On the real phone the first attempt now runs: 96-frame bursts, a 192-frame buffer, and a
    measured output latency of about 350 frames (7 ms). The default mode had given 1,600-frame
    bursts and 7,377 frames (about 154 ms). The audio driver routed the stream this time; the
    earlier failure did not happen again, so the fallback itself has not run on a device.
  - Listened to on the phone: the drums sound right, and a tap on the screen sounds with no
    noticeable delay.
  - A stream that fails later (a route change, headphones plugged) is logged and goes silent.
    It is not reopened yet.
  - The emulator refuses low-latency mode: it has no fast mixer, and its audio driver takes
    about 90 ms per write. A tap sounds about 120 to 160 ms late there.

What the first macOS desktop run found (Apple silicon, before milestone 7):

- **It works as built.** wgpu renders with Metal; the app downloads, verifies and installs the
  bank from its own offer; the Rock preset plays with a moving playhead on the default CoreAudio
  output; saving under a name, the autosave after quitting, and both system dialogs (save with
  the project's name filled in, open filtered to `.drums`) all work. A WAV export of Rock was
  measured: 10.73 s, peaks at -13 dBFS, hits every 0.27 s on the eighth-note grid. Nobody has
  listened to it.
- **The wide top row stacked the pattern tabs** one per line in the space left after the
  transport, at the desktop's default 960-point width. A wrapping row nested in another only gets
  what is left of the current line. The same fault was found on a landscape phone and fixed
  there: the tabs share the transport's row only when both fit (see "Grid layout by screen").

What milestone 7 found on iOS (Xcode 27, iOS 27 simulators, and an iPad on iPadOS 17):

- **eframe runs from an entry point `main.m` calls**, but not unchanged. Four problems in the
  libraries needed workarounds, each listed with its proper fix in
  [`upstream-fixes.md`](upstream-fixes.md):
  - The iOS 27 SDK refuses to launch an app that does not use scenes, and winit 0.30 makes its
    window without one. The `Info.plist` declares a scene, and `main.m` gives winit's window to
    it once eframe has made it.
  - eframe sized its drawing surface to the safe area while egui laid out the whole screen, so
    the picture was stretched and taps landed an eighth of the screen too low on an iPhone with
    a Dynamic Island. `main.m` hides the insets from winit's window, and the app pads itself by
    the real ones, read every frame so that rotation updates them.
  - The simulator's Metal device refused egui-wgpu's default limits; the app asks for the
    adapter's own.
  - On iPadOS 26 and later, the controls of a resizable window covered the Play button; the
    padding follows the corner-adapted safe area.
  - On a device without a notch the window reports no insets: the status bar counts only in the
    root view's safe area, and it covered the top row on the iPadOS 17 iPad. The padding takes
    the larger of the window's and the root view's insets.
- **The audio session works as briefed**: category `playback`, set in `main.m` before the
  engine starts. It also asks for a 5 ms buffer. Without that, the iPad gave 1024 frames
  (23 ms at 44.1 kHz) and the simulator 480; with it, the iPad gives 256 and the simulator 128.
- **iPad windows give the Split View widths.** iPadOS 27 runs the app in a resizable window.
  Narrowed to its minimum (about 270 points), the app switches to the phone layout with half a
  bar, as "Grid layout by screen" asks.
- **A landscape iPhone was cramped.** After the safe area, an iPhone 17 in landscape has about
  734 points of width, so the controls took five rows and the drum rows got about 5 points
  each. The compact layout for short, narrow screens (see "Grid layout by screen"), made for
  Android at the same time, fixes it on iOS too: one control row that scrolls sideways, a
  whole bar, and rows that can be tapped.
- **Exports show in the Files app**, under "On My iPhone › TickTapDrum › exports" (iPad on an
  iPad), checked on the simulator with a MIDI and a WAV file. The Projects screen said where
  with the folder's full container path; it now names that Files location instead.
- **The autosave and leaving the app work**: a project came back after the app was closed, and
  after Home and a return the app carried on.
- **Keeping the screen on while playing works** on the iPadOS 17 iPad: with a short Auto-Lock,
  the screen stayed on while a pattern played, and the log showed the idle timer turned off at
  Play. The Android side compiles and passes clippy but has not been run on a device.
- **The step levels word overlapped its painted bars** on the iPad: five spaces left too little
  room in that font. The number of spaces now comes from the measured width of a space.
- **Logs** go to the system log as well as stderr, so `log show` and Console.app read them
  under a UI test or on a device away from Xcode.
- **Checking without a person at the screen.** `tools/ios/drive.sh` runs taps, rotations, Home
  and screenshots through an XCUITest target, on a simulator or a device. It failed on the
  iPadOS 17 device ("Lost connection to testmanagerd") even with UI automation enabled;
  `xcrun devicectl device capture screenshot` still works there.

- All interface code lives in one host-independent function, so the host can change.
- Known weak spots: a fragile Android build setup, suspend and resume bugs (the audio stream
  included), and the iOS host's workarounds for winit 0.30 (see above), which a winit or eframe
  update may break or make unnecessary.
- **Fallback:** if eframe stalls on Android, switch to SDL3, whose mobile packaging karaokemachine
  already proves, with a fork of the `egui-sdl3` adapter.
- Grid cells react on press, not on click, and the grid sits outside any egui scroll area. On touch
  screens a drag inside a scroll area scrolls it, which would turn taps into scrolls.
- Row names wrap onto a second line where the row is tall enough, and end in "…" where they
  still do not fit. Portuguese drum names such as "Prato de condução" need it.

What the translation work found (Windows desktop and the Android emulator, by screenshot):

- On Windows the whole interface shows in Portuguese with the setting on pt-BR, and with no
  setting on a Portuguese Windows. Choosing English under ☰, Idioma changed every text and the
  window title at once, and the choice was kept.
- On the Android emulator the per-app language (set with `cmd locale set-app-locales`) shows
  at start, and a change while the app runs shows within a second, in both directions. The
  Portuguese menu and drum names fit the phone's width.
- Not checked: iOS and macOS (the localized bundle names and the iOS per-app language list
  need a Mac), the Android launcher name and the Settings language list on screen, Linux, a
  real phone, and every dialog in Portuguese one by one. Nobody who reads Portuguese natively
  has reviewed the wording.

### Audio thread

- The sequencer runs inside the audio callback. On each block it starts every step that falls due
  in that block, then renders the synthesizer.
- The interface never touches sequencer state directly. It sends commands through the queue:
  play, stop, set tempo, set swing, set a step, mute a row, and so on.
- On load, the interface hands the audio thread a fully built snapshot of the song, so nothing is
  allocated in the callback.
- Swing delays every second 16th-note step by a fraction of a step.
- WAV export runs the same sequencer and synthesizer offline, faster than real time, with no audio
  device.

## Data model

Shown as JSON. Field names are proposals.

```json
{
  "format": 1,
  "tempo_bpm": 100,
  "swing": 0.0,
  "step_levels": "on_off",
  "soundfont": { "kind": "colombogmgs2-17.11" },
  "kit_program": 0,
  "rows": [
    { "id": 1, "name": "Kick", "key": 36, "volume": 1.0, "mute": false, "solo": false }
  ],
  "patterns": [
    {
      "id": "A",
      "bar_count": 1,
      "steps": { "1": [ [0, 100], [8, 100], [16, 100] ] }
    }
  ],
  "song": { "order": ["A"], "loop": true }
}
```

- `steps` maps a row id to a list of `[step index, velocity]` pairs. Only steps that are on are
  stored.
- A step index of 16 or more in a pattern with `bar_count` 1 is a hidden bar's step. It is kept,
  as the "never deletes notes" rule requires.
- `format` is a version number, so later versions can read older files.
- The downloaded bank is stored by its id and version, so a file always names the exact bank it
  was made with.
- A user's own SoundFont is stored as `{ "kind": "file", "name": "..." }`. Storing only the name,
  not a full path, keeps the file portable between machines. The app asks for the file if it cannot
  find it.

## Proof-of-concept milestones

1. **Mobile spike.** An egui grid that plays a drum sound when tapped, running on an Android phone.
   This checks the riskiest part, the mobile host, first. *Done on the emulator, on Windows and
   on a real Android phone. Listened to on the phone and on Windows: the drums sound right, and a
   tap sounds with no noticeable delay on the phone.*
2. **Core loop.** One pattern, one bar, play and stop, tempo, on desktop. *Done. Checked on the
   Android emulator by screenshot: playback, the moving playhead, tempo changes while playing and
   the portrait halves following the playhead. Listened to on Windows: it sounds right.*
3. **Bars and patterns.** Bar count with hidden bars, several patterns, the song list. *Done.
   Checked on the Android emulator by screenshot, at phone and tablet sizes: hiding and restoring
   a bar with its notes, patterns A and B, a song that plays A then B and stops, and cueing a
   pattern while one loops. Copy bar inserts the copy after the bar and moves later bars, hidden
   ones included, so nothing is overwritten. Listened to on Windows: it sounds right.*
4. **Rows and kit.** Editable rows, mute, solo, volume, preview, kit choice. *Done. Checked on the
   Android emulator by screenshot: kit choice, mute and solo marks in the grid, moving a row with
   its note, the remove confirmation, renaming and adding a drum. Typing reached the rename box
   through the emulator's keyboard; whether a real phone shows its on-screen keyboard is not
   checked. Listened to on Windows: it sounds right.*
5. **Save and load** in JSON, with undo and redo. *Done. Checked on the Android emulator by
   screenshot: undo and redo, saving under a name, a new project, opening a saved one, the
   question before opening over unsaved changes, and the autosave coming back after the app was
   killed. The desktop file dialogs compile but have not been tried.*
6. **Extras.** Loudness levels, swing, preset beats, MIDI export, WAV export. *Done. Checked on
   the Android emulator: the Funk preset with its soft and loud steps, the tap cycle, the swing
   slider, and both exports. The exported MIDI file's header, tempo and kit were read back, and the
   WAV file has the expected length (11.6 s for four bars at 100 BPM plus the tail). Playback was
   listened to on Windows and sounds right. Nobody has listened to an exported WAV file yet, and
   the desktop save dialogs are untried.*
7. **iOS** build. *Done on the iOS 27 simulators, iPhone and iPad; partly checked on an iPad on
   iPadOS 17. Checked by screenshot through `tools/ios/drive.sh`: the bank download, the grid,
   the Rock preset playing with a moving playhead, tapping a step, rotation, the iPad's
   resizable window down to the phone layout, Home and return, the autosave after the app was
   closed, the on-screen keyboard, and MIDI and WAV exports showing in the Files app. While the
   simulator played, macOS showed its audio output in use; nobody listened to it. On the iPad:
   it installs, renders with Metal, downloads the bank, shows the grid, and plays: the user
   heard the beat and it sounded fine. Not checked there: the silent switch, tap delay by ear,
   and the steps the simulator covered. The brief and the findings are the next section and
   "Interface".*
8. **One control strip.** The layout in "One control strip": one row of buttons, the grid on
   the rest of the screen, and every other control in a pop-up. *Done. Checked on a real
   Android phone in portrait and landscape, and on the Windows desktop, by screenshot; see "One
   control strip" for what was and was not checked.*
9. **MIDI out** on desktop and iOS. The app sends its notes to an external MIDI device or to
   other music software. *Done on Windows, and listened to there through the Microsoft GS
   Wavetable Synth. On macOS the messages were checked through the virtual port, without sound;
   iOS compiles but was not run; Linux not yet compiled.* The brief and the findings are
   "Milestone 9: MIDI out" below.
10. **Android file picker.** Open and save projects, and export MIDI and WAV files, anywhere the
    system's file picker reaches. *Done on the Android emulator, checked by screenshot; not yet
    on the phone.* The brief and the findings are "Milestone 10: Android file picker" below.
11. **Groove import from files.** Open a groove file or a groove pack (a `.mid` or `.zip` file),
    keep only the files that are groove loops, list them, and add one to the project as a
    pattern. On desktop and Android, with drag and drop on a desktop. *Done on the Android
    emulator, checked by screenshot; not on the phone or a desktop, and no preview listened to.*
12. **Share to the app on Android.** "Share" a download link or a groove file to TickTapDrum, and
    open `.mid` and `.zip` files tapped in other apps. *Done on the Android emulator, checked
    with intents sent through adb and by screenshot; not from Chrome or Files by hand, and not
    on the phone.*
13. **Add from a link.** Paste a link to a groove file or pack, and the app downloads it. *Done on
    the Android emulator, with a real pack downloaded from Groove Monkee; pasting from the
    clipboard not checked.*

    The brief for milestones 11 to 13 is "Groove import" below, with what they found.
14. **MIDI in** on desktop and iOS. The user records a beat by playing along to the loop on the
    keyboard chosen as MIDI output. *Built; checked on Windows over a MIDI loopback port, not yet
    with a real keyboard.* The brief and the findings are "Milestone 14: MIDI in" below.

## Milestone 7: iOS

This section is written for whoever builds the iOS app, likely an agent on a Mac opening this
repository for the first time. Read [`BUILDING.md`](../BUILDING.md) for the tools a Mac needs,
and run the desktop app on the Mac first (`task run`): it exercises the same audio and rendering
stack the iOS app will. Its first run is recorded under "Interface".

### Definition of done

- **One command builds it**: a script, `tools/ios/build.sh`, and Taskfile tasks `build:ios` and
  `run:ios`, matching the Android ones.
- **It runs on the iOS simulator, on an iPhone and on an iPad.** On the first start it shows the
  bank download offer; after the download, the grid.
- **Play makes sound**, and a tapped step sounds.
- **These work**: rotation; Split View and Slide Over widths on the iPad (the layout follows the
  width, see "Grid layout by screen"); leaving the app and coming back; and the autosave bringing
  the project back after the app is closed.
- **Exports appear in the Files app**, under the app's folder.
- **A real device is optional.** It needs the user's Apple development team ID, which stays in
  `CLAUDE.local.md` and never goes into a committed file.
- Every new finding goes into this document, as the Android findings did under "Interface".

### Recommended route

Mirror the iOS port of karaokemachine, which already ships this audio stack (rustysynth and cpal)
on iPhone and iPad. Its repository is public: <https://github.com/rrgmc/karaokemachine>. Read
`ports/machine/ios/README.md`, `ports/machine/ios/project.yml` and `docs/architecture/ios.md`
there. Its UI is SDL3 rather than egui, but the packaging, the audio session and the folders carry
over unchanged.

- **A new crate, `apps/dm-ios`, built as a `staticlib`.** iOS does not load arbitrary dynamic
  libraries, so the Rust code is linked into the app binary. It mirrors `apps/dm-android`: it
  creates `dm_ui::DrumApp` with the data folder and the export folder it is given, and runs it
  with eframe (wgpu, which uses Metal on Apple platforms).
- **Two slices, packed as an xcframework**: `aarch64-apple-ios` for devices and
  `aarch64-apple-ios-sim` for the simulator. No Intel simulator.
- **An XcodeGen `project.yml` in `ports/ios`.** The `.xcodeproj` and the `Info.plist` are
  generated from it and git-ignored, as the Android `jniLibs` folder is. Edit `project.yml`, never
  the project in Xcode: the next build regenerates it.
- **A small `main.m`** sets up the audio session (below), works out the two folders, and calls an
  exported Rust entry point, for example `dm_ios_main(data_dir, export_dir)`.
- **The build script compiles the app**, not only the project, so a broken `main.m` fails the
  build instead of failing later in Xcode.

**Unverified: whether eframe runs from an entry point called by `main.m`.** On iOS, winit takes
over the application's main loop (`UIApplicationMain`), and eframe has no official iOS example.
Check this first, with a window that shows a single button. If it does not work:

1. Try cargo-mobile2's egui template (`cargo mobile init`), which generates an Xcode project
   around a Rust entry point, and keep the rest of this plan.
2. As a last resort, host egui on SDL3 the way karaokemachine hosts its UI. The research note
   [`research/open-questions.md`](research/open-questions.md) describes that route and its cost.

### iOS specifics

- **The audio session must be set up by the app.** cpal's CoreAudio backend configures no
  `AVAudioSession`. Without one, the app gets the `soloAmbient` category: the ringer switch mutes
  it, and sound stops when the app leaves the screen. `main.m` must set the category to `playback`
  and activate the session before the audio starts. karaokemachine found this the hard way: every
  log line said the engine had started, and nothing was heard. Declare no background audio mode;
  the app pauses when it leaves the screen, like the Android app.
- **Folders.** The data folder (bank, projects, autosave) goes in Application Support inside the
  app's container. Exports go in `Documents/exports`, and the `Info.plist` sets
  `UIFileSharingEnabled` and `LSSupportsOpeningDocumentsInPlace`, so they show in the Files app
  and in Finder over USB. Pass the export folder with `DrumApp::with_export_dir`, as the Android
  host does.
- **Screen edges.** egui-winit applies the iOS safe-area insets itself (notch, home indicator), so
  the iOS host probably should not add the fixed padding the Android host uses
  (`DrumApp::with_padding`). Check on a device or simulator with a notch. *Found otherwise:
  eframe's surface did not match egui's layout, so the host now hides the insets from winit and
  pads the app itself, every frame. See "Interface".*
- **Project settings**: `TARGETED_DEVICE_FAMILY` `1,2` (iPhone and iPad); exclude `x86_64` from
  simulator builds; deployment target iOS 15.0, and export `IPHONEOS_DEPLOYMENT_TARGET=15.0` for
  cargo too, so code compiled by the `cc` crate matches.
- **The bank** (238 MB) downloads on the first start, as on Android. The download uses rustls with
  its own root certificates, so it needs nothing from the system. Whether a large download over
  mobile data needs a warning is a product question to raise, not to decide alone.
- **The file dialogs** are already turned off on iOS (`crates/dm-ui/src/files.rs`); the Projects
  screen hides those buttons there.
- **Found in milestone 7** (details under "Interface"):
  - the on-screen keyboard appears for a project name, and typing reaches the field (simulator);
  - winit did not stop redrawing: the playhead kept moving, and the grid followed it to the
    next half bar, in runs of up to 20 seconds.

## Milestone 9: MIDI out

The app sends what it plays to a MIDI output: a hardware drum module or synthesizer, or another
program such as a DAW (a digital audio workstation). The notes are already General MIDI: drums
on channel 10, one key number per sound, and the kit as a program change. So a GM device plays
them with no mapping.

**Desktop (Windows, macOS, Linux) and iOS only.** Android is left for later; see "Android, later"
below.

**Setting it up must be one choice and no questions.** The user picks an output from one list,
and everything else follows from that choice. The app asks no questions about it and adds no other
switches.

### Definition of done

- **Settings has a "MIDI output" item.** It lists the outputs the system reports, plus "None",
  the default. Picking one is the whole setup. The choice belongs to the device, not to a project,
  like Keep in landscape. The list refreshes each time it opens, so a device plugged in after the
  app started shows without a restart.
- **The internal sound follows the choice.** With an output chosen, the app's own synthesizer is
  silent, so the drums are not heard twice. With "None" it plays again. There is no separate
  switch.
- **Play shows where the drums go.** While the chosen output is connected, a small blue badge with
  piano keys sits in the corner of the Play button, and its hover text names the output. It takes
  no room in the strip.
- **The choice is remembered by name.** On the next start, or when the device is plugged in
  again, the app reconnects to it with no action from the user.
- **Play sends the steps to that output** as note-on and note-off messages on channel 10, with
  each step's velocity. Tapping a row's name sends that note too.
- **The kit choice sends a program change**, when the output is chosen and whenever the kit
  changes.
- **Choosing another output, or None, silences the old one.** The app sends a note-off for every
  sounding note, then "All notes off" (controller 123) on channel 10. Stop lets hits ring out, as
  the internal sound does; their note-offs follow on the usual schedule.
- **A device that disappears** (unplugged, or the other program closed) does not stop playback.
  The internal sound comes back at once, and one line says the device is gone. The remembered
  choice stays, so the app reconnects when the device returns. Nothing asks the user to confirm.
- **Checked on a real target:** a hardware or software GM synthesizer on Windows, and on macOS or
  iOS if a Mac is available. Record what was heard and what was not checked.

### Recommended route

- **The `midir` crate** (0.11) for all four platforms. It uses WinMM on Windows, CoreMIDI on macOS
  and iOS, and the ALSA sequencer on Linux. It adds few dependencies: on Windows only
  `parking_lot`, since the `windows` crate it uses is already in the lock file through cpal; on
  Linux the `alsa` crate is shared with cpal too; Apple targets add the `coremidi` crate. It lives
  in its own crate, `crates/dm-midi`, so `dm-audio` stays free of it. On Android `dm-midi` builds
  as a stub that offers no outputs, and the menu item does not show.
- **The audio thread never calls a MIDI API**, because those calls can block. Where the sequencer
  calls `synth.note_on`, it also pushes the note into a second `rtrb` queue, with its frame time.
  A MIDI thread reads that queue and sends each message when it falls due. MIDI note-offs have
  deadlines of their own, 100 ms after each hit (`midi_off_at`, `MIDI_NOTE_SECONDS`), apart from
  the synthesizer's 4-second ring; see "Hits need their own note-offs" below.
- **The frame time becomes a wall-clock time** from the audio stream's own timestamps (cpal
  reports when each block will be heard). Then MIDI and internal sound stay in step.
- **The MIDI thread waits until each message is due.** midir sends at once on every platform: it
  has no scheduled send on WinMM or ALSA, and its timestamped CoreMIDI send is for macOS only. So
  the thread sleeps until the next message is due, at most about 2 ms at a time, and sends it
  then. Measure the jitter (how much each hit is early or late) before adding anything cleverer.
- **Silencing the internal sound** is a gain of zero on the synthesizer's output. The sequencer
  keeps running, so the playhead and WAV export are unaffected.

### Platform specifics

- **Windows:** older drivers let only one program open a MIDI port at a time, so opening can
  fail if a DAW holds the port. Show that as a plain message. Windows cannot create a virtual
  port for another program on the same computer; that needs a loopback tool such as loopMIDI.
- **macOS and Linux:** `midir` can also create a virtual output port named after the app, which
  a DAW sees as an input. Offer it as one more entry in the list.
- **iOS:** USB MIDI devices (through the camera adapter) and network MIDI sessions appear in
  CoreMIDI with no extra work. Bluetooth MIDI devices must first be paired through Apple's
  `CABTMIDICentralViewController`; leave Bluetooth out of this milestone. CoreMIDI needs no
  permission entry in the `Info.plist`.

### Left out

- **MIDI clock** (24 messages per beat, plus Start and Stop), which makes external gear follow the
  app's tempo. Start and Stop can set another sequencer running when the user did not expect it,
  so it would need its own switch. That conflicts with the one-choice setup, so it waits until
  someone asks for it.

### What milestone 9 found

- **How it is built.** `dm-audio` queues the messages (`Engine::take_midi_events`,
  `Engine::set_midi_out`); `dm-midi` sends them on its own thread; `dm-ui` holds the setting
  (`src/midi.rs`, kept in the data folder as `midi.output`). MIDI out is the Cargo feature `midi`,
  on by default in the desktop and iOS apps and off on Android; see `BUILDING.md`.
- **Checked on Windows, by screenshot of the desktop app:** the "MIDI output" item under
  Settings (in Portuguese), its list (None, then the system's outputs), choosing the Microsoft GS
  Wavetable Synth, the notice "Sending to …" at the bottom of the screen, Play and Stop, the
  automatic reconnect after a restart, and choosing None again, which removes the saved choice.
- **Timing on Windows.** The `engine` example played a beat through the real audio engine to the
  GS Wavetable Synth: every message went out 0.08 to 0.71 ms after its due time (median 0.36 ms),
  and none early. The `send` example gave the same picture over 16 bars (maximum 0.84 ms). This
  measures when the message reaches WinMM, not when sound leaves the synthesizer.
- **Hits need their own note-offs.** At first MIDI out used the synthesizer's note-off deadline:
  one per key, 4 seconds after the key's last hit, moved by each new hit. So a sound played at
  least every 4 seconds got no note-off while the beat played. That was assumed harmless, since
  General MIDI drum sounds ignore note-off. It is not. A hardware arranger keyboard went fully
  silent about 18 to 20 seconds into a beat, both from a Mac through a Bluetooth LE MIDI adapter
  and from an iPad over a cable. A throwaway probe reproduced it on the Mac: the same beat with
  note-offs 4 seconds late went silent at the same point while the messages kept going out, and
  with a note-off 100 ms after every hit it played for a full minute. Probably the keyboard keeps
  a voice for each note that has not ended, and runs out of them; that was not confirmed. Now
  MIDI out sends each note-off 100 ms after its hit, and a repeat hit on a key whose note has not
  ended sends that note-off first. The synthesizer keeps its 4-second ring.
- **Listened to on macOS, after that fix.** The `engine` example played its beat through the real
  audio engine to the same keyboard, through the Bluetooth adapter, for 90 seconds, and the user
  heard it play throughout. It sent 542 note-ons, 542 note-offs and 2 program changes. Messages
  went out a median of 0.37 ms after their due time (95th percentile 0.89 ms, worst 1.0 ms). Then
  the user played the desktop app to the keyboard, and everything played well. A release build with the fix also worked from the
  iPad over a cable.
- **Notices.** Messages in the menu's status line are seen only when ☰ opens, so MIDI messages
  also show for four seconds as a line at the bottom of the screen.
- **WinMM truncates port names** to 31 characters. Listing and connecting see the same truncated
  name, so choosing by name still works.
- **Listened to on Windows:** the user played the app through the Microsoft GS Wavetable Synth,
  and it worked.
- **Checked on macOS (Apple silicon), without sound.** Format, clippy and all tests pass, and
  `tools/ios/build.sh` builds the simulator app and the unsigned device app, so the CoreMIDI code
  compiles for macOS and iOS. The Mac had no MIDI outputs besides the app's own virtual port
  "Drum machine", so the `ports` example listed only that. The `engine` example played into the
  virtual port, and a small CoreMIDI listener (a throwaway program, not in the repository)
  received a program change and note-ons on channel 10: kick (36), snare (38) and hi-hat (42),
  one every 250 ms, with the expected velocities. The desktop app, started with "Drum machine" as
  the saved choice, opened the virtual port by itself, and the listener connected to it.
- **Timing on macOS.** Messages went out a median of 0.27 to 0.33 ms after their due time, none
  early. The worst was 0.52 ms in one run and 5.8 ms in another, which looked like a single delay
  while the listener started.
- **A command-line CoreMIDI program misses ports added after it starts.** The listener saw the
  virtual port only when the port existed before the listener started, probably because it has
  no run loop to receive CoreMIDI's notifications. The app has an event loop, so its list should
  refresh, but that was not checked.
- **Bluetooth MIDI on Windows is not reached (2026-11-20).** WinMM never lists Bluetooth LE MIDI
  devices, so a spike tried midir's `winrt` feature, which swaps WinMM for the WinRT MIDI API. On
  a Windows 11 PC with Windows MIDI Services (SDK 1.0.17-rc.4) installed, the `ports` example
  listed exactly the same outputs with both backends: the GS Wavetable Synth and the service's
  loopback ports. The keyboard's Bluetooth adapter was paired, and the keyboard was on, but
  neither backend listed it. Windows MIDI Services sends both older APIs through its own service
  (`midisrv`), and that service had no Bluetooth transport installed (only kernel-streaming,
  loopback and virtual). So on such a PC the `winrt` feature gains nothing, and the spike was not
  kept. Microsoft ships a Bluetooth LE MIDI transport and a "Windows MIDI Bluetooth Setup" app as
  a preview, separately from the service: the Transports package of "In-box Preview 14" (Transports
  1.0.52-preview.14, Tools 0.99.103-preview.14, November 2026; Windows 11 25H2 or later). A device
  connected there appears to every MIDI app like a wired one, so TickTapDrum needs no change.
- **Listened to on Windows over Bluetooth (2026-11-20).** With Preview 14's Tools and Transports
  installed and the adapter connected in Windows MIDI Bluetooth Setup, the default WinMM build's
  `ports` example listed the adapter by name. The `engine` example played a beat to it for 20
  seconds: 120 note-ons, 120 note-offs and 2 program changes. Messages went out a median of
  0.37 ms after their due time (95th percentile 0.71 ms, worst 0.93 ms), and the user heard the
  keyboard play. The radio latency after sending was not measured. Without the preview the user
  needs a cable, or a USB dongle that presents itself as a USB MIDI device. A PC without Windows
  MIDI Services, where WinRT still has its own Bluetooth path, was not tried.
- **Not checked:** another program as the output; unplugging and
  replugging a USB device while connected; a port held by another program. On macOS: the "MIDI output" dialog by hand, choosing None or another output, the IAC bus, and the list
  picking up a port added while the app runs. On iOS, only a cable to the keyboard was tried. Linux has not even
  been compiled. On Linux, ALSA port names include client numbers, which may change when a device
  is plugged in again and break reconnecting by name.

### MIDI out volume (2026-11-20)

Each row's volume already reaches MIDI out, because it scales the velocity of every note the row
sends. Nothing set the level of the device as a whole, so it played at whatever level it was left
at.

**The MIDI output dialog has a Volume slider**, 0 to 100%, shown while an output is chosen. The app
sends it as controller 7 ("channel volume") on channel 10. That is the General MIDI way to set a
channel's level.

- **Why controller 7, and not lower velocities.** Velocity scaling would work on any device, but
  many kits play a different sample for soft and hard hits. Lower velocities would change how the
  drums sound, not only how loud they are. Controller 7 changes only the level. Some devices
  ignore it; on those the slider does nothing.
- **Mapping.** 100% is 127, the controller's top, and the default. 0% is 0. Values between are
  rounded, so 79% is 100, the General MIDI reset value.
- **When it is sent.** When MIDI out turns on, right after the kit's program change, and each time
  the slider moves. A device keeps the last value it got, from any program, so the app sends it
  again on every connect.
- **Where it lives.** `Engine::set_midi_volume` in `dm-audio` (the renderer keeps the value and
  resends it); the setting in `dm-ui` (`src/midi.rs`), kept in the data folder as `midi.volume`
  while it is not 100. It belongs to the device, not to a project. The internal synthesizer
  ignores it.

**What was checked.** Unit tests cover the bytes sent, the resend on enable, the synthesizer
staying unchanged, the percent mapping and the stored setting. On Windows, the `engine` example
(which now halves the volume halfway through) sent `[127, 64]` to the Microsoft GS Wavetable Synth
with the beat, on time.

**Not checked.** Nobody has listened to the volume change on any device. The slider was not seen in
the desktop app or on iOS. Choosing a second output while another is connected sends neither the
kit nor the volume to the new one, because the sender goes from connected to connected with no
gap the interface sees. That gap was already there for the kit.

### Android, later

Android's native MIDI API (AMidi, Android 10 and later) can send only to a device that Java code
has opened first through `MidiManager.openDevice`. midir 0.11 has an Android backend that does
this through JNI from Rust, so it may need no Kotlin code in the port. It was found while building
milestone 9 and has not been tried. What still stands: AMidi needs Android 10, while the audio code
targets Android 8, and Bluetooth MIDI devices need a pairing step.

midir links `libamidi` directly (its `build.rs` asks for it), and that library exists only from
Android 10 (API 29). The app's `minSdk` and `tools/android/build.sh` use API 26. So turning midir
on for Android means either raising `minSdk` to 29, which drops Android 8 and 9, or patching midir
to load `libamidi` at run time, so older phones simply do not offer the setting. Left for later
(2026-11-19).

## Milestone 10: Android file picker

Today Android keeps projects only in the app's own folder, and saves exports to the app's
external files folder (`Android/data/<app>/files/exports`), which recent Android versions hide
from most file managers. This milestone uses Android's own file picker, the Storage Access
Framework (SAF), so people can open a `.drums` file from anywhere, save a copy anywhere (Downloads,
Google Drive, a USB stick), and export MIDI and WAV files to a place they choose. iOS is not part
of it.

As with MIDI out, it should add no questions and no settings: each action opens the system's
picker once, and the person picks a place or backs out.

### Definition of done

- **The ☰ menu on Android offers the same two file items as on a desktop**: open a project file,
  and save the project to a file. Each opens the system picker: "Open" (`ACTION_OPEN_DOCUMENT`)
  and "Save as" (`ACTION_CREATE_DOCUMENT`, with the project's name as the file name). The
  projects kept in the app's folder work as before.
- **Export MIDI and Export WAV open the system's "Save as" picker** on Android, as the desktop's
  save dialog does. The file name is the project's name with `.mid` or `.wav`. This replaces the
  hidden exports folder. WAV export still renders on its worker thread, with its progress bar and
  Stop button, and writes into the chosen file.
- **Backing out of the picker does nothing**, with no message.
- **Messages name the file** as the picker reported it (its display name), not a `content://`
  address.
- **A file that will not open or save** gives the same messages as on a desktop. A damaged or
  foreign file opened through the picker is refused with the existing "did not open" message.
- **Checked on the emulator by screenshot**, through its Files app: open a `.drums` file from
  Downloads, save one to Downloads and open it again, and export a MIDI and a WAV file there. Also
  on the phone, if it is connected.

### Recommended route

- **A small Java subclass of `NativeActivity`.** The picker returns its result to the activity
  that started it, through `onActivityResult`. `NativeActivity` with `hasCode="false"` cannot
  receive it, and android-activity does not forward it. So the port gains its first Java class,
  for example `dev.rrgmc.ticktapdrum.MainActivity extends NativeActivity`, which:
  - starts the picker when Rust asks (a Java method Rust calls through JNI, with the action, the
    MIME type and the suggested name);
  - in `onActivityResult`, passes the request number and the chosen URI (or none) to a native
    method that the Rust library exports.

  The manifest names this class and sets `hasCode="true"`; Gradle compiles it. Everything else
  stays in Rust. A separate helper activity is the alternative, but it adds a second activity to
  the lifecycle that already has a known restart fault (see "Interface").
- **Reading and writing go through a file descriptor.** Rust asks the `ContentResolver` for
  `openFileDescriptor(uri, mode)` through JNI, takes the descriptor with `detachFd`, and wraps it
  in a `std::fs::File`. Then the existing code reads and writes `.drums`, MIDI and WAV data
  unchanged. Use mode `"rwt"` when writing: some document providers ignore the truncate in
  `"wt"` and leave the tail of an older, longer file behind. The display name comes from a query
  of `OpenableColumns.DISPLAY_NAME`.
- **The picker becomes asynchronous in `dm-ui`.** On a desktop `rfd` blocks and returns at once;
  on Android the answer arrives frames later, after the app has been in the background. So
  `files.rs` changes from "pick and return a path" to "ask, then collect the answer on a later
  frame": the UI asks for a pick with what it is for (open a project, save a project, export MIDI,
  export WAV), and each frame takes any finished pick and carries on. The desktop implementation
  answers in the same frame through the same interface, so both share one flow.
- **Where a path was used, a target is used**: either a path (desktop) or an opened file plus
  its display name (Android). `store::read_file`, `store::write_file` and the export writers take
  a reader or writer instead of a path; the WAV worker takes the opened file.
- **MIME types.** Projects are saved as `application/octet-stream` with the `.drums` name. (Not
  `application/json`: the Downloads provider then adds `.json`, so `Beat.drums` becomes
  `Beat.drums.json`; found on the emulator.) Open asks for any type (`*/*`), because files from elsewhere may be labelled
  `application/octet-stream`; the existing format check refuses anything that is not a project.
  MIDI is `audio/midi`, WAV `audio/wav`.
- **The host owns the JNI.** `apps/dm-android` implements the picker behind a small trait or
  callback that `DrumApp` receives, as it already does for keeping the screen awake
  (`with_keep_awake`). `dm-ui` stays free of JNI.

### Risks

- **The activity can be destroyed while the picker is open**, when Android needs the memory.
  The app then ends its process (the known restart fault), and the pick is lost. The person sees
  the app restart with the autosave and can try again. The autosave means no work is lost.
- **The picker sends the app to the background.** Audio pauses as it does for Home, and resumes
  when the app comes back. Check that playback, if it was on, behaves as after Home.
- **Some document providers misbehave** (truncation, slow cloud files). Test Downloads on the
  emulator; cloud providers are not part of the check.

### Left out

- Opening a `.drums` file tapped in another app (an intent filter for `ACTION_VIEW`), and sharing
  an export straight to another app (`ACTION_SEND`). Both are small follow-ups once the
  descriptor plumbing exists.
- The same picker on iOS (`UIDocumentPickerViewController`), where exports already appear in the
  Files app.

### What milestone 10 found

- **How it is built.** `MainActivity` (Java, in `ports/android`) extends `NativeActivity`,
  starts the picker on the UI thread and passes the chosen URI to the native method
  `onFilePicked`. `apps/dm-android/src/picker.rs` opens the URI through the `ContentResolver` on
  a worker thread, so a slow provider cannot freeze the UI thread, and hands the file to
  `DrumApp::pick_answers`. `MainActivity` also loads the library itself (`System.loadLibrary`):
  `NativeActivity` opens it from native code, which does not register it for Java's native
  method lookup. In `dm-ui`, `files.rs` routes every pick (picker, export folder, or desktop
  dialog) into one flow, and `store.rs` and `export.rs` work on a path or an opened file alike.
- **Checked on the emulator (Android 16), by screenshot, in Portuguese:** "Abrir arquivo" and
  "Salvar em arquivo" in the Open dialog; saving to Downloads under the suggested name
  (`Minha batida.drums`); changing a step and opening that file again through the picker, which
  brought the saved beat back with the project named after the file, and the message "Arquivo
  aberto: Minha batida.drums."; exporting a WAV file (its size and header match a one-bar render
  plus the tail) and a MIDI file (a valid header) to Downloads, each with a message naming the
  file; and backing out of the picker, which wrote nothing and said nothing. The app process
  stayed the same throughout.
- **The Files app lists a freshly written file as 0 B**, although it holds its bytes. Its size
  listing lags behind writes made through a descriptor; it is cosmetic.
- **A WAV export that is stopped or fails leaves an empty file** where the person chose: a file
  reached through a descriptor cannot be deleted from Rust. Deleting it through
  `DocumentsContract.deleteDocument` is a possible follow-up.
- **Not checked:** the phone; overwriting an existing, longer file (the picker offers a new name
  such as "Beat (1)" instead); cloud providers; the activity being destroyed while the picker is
  open; and the desktop's dialogs after the refactor (the desktop app starts, but its open, save
  and export were not tried).

## Groove import

People find drum grooves on the web as MIDI groove packs: short drum loops, sold or given away by
vendors such as Groove Monkee, usually as a `.zip` of hundreds of `.mid` files. These milestones let
a person bring such grooves into the app in the way that suits where they found them: by opening a
downloaded file, by sharing it or its link to the app on Android, or by pasting a link.

**The app imports groove loops only, not any MIDI file.** A whole song, a live recording, a file in
3/4 or one made for another drum program is refused, with a reason. This keeps the result
predictable for people with little music knowledge, and it is what the spikes showed works. The
evidence is in [`research/groove-import-spike.md`](research/groove-import-spike.md); plain-text drum
tabs are not part of this (see [`research/tab-import-spike.md`](research/tab-import-spike.md)).

### What counts as a groove

A MIDI file is accepted only if all of these hold. Tested on the spike's files, the rule accepts
every Groove Monkee GM file in 4/4 that sits on the grid (189 of 241), and rejects most files made
for other drum programs.

| Check | Why |
|---|---|
| Notes on one channel only, normally channel 10 | Song files also hold bass, piano and so on |
| 4/4, and no change of time signature | The grid is 4/4 only |
| One tempo | Loops do not change tempo |
| At most 16 bars | Packs hold 1 to 8 bars; songs run to hundreds |
| At most 10% of notes more than a quarter step off the 16-step grid | Refuses live playing, triplets and 32nd-note rolls |
| At least 90% of notes on drum-kit keys (General MIDI 35 to 59) | Catches packs made for another drum program |

The import itself follows the spike: notes snap to the grid after fitting the swing and a constant
offset of at most a quarter step; velocities are kept as they are; the tempo comes from the file,
rounded to a whole BPM; two notes of one drum on one step become the louder one.

**A pack** (a `.zip`, or several files at once) goes through the rule file by file. When a zip holds
several versions of the same grooves for different drum programs, the import takes the folder whose
name says GM or General MIDI and ignores the others. The result is a count with reasons, such as
"189 of 241 grooves added; 52 were not in 4/4".

**Refusals say why**, in words a beginner understands: "This file is in 3/4", "This looks like a
whole song, not a groove", "This seems made for another drum program; try the pack's General MIDI
version", "This is a web page, not a groove file".

### Where grooves go

Accepted grooves go into a **groove list** kept in the app's folder, named after their files and
grouped by the pack (or the file) they came from. From the list a person can play a groove, delete
it, and **add it to the project as a new pattern**, using the project's own tempo and kit. Adding
asks nothing more; undo removes it. A groove whose drums are not among the project's rows adds the
rows it needs.

Every import, a single file or a pack, opens the groove list on the new pack, so a single groove
is one tap away from Play or Add. Re-importing a file of the same name replaces its pack.

### Milestone 11: groove import from files

#### Definition of done

- **A new crate, `dm-groove`,** turns bytes into accepted grooves and a report: `.mid` and `.zip`
  input, the rule above, the GM folder choice, and the snapping. It has no interface and no
  platform code, so it is tested on its own, with the spike's checks as tests (the export and
  re-import of every preset gives identical steps).
- **The ☰ menu has "Add grooves from a file"** on desktop (the system dialog, `.mid` and `.zip`)
  and on Android (the system picker from milestone 10, asking for MIDI and zip types).
- **A desktop accepts files dropped on the window.**
- **The groove list and "Add as pattern"** as described above.
- **Checked** on Windows and on the Android emulator, by screenshot, with Groove Monkee's free GM
  pack and a refused file (a song, a 3/4 groove, a Perfect Drums file).

#### Recommended route

- `midly` reads MIDI; the workspace's `zip` crate, already used for the bank, reads packs.
- Limits against damaged or hostile files: at most 2,000 files and 100 MB unpacked in a zip, at
  most 1 MB per MIDI file. Files are recognised by their content (`MThd`, a zip header), not their
  names.
- The groove list stores each accepted groove in the app's own format, not the original file, so
  later imports do not depend on the parser.

### Milestone 12: share to the app on Android

#### Definition of done

- **TickTapDrum appears in Android's Share sheet** for text (a link) and for MIDI and zip files,
  and in "Open with" for `.mid` and `.zip` files (`ACTION_SEND` and `ACTION_VIEW` intent filters).
- **A shared link** goes through milestone 13's download, so milestone 12 needs it; a shared or
  opened file goes straight to the import.
- **It works whether the app is running or not.** The activity is `singleTask`, so a share into a
  running app arrives through `onNewIntent`, not as a second copy.
- **Checked** on the emulator and on the phone: share a link from Chrome, share a downloaded file
  from Files, and tap a `.mid` file in Files.

#### Recommended route

- `MainActivity` reads its starting intent and `onNewIntent`, and passes a link or a `content://`
  address to Rust through a second native method, beside `onFilePicked`. Files open through the
  same `ContentResolver` code as the picker.

### Milestone 13: add from a link

#### Definition of done

- **"Add grooves from a link"** in the ☰ menu: a text box, an Add button, and the download's
  progress. It accepts `http` and `https` links to a `.mid` or `.zip` file. A GitHub `…/blob/…`
  link is changed to its raw file.
- **A web page is refused** with the message above; the app does not search the page for links.
- **Limits:** a 30-second timeout and the size limits of milestone 11, checked while downloading.
- **Checked** on Windows and on the phone, with a Groove Monkee zip link, a single `.mid` link, a
  web page and a link that fails.

#### Recommended route

- Download with `ureq`, as the bank download does.

### Risks

- **Pasting may not work on Android.** egui's clipboard support on Android is limited, and its
  text input there is unreliable (see "Interface"). Check on the phone first. If pasting fails,
  milestone 12's Share is the way to add a link on Android, and the link box stays a desktop
  feature.
- **Free packs often sit behind a checkout or a web page**, so a link to the download itself is
  not always available. Saving the file and opening it (milestone 11) always works.
- **The rule may refuse grooves people want,** such as shuffles and triplet feels, which the grid
  cannot hold. The refusal message should say so.
- **Licences.** Packs forbid redistribution. The app only stores the person's own copy, and never
  uploads or shares it.

### Left out

- **iOS.** Sharing into an iOS app needs a share extension, a separate target, and iOS has no file
  picker yet. It follows once milestone 11 works.
- **Bundled grooves.** Groove Monkee and the other vendors forbid it. Google's Groove MIDI Dataset
  (CC BY 4.0) is the only source that may ship with the app; a curated starter set from it is a
  separate decision.
- **A help page with links to free packs**, which should tell people to download the "GM" or
  "General MIDI" version.
- **Choosing bars from a longer file**, and any other MIDI: songs, live recordings, other meters.

### What milestones 11 to 13 found

- **How it is built.** `dm-groove` reads `.mid` and `.zip` files, applies the rule, keeps packs as
  JSON, and downloads links (`ureq`, as the bank does). `dm-project` adds an import as one pattern
  (`Project::add_imported_pattern`), collapsing exact repeats. The engine has a hidden preview
  track (`Engine::preview`) with its own storage, so previewing never touches the project. In
  `dm-ui`, `import_grooves` is the one entry point for a file, whether it was picked, dropped,
  downloaded or shared; hosts hand files and text in through `DrumApp::incoming_files`. On
  Android, `MainActivity` passes shares to Rust through `onShared`, and `share.rs` queues them
  until the interface exists.
- **Checked on the emulator (Android 16), by screenshot, in English:**
  - "Add grooves from a file" opened the system picker. Groove Monkee's Chilled Beat pack opened
    as a list of its grooves; Play turned into Stop; Add made pattern B, and its 8 bars of one
    repeated bar became 1 bar, with a note saying so.
  - The 3/4 metal pack was refused whole: "Nothing was added ... 18 files were left out: not in
    4/4 (18)."
  - A link shared with the app stopped (an `ACTION_SEND` intent through adb) waited for the
    interface, downloaded Groove Monkee's free Perfect Drums pack and opened its grooves. A `.mid`
    opened with the running app (`ACTION_VIEW` with a content address) became its own pack.
  - Typing a link into "Add grooves from a link" (through the input system) and tapping Add
    downloaded the GetGood Drums sampler: 84 grooves added, 190 files left out with reasons.
  - With the experimental setting on (since removed), a typed 1-bar tab became pattern C as
    written.
- **Fixed after the first look:** names in a pack all began "076 Chilled Beat 1 ..." and were cut
  short identically, so the list now drops the words every groove in a pack shares. The
  experimental setting's label was too long for its button.
- **The mapping check catches most, not all, files made for other drum programs.** About a third
  of the GetGood Drums and Perfect Drums files still pass, probably those without a hi-hat. A
  pack's GM folder, chosen automatically, remains the main protection.
- **Found on the desktop: empty bars in GMD grooves.** Most GMD files end (their End of Track) a
  fifth of a bar to a whole bar after the last note, so a 1-bar fill became 2 bars with an empty
  second bar. Some fills start with a hit just before the second bar line, which left the first
  bar empty. The length now comes from the last note, and End of Track counts only when it is on a
  bar line; empty bars at either end are dropped. Over the GMD zip, 359 of 1,150 files are now
  accepted (286 before, because padding had pushed 73 past 16 bars), and none has an empty first
  or last bar. The desktop app also had no clipboard, so a link could not be pasted; eframe's
  `clipboard` feature is now on.
- **Not checked:** the phone; a desktop (its dialog and drag and drop); hearing a preview;
  pasting from the clipboard into a text box on Android; sharing from Chrome and Files by hand;
  iOS, which has none of this yet.

## Paste a drum tab

Plain-text drum tabs are mostly whole-song transcriptions with no standard (see
[`research/tab-import-spike.md`](research/tab-import-spike.md)), so they do not fit a simple app
as a song import. Copying a few bars of a beat from a tab is still worth having. The feature
started behind an experimental setting, off by default; after trying it on the desktop, the
author made it a plain menu item.

- **The ☰ menu's Import section offers "Paste a drum tab"**: a text box,
  and "Add as pattern". Text shared to the app that is not a link opens the same dialog.
- **The paste becomes one new pattern** after the existing ones, at most 16 bars; the rest is
  left out and the message says how many bars. The pattern uses rows that already play each
  drum and adds rows for the drums the project lacks. Tempo, swing, kit and song do not change.
  Undo removes it.
- **Nothing is ever asked.** The app reads what it can recognise on its own and refuses the rest,
  with a reason, keeping the text so it can be fixed:
  - Drum names come from the tab's own legend first, then a built-in table of common labels.
  - Rows with an unknown name are left out if they hold at most 10% of the hits; otherwise the
    paste is refused, naming them.
  - After fixing a spacer column or a bar one column short, every bar must be 4, 8, 16 or 32
    characters wide. Triplets and other widths are refused, not guessed. A checkbox in the
    dialog, "Leave out bars that do not fit", off by default and kept while the app runs, leaves
    those bars out instead and says how many; the paste is still refused if none is left. It
    exists because a whole song is often refused for one triplet fill.
  - Rows of one system with different numbers of bars are refused.
  - Repeat marks such as `x4` repeat the system; `%` repeats the bar before.
- **The parser is the crate `dm-tab`,** ported from the spike's tab parser, with tests.
- **On Android, pasting into the text box was not checked** (see "Groove import", Risks).
  Sharing text to the app covers it.

## The ☰ menu's sections

The menu grew past a screen, so two groups fold open in place: **Import** (add from a file, add
from a link, paste a drum tab, the groove list) and **Settings**. Both
start closed and stay as left while the app runs. A section folds open inside the one scrolling
list instead of opening a submenu beside it, which works the same with a finger and a mouse.

## Where to find grooves and tabs

The app points to free sources from the dialogs that take them, so nobody has to search first.
eframe's `links` feature opens a link in the system browser on the desktop and on Android.

- **"Add grooves from a link" lists free grooves.** Links to Magenta's page and to Groove
  Monkee's two free pages open in the browser, with a line saying to copy a download link there
  and paste it.
- **"Paste a drum tab" lists where to find tabs:** Ultimate Guitar's drum tabs and Tabcrawler,
  opened in the browser.
- Every address answered on 2026-11-20. Pages move, so a link may break in a later version.

## Grooves that come with the app

Every file of Google Magenta's Groove MIDI Dataset (CC BY 4.0) that the app accepts ships with
it: 359 beats and fills, whole takes of up to 16 bars, so there are grooves by real drummers
without a download. `dm-groove`'s `curate_magenta` example gathers them from the dataset's MIDI
zip and writes `crates/dm-ui/assets/magenta-grooves.json` (250 KB). Each groove's folder is its
style; the dataset's "grooveN" takes count as their style, which gives 24 styles. Within a
style the beats come first, then the fills ("Funk 3 (fill)"), each by tempo, numbered in that
order, so the list reads in the order of its names. Each drummer's take is kept, even where
several share a file name.

The author first asked for a few beats per style (25, cut to 4 bars, then whole takes), then for
all of them. Putting grooves of up to 4 bars first looked odd, as the numbers then jumped, so
the order follows the names. Rock has the most (64 beats, 28 fills); several styles have only
fills.

- **"Ready-made styles" has a button, "Grooves by real drummers",** that lists them with their
  credits and links to the dataset and the licence, by style: each style is a heading with its
  count that opens and closes. Each groove shows its tempo and length. Load starts a new project from one, as a
  ready-made style does: pattern A holds the groove at its tempo, and it asks first when there
  are unsaved changes.
- **The Grooves list always shows them first,** as a pack that cannot be deleted, with the
  credits and the same styles. Add puts one in the project as a new pattern, like any imported groove. The Grooves
  item is therefore always in the menu.
- **The credits** are in the dialog, the Grooves list, the README and
  `crates/dm-ui/assets/MAGENTA-CREDITS.txt`, as CC BY 4.0 asks.
- The link dialog no longer offers to download the whole dataset; the page link stays.

## Bars under the buttons

A setting, "Show bars under the buttons", shows the bar thumbnails from the bar pop-up all the
time, on one line under the strip that scrolls sideways. A tap on a thumbnail shows that bar, as
in the pop-up, and the line scrolls to the bars the grid shows whenever they change, such as
while following the music. Until the person chooses, it is on for a desktop and for a screen
whose short side is at least 600 points (a tablet), and off for a phone, which needs the room
for the grid.

- **The line starts with two buttons the size of a thumbnail,** which stay put while the
  thumbnails scroll: Follow (a crosshair), and a play icon with a bar number, which plays the
  pattern from the first bar the grid shows (offered for a pattern only). Moving to another bar
  by hand while playing stops following; Follow, here or in the bar pop-up, turns it back on.
  The line's scroll bar takes its own room under the thumbnails instead of floating over them.
- **While the line shows, the bar pop-up leaves its thumbnails out,** and with them "Play from
  bar N", which the line offers instead.
- **Otherwise the bar pop-up offers "Play from bar N",** N being the first bar the grid shows.
  It plays the pattern from that bar's first step, loops the whole pattern after, and follows
  it. A song plays from its start, so it is offered only for a pattern.
- **The window's title names the project** once it has one: "Name - TickTapDrum".

## Export as a keyboard style

Built; "What the build found" below says what was checked. Arranger keyboards play a "style": a drum and band backing with several
sections (main variations, fills, intros, endings) that follows the player's chords. A beat made
in the app can become the drum part of such a style.
[`research/arranger-style-export.md`](research/arranger-style-export.md) has the findings.

### What is known

- **Yamaha first** (PSR-SX720 and similar). A Yamaha style is a format-0 MIDI file with markers
  for its sections. A drum-only style needs none of the extra data blocks, so the existing MIDI
  export does most of the work.
- **The SX720 accepts a drum-only style** (checked 2026-11-20). Two test files, one marked SFF1 and
  one SFF2, with no CASM block, loaded from the User list and played.
- **A style can be sent straight to the keyboard over MIDI,** into its User drive, using Yamaha's
  file transfer messages. This worked over Bluetooth MIDI. The research note lists the messages,
  and the example program `style_send` in `dm-midi` sends them.
- **Korg Pa later, if asked for.** Korg keyboards import a MIDI file separated by markers into a
  style, so the same writer could serve both with different marker names.
- **Roland and Casio are left out:** no open format or import route was found.

### Two choices in the Export dialog

The ☰ menu's Export dialog keeps MIDI file and WAV recording, then shows a small heading, "For a
keyboard", with two more buttons. Each has a line under it, as the others do.

| Button | Line under it |
|---|---|
| **Keyboard style** | A style for a Yamaha arranger keyboard, such as the PSR-SX720. Copy it to a USB stick and choose it under User. |
| **Send to keyboard** | Puts the style straight into the keyboard's User styles, through the MIDI output. |

- **The buttons name no brand;** the lines under them do. If Korg is added, the buttons stay and
  the dialog gains a choice of brand.
- **The app already says "style" for its ready-made beats** ("Load default style"), so these
  buttons always say "Keyboard style", never "Style" alone.

### Choosing the sections

Both buttons open the same step first: a list of the style's sections, each with a pattern picker
that also offers "None". This is an export, so asking is fine, unlike imports.

- **The sections:** Intro, Main A to D each with its fill, Break and Ending. They use Yamaha's
  Intro A, Fill In AA to DD and Ending A; the other intros and endings are left out.
- **The app fills the list in from the song.** Patterns in song order become Main A, B, C and D. A
  one-bar pattern that follows a main in the song becomes that main's fill. Without a song, the
  patterns' own order is used.
- **Fills and the Break are one bar,** so their pickers offer only one-bar patterns.
- **A main with no fill uses its own last bar as its fill.** Some keyboards play the Ending where
  a fill is missing, which would surprise the player.
- **Main A is required.** Without it the button that finishes is greyed out, and a line says why.
- **What goes into the style:** the project's name as the style's name, the tempo as its tempo,
  and the kit as Yamaha's drum kit bank and program. Mute, solo, row volume and swing apply to the
  notes, as in the MIDI export. Drums go on channel 10, the main rhythm part, at 1920 ticks per
  beat, marked SFF1 (the SX720 played both SFF1 and SFF2).

### Keyboard style: a file

- It writes `<project name>.sty` wherever the other exports go: the system's save dialog on a
  desktop, the picker on Android, the exports folder on iOS.
- It works on every platform.

### Send to keyboard: straight over MIDI

- **It uses the MIDI output chosen in Settings.** There is no new setting. With no output chosen,
  the button is greyed out, with the line "Choose the keyboard under Settings, MIDI output
  first."
- **The app also listens to that device,** because the keyboard answers each message. `dm-midi`
  opens the input with the same name as the output; the Bluetooth adapter tried had one name for
  both.
- **The transfer runs on the MIDI thread,** which owns the output connection; Windows lets only
  one connection hold a port. Playback stops first. A progress bar shows while it runs; the test
  style took a second or two over Bluetooth.
- **A style with the same name on the keyboard is replaced,** so sending again after an edit
  updates it instead of filling the User list with copies. The app lists the `STYLE` folder,
  deletes the old file, checks that it is gone, then writes the new one.
- **When it ends**, one line says what happened:
  - "Saved on the keyboard as <name>. Choose it under Style, User."
  - "The keyboard did not answer. Sending works with Yamaha PSR-SX and Genos keyboards." This also
    covers a keyboard that answers as another model.
- **Desktop and iOS only.** The button does not show on Android, which has no MIDI out yet.

### Room for other keyboards

Only Yamaha is offered now, but the export is built so that another brand, such as Korg, can be
added without changing what is already there.

- **`dm-project` has a `KeyboardKind`.** Each kind says which sections its styles have, which of
  them must be one bar, its file extension, and how to write the file. The sections, the app's
  suggestion and the checks are shared by every kind. The Yamaha writer is in its own file.
- **`dm-midi` takes a target for Send to keyboard.** Each brand's transfer messages live in a
  module of their own; noticing a keyboard that is switched off is shared.
- **The interface names no brand on its buttons.** The line under each button names the brand.
  With more than one kind, the section step gains a choice of brand at the top.

### What the build found

Checked on 2026-11-20 on Windows, with a PSR-SX720 behind a Bluetooth MIDI adapter.

- **How it is built.** `dm-project` has `KeyboardKind`, `Section` and `StyleSections` in
  `style.rs`, and the Yamaha writer in `style_yamaha.rs`. `dm-midi` has the transfer in
  `transfer/` (the Yamaha session in `transfer/yamaha.rs`) and the switch-off check in
  `liveness.rs`. `dm-ui` has the section step in `keyboard.rs`. The `style_send` example uses the
  library; its `probe` command lists the keyboard's files, and `watch` prints the MIDI status, for
  testing a switch-off.
- **The writer** gives, for the same beat, exactly the bytes of the file the SX720 played; a test
  holds it to that.
- **Keyboard style** saved a valid `.sty` file through the save dialog. The dialogs and the
  pattern pickers were looked at on screen.
- **Send to keyboard** put "Minha batida.sty" into the keyboard's User drive in about two
  seconds, with the progress bar and the result line. Sending a file twice left one copy, so the
  delete message works.
- **A keyboard switched off is noticed.** Windows keeps a switched-off Bluetooth MIDI device
  listed, and sends to it seem to work, for about 30 seconds. Then it vanishes for a moment and
  returns, and the old connections stay dead. The port list alone therefore missed it. The app
  now listens to the input of the same name: a device that sends Active Sensing (the SX720 sends
  about 5 a second) and stops for a second counts as gone. The input is reopened each second until
  Active Sensing returns; then the output reopens. Switching the keyboard off and on with the app
  running made it drop the output and reconnect on its own. How quickly it noticed was not timed.
- **A badge on Play** shows while the drums go to a MIDI output.

Not checked:

- **USB.** Only Bluetooth MIDI was tried.
- **The SX720's drum kit numbers.** The XG numbers (bank MSB 127, the kit as the program) are
  used. Nobody has listened to each kit of the app's short list on the keyboard.
- **A style with no Intro or Ending:** what the keyboard does when those buttons are pressed.
- **Phone screens.** The section step's pickers are small for a finger.
- **iOS and macOS,** and the Send to keyboard failure messages on real hardware.

## Milestone 14: MIDI in

The keyboard chosen under "MIDI output" also becomes a way to write beats. With MIDI input on and
a drum kit selected on the keyboard, the user plays a pattern and plays along to the loop. Each hit lands on the
nearest step. Placing notes on a grid is hard for some beginners; playing a beat is easier.

**It must stay as simple as the rest of the app.** There is one button and no question, and no
Record button: with MIDI in on, playing a pattern records.

**No mapping is needed.** A keyboard with a drum kit selected sends the General MIDI key for each
drum: 36 is the kick, 38 the snare, 42 the closed hi-hat. The app's rows use the same keys.

**Desktop (Windows, macOS, Linux) and iOS only,** like MIDI out. Android waits for MIDI out
there; see "Android, later".

### Definition of done

- **MIDI input is off until the person turns it on.** A "MIDI in" button in the strip, after
  Play, turns it on and off. It shows while a MIDI output is connected, and looks pressed while
  on. It is a device setting, kept in the data folder as `midi.input`, so it survives a restart. While it is off, the app ignores the keyboard's keys. Turning it
  off during a take ends the take. Off is the default
  because people also play the keyboard over the drums: a melody's keys fall in the drum range,
  and the app would take them for drums.
- **The input is the MIDI output's own device.** The app listens to the input with the same name
  as the chosen output, which Send to keyboard and the switch-off check already open. With no
  output chosen, the app listens to nothing.
- **The keyboard plays its own sound.** The app neither plays the hit nor sends it back, so
  nothing sounds twice. The beat the app plays still goes to the keyboard as before.
- **One channel is taken.** The first channel that sends a note-on after the input opens is the
  one the app listens to, until the input opens again. A keyboard sends its right-hand part on its
  own channel, usually 1, and may send its style's drums on channel 10 at the same time; the rule
  keeps the style out. This needs checking on a real keyboard.
- **Keys outside the drum kit are ignored.** General MIDI drums are keys 35 to 81. Some kits use
  keys beyond that range (XG kits go from 13 to 87); the app has no sound to match them.
- **Playing a pattern records.** While MIDI input is on and the keyboard connected, each run of the
  pattern being edited, from Play to Stop, is a take. Playing the song records nothing, and
  neither does a key pressed while stopped: it only flashes its row. A small red dot on Play, by
  the MIDI badge, shows that playing records.
- **Each hit lands on the nearest step**, measured from when the user heard the beat, not from
  when the message arrived. A hit just before the loop's start lands on its first step. Swing
  counts: a hit is snapped to the swung grid, with groove import's snapping
  (`dm-groove`, `snap`).
- **Recording only adds notes.** A hit on a step already on keeps the louder of the two levels.
  Removing notes stays a tap on the grid. One take, from Play to Stop, undoes as one step.
- **Velocity sets the level.** A light hit makes a soft step, a middling one a normal step, and a
  hard one a loud step. The step levels button moves up to show the levels the take used, as a
  beat to start from does. The thresholds are set by ear on a real keyboard.
- **A key flashes its row,** recording or not: the row's name lights up for a moment, so the
  player sees which drum each key is.
- **A key with no row is offered, not added.** It shows as an inactive row under the others: faded,
  with the drum's name and picture and a plus sign. It flashes like the others. Clicking it adds
  the drum as a row at the bottom, as one undo step. The app never asks which row a key belongs
  to.
- **At most 3 inactive rows,** the drums most recently played that have no row. A fourth pushes
  out the oldest. They belong to this session, not to the project, and are not saved.
- **Hits recorded on an inactive row wait in it** as faint steps of the pattern they were played
  in. Adding the row keeps them, at the levels played. They are dropped when the row is pushed
  out, when another project opens or a new one starts, and on Undo or Redo, which work on the
  project alone. They do not play: only the keyboard sounded them.
- **A keyboard that disappears** stops recording, keeps what was recorded, and shows one line.
- **Checked on a real target:** the PSR-SX720 over Bluetooth on Windows, with a drum kit on its
  right-hand part. Record what was heard, how far off hits landed, and what was not checked.

### Recommended route

- **`dm-midi` opens the input.** It already does for Send to keyboard and the switch-off check
  (`sender.rs`). Those uses and this one share one connection, since Windows lets only one
  connection hold a port. The callback filters channel and key, then pushes each hit with its
  timestamp into a queue. The input callback never touches the synthesizer or the project.
- **The engine plays hits from a queue,** as it plays steps. The audio thread drains it at the
  start of each block, so a hit sounds within one block. The block size sets the delay the user
  feels; measure it on each platform.
- **The UI records.** It reads the hits the engine played, each with the playback position at
  which the user heard it, and turns them into step edits through the normal undo path. The engine
  knows that position from the audio stream's timestamps, which MIDI out already uses.
- **The input's timestamp is the hit's time.** midir gives each message a time in microseconds.
  Turning it into the engine's clock must not use the time the callback ran, which Bluetooth and
  WinMM can delay.

### Risks

- **Bluetooth timing.** Bluetooth LE MIDI adds 10 to 30 ms of delay that varies from hit to hit.
  At 120 BPM a step lasts 125 ms, so snapping should hide this.
- **The channel rule** may pick the wrong channel if the style plays before the first key. Test
  it on the SX720 before relying on it.

### Left out

- **A choice of input.** Drum pads or a kit that only send, with no output of the same name, are
  not reached. `dm-midi` can already open any input by name (`InputChoice`), so a setting can
  follow if someone asks.
- **The app sounding the hits.** The engine can play live hits at the row's volume, but with the
  input always the output's device the keyboard does that itself.
- **Step entry while stopped.** The grid has no cursor, so a hit has no obvious step to land on.
- **A count-in** before recording. The loop already gives the beat; add one if people ask.
- **Recording into the song**, across patterns.
- **Recording that replaces** what is in the pattern.
- **MIDI clock in,** following another device's tempo.

### What milestone 14 found

Checked on 2026-11-20 on Windows, by screenshot of the desktop app, with the Windows MIDI
Services loopback pair as the MIDI output. A throwaway program (not in the repository) played a
beat at the app's tempo into the pair's other port, on channel 1.

- **How it is built.** `dm-audio` takes live hits from a queue (`Engine::take_live_hits`), plays
  them at the row's volume when asked to, and reports where in the beat each was heard
  (`Engine::take_heard_hits`, `live.rs`). `dm-midi` reads the notes from the output's input, with
  the channel lock, the key range and the timestamp mapping in `input.rs`. `dm-ui` records in
  `record.rs`; `dm_groove::snap` is now public for it. A take is the undo group `Recording`.
- **The engine can also sound live hits,** and `dm-midi` can open any input by name
  (`InputChoice`). The app uses neither: its input is always the output's device, which sounds
  its own keys.
- **The first build had a Record button,** which showed beside Play once the loopback input
  connected and turned red while recording. It was replaced by the MIDI in button.
- **The take landed as played** (before inactive rows; see below). Loud kicks on every beat, normal snares on beats 2 and 4, soft
  closed hi-hats on every second step, all evenly spaced, with no hit doubled or moved by jitter.
  A cowbell, which had no row, got a Cowbell row at the bottom. A crash sent on channel 10 after
  the channel 1 notes was ignored, and so was a key outside the drum range sent first. The step
  levels button moved up to show loud and soft.
- **One Undo took the whole take back,** the new row and the step levels included, and left
  nothing more to undo.
- **The loopback's timestamps run about 30 ms behind its callback,** steadily; the mapping
  absorbs that.
- **The output and its input share a cut name.** WinMM cuts names to 31 characters, so both
  loopback ports are "Loopback Padrão de Aplicativo (". Choosing by name takes the first. The
  same was true of outputs before.

**The MIDI in button, flashes and inactive rows** were checked the same way, by screenshot:

- With MIDI in off, a beat played into the loopback changed nothing.
- With it on and the pattern stopped, Play showed its red dot, the kick and closed hi-hat names
  flashed in their colours, and the cowbell, which has no row, showed as a faded row with a plus
  sign. Nothing was recorded.
- With the pattern playing, the take landed as before. The cowbell's hit waited as a faint step
  in its faded row. Clicking the row added Cowbell with that note. Undo took back the add alone,
  then the take.
- The user played the PSR-SX720 into the first build and found it worked.

**What the PSR-SX720 sends when a voice changes** (2026-11-20, over its Bluetooth adapter). The
right-hand part sent its notes on channel 3. Switching it from StandardKit to ElectroKit sent, at
once: the XG message `F0 43 10 4C 08 02 07 01 F7` (part 3's mode is Drum), bank select 127 and 0,
program 70, then the part's volume, reverb and two more XG settings. Nothing is sent for the
voice already chosen, and General MIDI has no way to ask for it. So the app could tell drums
from a melody at each change, but only from the next change on. Program 70 is Yamaha's own
number, not XG's Electro kit (24), so matching the keyboard's kit to an app kit would need a
table of the SX720's kits. Neither is built.

Not checked:

- **The real keyboard, in detail.** The user found the first build worked on the PSR-SX720, but
  these were not looked at closely: whether the channel rule keeps its style's drums out, how late
  hits land over Bluetooth, the soft and loud thresholds by ear, and the MIDI in button, flashes
  and inactive rows with the keyboard.
- **The strip on a 375-point phone,** with the MIDI in button. The other strip buttons fit there.
- **Idle redraws.** While MIDI in is on, the app redraws 20 times a second when nothing plays, so
  a flash shows at once (`INPUT_POLL`). Its CPU cost was not measured.
- **The "gone" line,** shown when the keyboard disappears during a take.
- **Hits at the very start of the first loop.** A hit a fraction of a frame before play starts
  has no position and is dropped.
- **macOS, iOS and Linux.**

## Open questions

None at present. The three earlier ones were researched in
[`research/open-questions.md`](research/open-questions.md) and are now decided above:

- The mobile host is eframe, with SDL3 as the fallback (see "Interface").
- The bank is ColomboGMGS2, downloaded (see "How the bank is downloaded"). The research leaned to
  bundling GeneralUser GS; the decision follows karaokemachine's recommendation instead.
- The phone layout follows the research's proposal (see "Grid layout by screen").
