//! The groove list: grooves imported from MIDI files and packs, grouped by the pack they came
//! from. Each can be heard, added to the project as a new pattern, or deleted. The dialog opens
//! from the ☰ menu (`menu.rs`); reading the files is `dm-groove`'s work, and the app's import
//! entry point is [`crate::DrumApp::import_grooves`]. See "Groove import" in `docs/DESIGN.md`.
//!
//! This module also puts into words what an import and an add did.

use dm_groove::{Groove, Import, ImportError, Pack, Refusal};
use dm_project::Imported;
use egui::{RichText, Vec2};

use crate::icons;
use crate::widgets::{self, TAP, TEXT_SIZE};

/// The key of the grooves that come with the app, in [`GrooveEvent`]s. No pack in the store has
/// it: keys there are file names.
pub(crate) const MAGENTA_KEY: &str = "builtin:magenta";
/// Where the grooves that come with the app are from.
const MAGENTA_PAGE: &str = "https://magenta.tensorflow.org/datasets/groove";
const MAGENTA_LICENSE: &str = "https://creativecommons.org/licenses/by/4.0/";

/// The grooves that come with the app: the 359 beats and fills of Google Magenta's Groove MIDI
/// Dataset (CC BY 4.0) that the app accepts, in 24 styles, gathered by `dm-groove`'s
/// `curate_magenta` example. Each groove's folder is its style. See
/// `assets/MAGENTA-CREDITS.txt`.
pub(crate) fn magenta_pack() -> Pack {
    dm_groove::pack_from_json(include_str!("../assets/magenta-grooves.json"))
        .expect("the bundled grooves are valid")
}

/// The largest file the import reads, in bytes. `dm-groove` allows 100 MB unpacked in a zip, so
/// a larger file cannot be a pack it accepts.
pub(crate) const MAX_FILE: usize = 100 * 1024 * 1024;

/// What the person did in the groove list. Packs are named by their key in the store; grooves
/// by their place in their pack.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GrooveEvent {
    /// Choose a MIDI file or a zip to import.
    AddFromFile,
    /// Download a MIDI file or a zip from this link, cleaned up by `dm_groove::link_address`,
    /// and import it.
    Download(String),
    /// Stop the download from a link.
    StopDownload,
    /// The groove list has just opened: read it again.
    Open,
    /// Loop a groove on its own, stopping any other.
    Play { pack: String, groove: usize },
    /// Stop the groove playing.
    Stop,
    /// Add a groove to the project as a new pattern.
    Add { pack: String, groove: usize },
    /// Delete a pack and its grooves. Asked first.
    DeletePack(String),
    /// Delete one groove. Asked first.
    DeleteGroove { pack: String, groove: usize },
}

/// What the groove items and the groove list need to know to draw themselves.
pub(crate) struct GroovesView<'a> {
    /// The grooves that come with the app.
    pub(crate) magenta: &'a Pack,
    /// The packs read when the list opened, each with its key.
    pub(crate) packs: &'a [(String, Pack)],
    /// The groove playing on its own: its pack's key and its place in the pack.
    pub(crate) playing: Option<(&'a str, usize)>,
    /// A line at the top of the list: what the last import did, or why an add failed.
    pub(crate) note: Option<&'a str>,
}

/// A question the list asks before deleting.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Ask {
    DeletePack {
        key: String,
        name: String,
    },
    DeleteGroove {
        key: String,
        index: usize,
        name: String,
    },
}

impl Ask {
    /// The question, and the answers for yes and no.
    pub(crate) fn words(&self) -> (String, String, String) {
        let question = match self {
            Ask::DeletePack { name, .. } => {
                tr!("grooves-delete-pack-question", name = name.as_str())
            }
            Ask::DeleteGroove { name, .. } => {
                tr!("grooves-delete-groove-question", name = name.as_str())
            }
        };
        (question, tr!("grooves-delete"), tr!("grooves-keep"))
    }

    /// What a yes does.
    pub(crate) fn event(self) -> GrooveEvent {
        match self {
            Ask::DeletePack { key, .. } => GrooveEvent::DeletePack(key),
            Ask::DeleteGroove { key, index, .. } => GrooveEvent::DeleteGroove {
                pack: key,
                groove: index,
            },
        }
    }
}

fn big(text: &str) -> RichText {
    RichText::new(text).size(TEXT_SIZE)
}

