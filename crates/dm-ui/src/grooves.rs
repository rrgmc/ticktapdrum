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
    /// Whether the groove list has any grooves, which the menu's Grooves item needs.
    pub(crate) any: bool,
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
/// its grooves under it. `focus` is a pack to open and show, once, such as the one just imported.
/// A delete sets `ask` instead of deleting.
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
    if view.packs.is_empty() {
        ui.label(big(&tr!("grooves-empty")));
        return;
    }
    ui.label(tr!("grooves-intro"));
    let only_one = view.packs.len() == 1;
    for (key, pack) in view.packs {
        ui.separator();
        let id = ui.make_persistent_id(("dm-groove-pack", key.as_str()));
        let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            id,
            only_one,
        );
        let focused = focus.as_deref() == Some(key.as_str());
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
            let delete_width = TAP + ui.spacing().item_spacing.x;
            let heading = tr!(
                "grooves-pack",
                name = pack.name.as_str(),
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
            if ui
                .add(widgets::button(icons::TRASH, ""))
                .on_hover_text(tr!("grooves-delete"))
                .clicked()
            {
                *ask = Some(Ask::DeletePack {
                    key: key.clone(),
                    name: pack.name.clone(),
                });
            }
        });
        state.store(ui.ctx());
        if state.is_open() {
            for (index, groove) in pack.grooves.iter().enumerate() {
                let playing = view.playing == Some((key.as_str(), index));
                groove_row(ui, key, index, groove, playing, ask, events);
            }
        }
    }
    *focus = None;
}

/// One groove: Play or Stop, its name with its folder and length below, then Add and Delete.
/// Everything fits on one line on a phone; a long name is cut short.
fn groove_row(
    ui: &mut egui::Ui,
    key: &str,
    index: usize,
    groove: &Groove,
    playing: bool,
    ask: &mut Option<Ask>,
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
            if ui
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
                .add(widgets::button(icons::PLUS, &tr!("grooves-add")))
                .on_hover_text(tr!("grooves-add-tip"))
                .clicked()
            {
                events.push(GrooveEvent::Add {
                    pack: key.to_owned(),
                    groove: index,
                });
            }
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.add(egui::Label::new(big(&groove.name)).truncate());
                let bars = tr!("grooves-bars", count = groove.bars.len());
                let detail = if groove.folder.is_empty() {
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
