# Research: the design's open questions

Findings from 2026-11-19 on the three open questions in [`DESIGN.md`](../DESIGN.md). These are
findings, not decisions. A finding becomes binding only when `DESIGN.md` records it.

Claims marked *unverified* could not be confirmed from a primary source.

## 1. Which host runs egui on mobile

Versions current at the time: egui and eframe 0.37.0, `sdl3` crate 0.22.1.

### eframe on Android

- eframe lists Android as a supported native target. Support arrived in 0.30.
  ([eframe docs](https://docs.rs/crate/eframe/latest),
  [changelog](https://raw.githubusercontent.com/emilk/egui/main/crates/eframe/CHANGELOG.md))
- The official example `examples/hello_android` builds with cargo-apk. Its README asks for a
  patched fork of cargo-apk. ([example](https://github.com/emilk/egui/tree/main/examples/hello_android))
- Other build routes exist: xbuild ([egui-android-demo](https://github.com/fredrik-hammar/egui-android-demo))
  and the egui template in [cargo-mobile2](https://lib.rs/crates/cargo-mobile2).
- The wgpu backends Android needs are not enabled by default and must be turned on by hand.
- **Soft keyboard:** weak. winit has no Android IME yet; the draft PR is described by testers as
  unreliable. ([winit#4451](https://github.com/rust-windowing/winit/pull/4451)) A drum grid needs
  little typing, so this matters less here.
- **Safe area:** egui-winit applies insets on iOS only, so Android notches and bars are not handled.
  ([source](https://docs.rs/crate/egui-winit/0.34.2/source/src/safe_area.rs))
- **Lifecycle:** early versions crashed on resume. Later releases save state on suspend, recover a
  lost wgpu surface and support the back button.
- One real app, the Grim wallet, ships on Android with it and calls its support "limited".
  ([Grim](https://code.gri.mw/GUI/grim))

### eframe on iOS

- No official example or template.
- Code for iOS exists: build fixes, safe area insets, and dynamic island placement.
- Packaging goes through cargo-mobile2's Xcode project, or a Rust static library in a hand-made
  Xcode project. Both need macOS.
- A report of redraws stopping on iOS under winit is *unverified* as still open.

### egui hosted by SDL3

- [egui-sdl3](https://github.com/abus-sh/egui-sdl3) 0.4.0: renders with glow, wgpu or the SDL
  canvas. It tracks egui 0.34 and `sdl3` 0.18, three egui versions behind. It has very few users
  and says nothing about Android, iOS or touch.
- [egui-sdl3-platform](https://crates.io/crates/egui-sdl3-platform) 0.1.1: input only, older egui
  and `sdl3`.
- No crate renders egui through SDL_GPU. Neither crate shows evidence of use on mobile.
- Wiring it by hand is feasible: the `sdl3` crate provides the window handles wgpu needs on Android
  and iOS. The work is translating SDL events (finger events included) to egui input, driving the
  egui wgpu renderer, recreating the surface on background and foreground, and reading safe area
  and DPI. Estimate: 3 to 6 days, or about 1 day starting from a fork of egui-sdl3 (*unverified*).

### Touch in egui

- One finger acts as the pointer. Two or more fingers produce gestures such as pinch zoom.
- A touch counts as a click only if it moves under 6 points and lasts under 0.6 seconds. Both
  limits are fixed in egui.
- On touch screens, dragging inside a scroll area scrolls it. Grid cells inside a scroll area can
  therefore turn taps into scrolls. Keep the grid outside a scroll area, and react on press, not
  on click, which also lowers latency.
- egui's default control size (40 by 20 points) is too small for fingers. The app must size its
  own cells.

### Finding

Start the mobile spike with **eframe** (wgpu, GameActivity), from `hello_android`. It is the
shortest path and is maintained upstream. Keep all UI in one host-independent function so the host
can change. If eframe stalls on Android, fall back to SDL3, whose mobile packaging karaokemachine
already proves, with a fork of egui-sdl3.

Risks: a fragile Android toolchain, suspend and resume bugs (including losing the audio stream),
no Android safe area, and iOS unproven for eframe.

## 2. Which SoundFont to bundle

The app needs a SoundFont whose licence allows shipping it inside an app, possibly a commercial one.

| SoundFont | Size | Licence | Drum kits | Verdict |
|---|---|---|---|---|
| [GeneralUser GS](https://github.com/mrbumpy409/GeneralUser-GS) 2.0.3 | about 32 MB | Custom; allows use without restriction, including commercial software | Standard 1-3, Room, Power, Electronic, 808/909, Dance, Jazz, Brush, Orchestral, SFX | **First choice** |
| FluidR3_GM | about 141 MB (*unverified*) | MIT | Standard set of GS kits (*unverified*) | Too large whole; drums could be extracted |
| MuseScore_General 0.2 | 206 MB (.sf2), 38 MB (.sf3) | MIT, notices must be kept | GS kits plus extras (*unverified*) | Too large as .sf2 |
| TimGM6mb | about 6 MB | GPL-2 (per Debian) | *unverified* | GPL is a risk for a closed app |
| Arachno | 136 MB | None found; site forbids reproduction | 9 kits | Not usable without permission |
| SGM-V2.01 | several hundred MB | None found; samples of doubtful origin | | Reject |
| MuldjordKit (FreePats) | 53 MB | CC BY 4.0 | One acoustic kit, GM mapping unstated | Too limited |

- GeneralUser GS's author states he cannot be fully sure where every sample came from. The licence
  itself notes this may matter for commercial software.
  ([licence](https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/main/documentation/LICENSE.txt))
- karaokemachine already bundles GeneralUser GS, so it is known to work with the rustysynth fork.
- Coverage of the GM drum keys 35 to 81 in GeneralUser GS is *unverified*; its percussion map PDF
  was not read.
- No reliable comparisons of drum sound quality were found for any candidate.
- Whether rustysynth reads the compressed .sf3 format is *unverified* (believed not).

### How rustysynth finds drum kits

From the [rustysynth](https://github.com/sinshu/rustysynth) source:

- MIDI channel 10 (index 9) is the percussion channel and uses bank 128.
- A program change on that channel selects the kit with that program number in bank 128.
- A missing kit falls back to bank 128, program 0 (Standard).

So a drum-only SoundFont must place its kits in bank 128, at the GM program numbers.

### Finding

1. **GeneralUser GS**: every kit the design needs, a licence that allows software use, and
   already proven in karaokemachine. The cost is 32 MB on mobile and the stated sample-origin
   uncertainty.
2. **A drum-only SoundFont extracted from FluidR3_GM or MuseScore_General** (MIT): the clearest
   licence and a much smaller file, at the cost of building it and shipping the notices.

## 3. How the grid fits on a phone

### Touch target sizes

- Android (Material): at least 48 by 48 dp, with 8 dp between targets.
  ([Google](https://support.google.com/accessibility/android/answer/7101858?hl=en))
- iOS: 44 by 44 pt is the widely cited minimum.
  ([Deque](https://dequeuniversity.com/rules/attest-ios/1.0/touch-target-size))
- WCAG 2.2: 24 by 24 px at level AA, 44 px at level AAA.
  ([DAISY](https://kb.daisy.org/publishing/docs/wcag/target-size-minimum.html))
- Research on one-thumb use recommends about 9.6 mm for repeated taps, close to 48 dp.
  ([Parhi et al. 2006](https://dx.doi.org/10.1145/1152215.1152260))

### What fits

With a label column of 64 to 72 pt and 2 pt gaps:

- **Portrait** (360 to 430 wide): 6 to 8 columns at 44 to 48 pt. Sixteen columns would mean cells
  of 18 to 22 pt, below every guideline.
- **Landscape** (740 to 930 wide): 13 to 18 columns, so 16 steps fit on most phones.
- **Landscape height** is the tighter limit: 10 rows get only 26 to 33 pt each after the transport
  bar. Rows must scroll vertically or cells must be shorter than they are wide.

### What other apps do

- **GarageBand Beat Sequencer**: 16 steps, one row per drum. Longer patterns show an overview of
  sections above the grid; tapping one edits it. Dragging across the grid paints steps.
  ([Apple](https://support.apple.com/guide/garageband-iphone/chs2fd3ab74e/ios))
- **Logic Pro for iPad step sequencer**: longer patterns split into pages. Scrolling sits in a
  separate strip because swipes on the grid edit steps.
  ([Apple](https://support.apple.com/guide/logicpro-ipad/lpip18ed3ba1/ipados))
- **FL Studio Mobile**: tap to toggle; a long press on a row offers fills such as every 2 or 4
  steps. ([manual](https://www.image-line.com/fl-studio-learning/fl-studio-mobile-online-manual/html/plugins/FL%20Studio%20Mobile_Editors.htm))
- Every documented app keeps a fixed label column on the left. None rotates the grid so steps run
  vertically.
- Details for Korg Gadget, DM1, BandLab, Medly and others are *unverified*.

### Interaction details

- Tap toggles a step. Dragging paints: the first cell decides whether the drag turns cells on or off.
- One gesture per area: one finger on the grid always paints. Scrolling happens elsewhere, on a
  separate strip or with two fingers.
- Shading alternate groups of 4 steps is the common way to show beats (*unverified* as a formal
  guideline). A highlighted column shows the playhead.

### Finding

- **Phone portrait:** show half a bar, 8 steps at 36 to 44 pt. Cells may be taller than wide. A
  two-part switch above the grid picks the half and lights the one the playhead is in.
- **Phone landscape:** show all 16 steps at 40 to 50 pt; rows scroll vertically if 10 do not fit.
  The label column stays fixed.
- **Tablet and desktop:** 16 steps per bar, with 2 to 4 bars side by side when cells stay at 44 pt
  or more.
- Do not force an orientation, and do not rotate the grid.
- **Bars:** a strip above the grid shows each bar as a small thumbnail of its hits. Tapping one edits
  it. "Follow playhead", on by default while playing, turns the page automatically. The strip also
  holds "+ bar" and "copy bar".
- **Gestures:** one finger toggles or paints on the grid. Scrolling and swiping happen only on the bar
  strip.
- **Visuals:** shade alternate groups of 4 steps, number beats 1 to 4, highlight the playhead
  column, and give rows icons and short names.
- **Beginner aid:** a long press on a row offers "every beat", "every 2 steps" and "clear".