/// The groove list's contents: the note, then each pack as a heading that opens and closes, with
/// its grooves under it. The pack that comes with the app is always first and cannot be deleted;
/// Add puts one of its grooves in as a new pattern, like any other. `focus` is a pack to open
/// and show, once, such as the one just imported. A delete sets `ask` instead of deleting.
pub(crate) fn list(
    ui: &mut egui::Ui,
    view: &GroovesView,
    focus: &mut Option<String>,
    ask: &mut Option<Ask>,
    events: &mut Vec<GrooveEvent>,
) {
    if let Some(note) = view.note {
        ui.label(big(note).color(ui.visuals().warn_fg_color));
    }
    ui.label(tr!("grooves-intro"));
    let magenta = std::iter::once((MAGENTA_KEY, view.magenta, false));
    let imported = view
        .packs
        .iter()
        .map(|(key, pack)| (key.as_str(), pack, true));
    for (key, pack, deletable) in magenta.chain(imported) {
        ui.separator();
        let id = ui.make_persistent_id(("dm-groove-pack", key));
        let mut state =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
        let focused = focus.as_deref() == Some(key);
        if focused {
            state.set_open(true);
        }
        // The heading: the pack's name and how many grooves it has, which opens and closes it,
        // then Delete for the whole pack.
        ui.horizontal(|ui| {
            let arrow = if state.is_open() {
                icons::CARET_DOWN
            } else {
                icons::CARET_RIGHT
            };
            let delete_width = if deletable {
                TAP + ui.spacing().item_spacing.x
            } else {
                0.0
            };
            let name = if deletable {
                pack.name.clone()
            } else {
                tr!("grooves-magenta-title")
            };
            let heading = tr!(
                "grooves-pack",
                name = name.as_str(),
                count = pack.grooves.len()
            );
            let width = (ui.available_width() - delete_width).max(TAP);
            let response = ui
                .allocate_ui(Vec2::new(width, TAP), |ui| {
                    ui.add(
                        egui::Button::new(widgets::label(arrow, &heading))
                            .truncate()
                            .min_size(Vec2::new(TAP, TAP)),
                    )
                })
                .inner;
            if response.clicked() {
                state.toggle(ui);
            }
            if focused {
                response.scroll_to_me(Some(egui::Align::TOP));
            }
            if deletable
                && ui
                    .add(widgets::button(icons::TRASH, ""))
                    .on_hover_text(tr!("grooves-delete"))
                    .clicked()
            {
                *ask = Some(Ask::DeletePack {
                    key: key.to_owned(),
                    name: pack.name.clone(),
                });
            }
        });
        state.store(ui.ctx());
        if state.is_open() && !deletable {
            ui.label(RichText::new(tr!("grooves-magenta-credits")).small());
            let add = (tr!("grooves-add"), tr!("grooves-add-tip"));
            magenta_styles(ui, view, (&add.0, &add.1), events);
        } else if state.is_open() {
            let names = short_names(&pack.grooves);
            for (index, groove) in pack.grooves.iter().enumerate() {
                let playing = view.playing == Some((key, index));
                groove_row(
                    ui,
                    key,
                    index,
                    groove,
                    &names[index],
                    playing,
                    deletable.then_some(&mut *ask),
                    (&tr!("grooves-add"), &tr!("grooves-add-tip")),
                    events,
                );
            }
        }
    }
    *focus = None;
}

/// The grooves that come with the app, from the Ready-made styles dialog: what they are and
/// where they are from, with links to the dataset and its licence, then each groove with Play
/// and Load. Load sends [`GrooveEvent::Add`], which the menu turns into starting a new project
/// from the groove, as a ready-made style does. They cannot be deleted.
pub(crate) fn magenta_list(ui: &mut egui::Ui, view: &GroovesView, events: &mut Vec<GrooveEvent>) {
    if let Some(note) = view.note {
        ui.label(big(note).color(ui.visuals().warn_fg_color));
    }
    ui.label(tr!("grooves-magenta-intro"));
    ui.label(RichText::new(tr!("grooves-magenta-credits")).small());
    ui.horizontal_wrapped(|ui| {
        ui.hyperlink_to(tr!("grooves-magenta-page"), MAGENTA_PAGE)
            .on_hover_text(MAGENTA_PAGE);
        ui.hyperlink_to(tr!("grooves-magenta-license"), MAGENTA_LICENSE)
            .on_hover_text(MAGENTA_LICENSE);
    });
    ui.separator();
    let load = (tr!("menu-start"), tr!("grooves-magenta-load-tip"));
    magenta_styles(ui, view, (&load.0, &load.1), events);
}

/// The grooves that come with the app, by style: each style a heading with its count that opens
/// and closes, with its grooves under it. Their folder is their style, and the pack keeps a
/// style's grooves together, in the order of their names. `add` is the Add button's word and hover text.
fn magenta_styles(
    ui: &mut egui::Ui,
    view: &GroovesView,
    add: (&str, &str),
    events: &mut Vec<GrooveEvent>,
) {
    let grooves = &view.magenta.grooves;
    let mut start = 0;
    while start < grooves.len() {
        let style = grooves[start].folder.as_str();
        let end = grooves[start..]
            .iter()
            .position(|g| g.folder != style)
            .map_or(grooves.len(), |n| start + n);
        let id = ui.make_persistent_id(("dm-magenta-style", style));
        let mut state =
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
        let arrow = if state.is_open() {
            icons::CARET_DOWN
        } else {
            icons::CARET_RIGHT
        };
        let heading = tr!("grooves-pack", name = style, count = end - start);
        if ui
            .add(
                egui::Button::new(widgets::label(arrow, &heading))
                    .frame(false)
                    .min_size(Vec2::new(TAP, TAP)),
            )
            .clicked()
        {
            state.toggle(ui);
        }
        state.store(ui.ctx());
        if state.is_open() {
            for (index, groove) in grooves.iter().enumerate().take(end).skip(start) {
                let playing = view.playing == Some((MAGENTA_KEY, index));
                groove_row(
                    ui,
                    MAGENTA_KEY,
                    index,
                    groove,
                    &groove.name,
                    playing,
                    None,
                    add,
                    events,
                );
            }
        }
        start = end;
    }
}

/// The grooves' names without the words they all start with, so a phone shows what tells them
/// apart: "076 Chilled Beat 1 Clap A" and "076 Chilled Beat 1 Fill 2" become "Clap A" and
/// "Fill 2". A name that would be left empty keeps its full form.
pub(crate) fn short_names(grooves: &[Groove]) -> Vec<String> {
    let words: Vec<Vec<&str>> = grooves
        .iter()
        .map(|g| g.name.split_whitespace().collect())
        .collect();
    let shared = if words.len() < 2 {
        0
    } else {
        let first = &words[0];
        (0..first.len())
            .take_while(|&i| words.iter().all(|w| w.get(i) == Some(&first[i])))
            .count()
    };
    grooves
        .iter()
        .zip(&words)
        .map(|(groove, words)| {
            if shared < words.len() {
                words[shared..].join(" ")
            } else {
                groove.name.clone()
            }
        })
        .collect()
}

/// One groove: Play or Stop, its short name with its folder and length below, then Add and
/// Delete. Everything fits on one line on a phone; a long name is cut short, and the full name
/// shows on hover. Without `ask` there is no Delete, as for the grooves that come with the app.
/// `add` is the Add button's word and its hover text.
#[allow(clippy::too_many_arguments)]
fn groove_row(
    ui: &mut egui::Ui,
    key: &str,
    index: usize,
    groove: &Groove,
    name: &str,
    playing: bool,
    ask: Option<&mut Option<Ask>>,
    add: (&str, &str),
    events: &mut Vec<GrooveEvent>,
) {
    ui.horizontal(|ui| {
        let (icon, tip) = if playing {
            (icons::STOP, tr!("grooves-stop"))
        } else {
            (icons::PLAY, tr!("grooves-play"))
        };
        if ui
            .add(widgets::toggle_button(playing, icon, ""))
            .on_hover_text(tip)
            .clicked()
        {
            events.push(if playing {
                GrooveEvent::Stop
            } else {
                GrooveEvent::Play {
                    pack: key.to_owned(),
                    groove: index,
                }
            });
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(ask) = ask
                && ui
                    .add(widgets::button(icons::TRASH, ""))
                    .on_hover_text(tr!("grooves-delete"))
                    .clicked()
            {
                *ask = Some(Ask::DeleteGroove {
                    key: key.to_owned(),
                    index,
                    name: groove.name.clone(),
                });
            }
            if ui
                .add(widgets::button(icons::PLUS, add.0))
                .on_hover_text(add.1)
                .clicked()
            {
                events.push(GrooveEvent::Add {
                    pack: key.to_owned(),
                    groove: index,
                });
            }
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(big(name)).truncate())
                    .on_hover_text(&groove.name);
                let bars = tr!("grooves-bars", count = groove.bars.len());
                // A groove that comes with the app is listed under its style, its folder, so
                // its tempo says more.
                let detail = if key == MAGENTA_KEY {
                    format!("{} BPM · {bars}", groove.bpm)
                } else if groove.folder.is_empty() {
                    bars
                } else {
                    format!("{} · {bars}", groove.folder)
                };
                ui.add(egui::Label::new(RichText::new(detail).small().weak()).truncate());
            });
        });
    });
}

/// What the preview plays for a groove: its hits, counted in steps from its start.
pub(crate) fn preview_hits(groove: &Groove) -> (Vec<dm_audio::PreviewHit>, usize) {
    let steps = groove.bars.len() * dm_project::STEPS_PER_BAR;
    let hits = groove
        .bars
        .iter()
        .enumerate()
        .flat_map(|(bar, hits)| {
            hits.iter().map(move |hit| dm_audio::PreviewHit {
                step: bar * dm_project::STEPS_PER_BAR + hit.step,
                key: hit.key,
                velocity: hit.velocity,
            })
        })
        .collect();
    (hits, steps)
}

/// A refusal in a beginner's words, short enough to list several.
fn refusal(refusal: Refusal) -> String {
    match refusal {
        Refusal::NotMidi => tr!("grooves-refusal-not-midi"),
        Refusal::NoNotes => tr!("grooves-refusal-no-notes"),
        Refusal::SeveralChannels => tr!("grooves-refusal-several-channels"),
        Refusal::NotFourFour => tr!("grooves-refusal-not-four-four"),
        Refusal::TempoChanges => tr!("grooves-refusal-tempo-changes"),
        Refusal::TooLong => tr!("grooves-refusal-too-long"),
        Refusal::OffGrid => tr!("grooves-refusal-off-grid"),
        Refusal::OtherDrumProgram => tr!("grooves-refusal-other-drum-program"),
    }
}

/// The refusals counted by reason, most common first: "not in 4/4 (3); no notes (1)". Reasons
/// with the same count keep the order they first appeared in.
fn reasons(refused: &[(String, Refusal)]) -> String {
    let mut counts: Vec<(Refusal, usize)> = Vec::new();
    for (_, why) in refused {
        match counts.iter_mut().find(|(r, _)| r == why) {
            Some((_, count)) => *count += 1,
            None => counts.push((*why, 1)),
        }
    }
    counts.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    counts
        .into_iter()
        .map(|(why, count)| tr!("grooves-reason-count", reason = refusal(why), count = count))
        .collect::<Vec<_>>()
        .join("; ")
}

/// What an import that added grooves did: how many came from `name`, then what was left out.
pub(crate) fn added(import: &Import, name: &str) -> String {
    let mut message = tr!("grooves-added", count = import.grooves.len(), name = name);
    if !import.refused.is_empty() {
        message.push(' ');
        message.push_str(&tr!(
            "grooves-left-out",
            refused = import.refused.len(),
            reasons = reasons(&import.refused)
        ));
    }
    if import.other_mappings_skipped > 0 {
        message.push(' ');
        message.push_str(&tr!("grooves-only-general-midi"));
    }
    message
}

/// Why an import added nothing. A single file refused names its one reason.
pub(crate) fn nothing_added(import: &Import, name: &str) -> String {
    match import.refused.as_slice() {
        [] => tr!("grooves-none-found", name = name),
        [(_, why)] => tr!("grooves-not-added", name = name, reason = refusal(*why)),
        refused => tr!(
            "grooves-none-added",
            name = name,
            refused = refused.len(),
            reasons = reasons(refused)
        ),
    }
}

/// Why a file could not be imported at all.
pub(crate) fn import_error(error: &ImportError, name: &str) -> String {
    match error {
        ImportError::NotGrooves => tr!("grooves-error-not-grooves", name = name),
        ImportError::TooLarge => tr!("grooves-error-too-large", name = name),
        ImportError::BrokenZip => tr!("grooves-error-broken-zip", name = name),
        ImportError::NoMidiFiles => tr!("grooves-error-no-midi-files", name = name),
    }
}

/// What adding a groove as a pattern did, then a sentence for each change to what was given.
pub(crate) fn added_pattern(added: &Imported, groove: &Groove, pattern: char) -> String {
    let mut message = tr!(
        "grooves-added-pattern",
        pattern = pattern.to_string(),
        name = groove.name.as_str(),
        bars = added.bars
    );
    let mut notes = Vec::new();
    // Bars that only repeated a shorter groove became that groove.
    if added.bars_given > added.bars + added.bars_left_out {
        notes.push(tr!(
            "grooves-repeats",
            given = added.bars_given,
            bars = added.bars + added.bars_left_out
        ));
    }
    if added.bars_left_out > 0 {
        notes.push(tr!("grooves-bars-left-out", count = added.bars_left_out));
    }
    if added.rows_added > 0 {
        notes.push(tr!("grooves-rows-added", count = added.rows_added));
    }
    if added.drums_left_out > 0 {
        notes.push(tr!("grooves-drums-left-out", count = added.drums_left_out));
    }
    for note in notes {
        message.push(' ');
        message.push_str(&note);
    }
    message
}

/// Why a groove was not added to the project.
pub(crate) fn add_refused(refused: dm_project::Refused) -> String {
    match refused {
        dm_project::Refused::TooManyPatterns => tr!("grooves-no-room"),
        _ => tr!("grooves-add-failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grooves_that_come_with_the_app_load() {
        let pack = magenta_pack();
        assert_eq!(pack.grooves.len(), 359);
        for groove in &pack.grooves {
            assert!((1..=16).contains(&groove.bars.len()), "{}", groove.name);
            assert!(!groove.folder.is_empty(), "{}", groove.name);
            assert!(
                groove.bars.iter().any(|bar| !bar.is_empty()),
                "{}",
                groove.name
            );
        }
    }

    #[test]
    fn short_names_drop_the_words_all_grooves_share() {
        let groove = |name: &str| Groove {
            name: name.into(),
            folder: String::new(),
            bpm: 100.0,
            bars: vec![Vec::new()],
        };
        let pack = [
            groove("076 Chilled Beat 1 Clap A"),
            groove("076 Chilled Beat 1 Fill 2"),
            groove("076 Chilled Beat 1"),
        ];
        assert_eq!(
            short_names(&pack),
            vec!["Clap A", "Fill 2", "076 Chilled Beat 1"]
        );
        assert_eq!(short_names(&pack[..1]), vec!["076 Chilled Beat 1 Clap A"]);
    }

    fn groove(bars: usize) -> Groove {
        Groove {
            name: "Beat".into(),
            folder: String::new(),
            bpm: 90.0,
            bars: (0..bars)
                .map(|bar| {
                    vec![dm_groove::BarHit {
                        key: 36,
                        step: bar,
                        velocity: 100,
                    }]
                })
                .collect(),
        }
    }

    #[test]
    fn a_preview_counts_steps_from_the_start() {
        let (hits, steps) = preview_hits(&groove(2));
        assert_eq!(steps, 32);
        let at: Vec<usize> = hits.iter().map(|h| h.step).collect();
        assert_eq!(at, vec![0, 17]);
    }

    // Other tests switch the language while these run, so these check only what every
    // language's text contains: the numbers and names filled in.

    #[test]
    fn refusals_are_counted_most_common_first() {
        let refused = vec![
            ("a.mid".into(), Refusal::NoNotes),
            ("b.mid".into(), Refusal::NotFourFour),
            ("c.mid".into(), Refusal::NotFourFour),
        ];
        let text = reasons(&refused);
        let (first, second) = text.split_once("; ").unwrap();
        assert!(first.contains("(2)") && second.contains("(1)"), "{text}");
    }

    #[test]
    fn an_import_says_what_was_added_and_left_out() {
        let import = Import {
            grooves: vec![groove(1), groove(2)],
            refused: vec![("x.mid".into(), Refusal::OffGrid)],
            other_mappings_skipped: 4,
        };
        let text = added(&import, "Pack.zip");
        assert!(text.contains('2') && text.contains("Pack.zip"), "{text}");
        assert!(text.contains("(1)"), "{text}");
        assert!(!text.contains("grooves-"), "{text}");

        let one = Import {
            refused: vec![("Song.mid".into(), Refusal::SeveralChannels)],
            ..Import::default()
        };
        let text = nothing_added(&one, "Song.mid");
        assert!(text.contains("Song.mid") && !text.contains("(1)"), "{text}");
        let none = nothing_added(&Import::default(), "Empty.zip");
        assert!(none.contains("Empty.zip"), "{none}");
    }

    #[test]
    fn an_added_pattern_mentions_only_what_changed() {
        let plain = Imported {
            index: 2,
            bars: 2,
            bars_given: 2,
            bars_left_out: 0,
            rows_added: 0,
            drums_left_out: 0,
        };
        let short = added_pattern(&plain, &groove(2), 'C');
        assert!(short.contains('C') && short.contains("Beat"), "{short}");
        let busy = Imported {
            bars_given: 8,
            rows_added: 3,
            ..plain
        };
        let long = added_pattern(&busy, &groove(8), 'C');
        assert!(long.contains('8') && long.contains('3'), "{long}");
        assert!(long.len() > short.len(), "{long}");
    }
}
