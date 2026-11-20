//! The section step of the keyboard style export: which pattern plays each section of the style.
//! Both Keyboard style and Send to keyboard open it from the Export dialog. See "Export as a
//! keyboard style" in `docs/DESIGN.md`.
//!
//! The choice starts from [`StyleSections::suggested`] and is kept while the dialog is open. It
//! is not saved in the project.

use dm_project::{KeyboardKind, Project, Section, StyleProblem, StyleSections};
use egui::{RichText, Vec2};

use crate::widgets::{self, TAP, TEXT_SIZE};

/// What the step's finishing button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Goal {
    /// Save a keyboard style file.
    File,
    /// Send the style to the keyboard over the MIDI output.
    Send,
}

/// The section step while its dialog is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleStep {
    pub(crate) goal: Goal,
    pub(crate) kind: KeyboardKind,
    pub(crate) sections: StyleSections,
}

impl StyleStep {
    /// The step for `goal`, with the first keyboard kind and the app's suggestion.
    pub(crate) fn new(goal: Goal, project: &Project) -> Self {
        Self {
            goal,
            kind: KeyboardKind::ALL[0],
            sections: StyleSections::suggested(project),
        }
    }

    /// Changes the keyboard kind, and empties the sections the new kind does not have.
    pub(crate) fn set_kind(&mut self, kind: KeyboardKind) {
        self.kind = kind;
        for section in Section::ALL {
            if !kind.has(section) {
                self.sections.set(section, None);
            }
        }
    }
}

/// The width of the section names, so the pickers line up.
const NAME_WIDTH: f32 = 96.0;

/// The rows the dialog shows for `kind`, in order: each main with its fill right under it.
pub(crate) fn rows(kind: KeyboardKind) -> Vec<Section> {
    Section::ALL.into_iter().filter(|&s| kind.has(s)).collect()
}

/// The patterns a section's picker offers, as engine slot and letter, in the project's order.
/// A section that must be one bar offers only one-bar patterns.
pub(crate) fn choices(
    project: &Project,
    kind: KeyboardKind,
    section: Section,
) -> Vec<(usize, char)> {
    let one_bar = kind.one_bar(section);
    project
        .patterns()
        .iter()
        .filter(|p| !one_bar || p.bars() == 1)
        .map(|p| (p.slot(), p.name()))
        .collect()
}

/// The letter A to D of a main and its fill.
fn variation(index: usize) -> char {
    dm_project::letter(index)
}

/// A section's name, as keyboards print it.
fn section_name(section: Section) -> String {
    match section {
        Section::Intro => tr!("keyboard-intro"),
        Section::Main(i) => tr!("keyboard-main", letter = variation(i).to_string()),
        Section::Fill(i) => tr!("keyboard-fill", letter = variation(i).to_string()),
        Section::Break => tr!("keyboard-break"),
        Section::Ending => tr!("keyboard-ending"),
    }
}

/// The brand of a keyboard kind, which is never translated.
pub(crate) fn brand(kind: KeyboardKind) -> &'static str {
    match kind {
        KeyboardKind::Yamaha => "Yamaha",
    }
}

/// The line under the Keyboard style button for a kind. Only these lines name a brand.
pub(crate) fn about(kind: KeyboardKind) -> String {
    match kind {
        KeyboardKind::Yamaha => tr!("menu-keyboard-style-about-yamaha"),
    }
}

/// Why a style cannot be made, in one plain line.
pub(crate) fn problem_text(problem: &StyleProblem) -> String {
    match problem {
        StyleProblem::NoMainA => tr!("keyboard-problem-no-main-a"),
        StyleProblem::NotOneBar => tr!("keyboard-problem-not-one-bar"),
        StyleProblem::NoSuchPattern => tr!("keyboard-problem-no-such-pattern"),
    }
}

/// The step's contents: a brand picker when there is more than one kind, a row per section,
/// the first problem, and the finishing button. Returns true when the finishing button was
/// tapped.
pub(crate) fn contents(ui: &mut egui::Ui, project: &Project, step: &mut StyleStep) -> bool {
    if KeyboardKind::ALL.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            for &kind in KeyboardKind::ALL {
                if ui
                    .add(crate::toggle(step.kind == kind, big(brand(kind))))
                    .clicked()
                {
                    step.set_kind(kind);
                }
            }
        });
    }
    ui.label(tr!("keyboard-sections-about"));
    for section in rows(step.kind) {
        let chosen = step.sections.get(section);
        let mut picked = chosen;
        ui.horizontal(|ui| {
            ui.add_sized(
                Vec2::new(NAME_WIDTH, TAP),
                egui::Label::new(big(&section_name(section))),
            );
            let shown = chosen.map_or_else(
                || tr!("keyboard-none"),
                |slot| project.slot_name(slot).to_string(),
            );
            egui::ComboBox::from_id_salt(("keyboard-section", format!("{section:?}")))
                .selected_text(big(&shown))
                .height(400.0)
                .show_ui(ui, |ui| {
                    let none = ui.add(
                        egui::Button::selectable(picked.is_none(), big(&tr!("keyboard-none")))
                            .min_size(Vec2::new(TAP * 2.0, TAP)),
                    );
                    if none.clicked() {
                        picked = None;
                    }
                    for (slot, letter) in choices(project, step.kind, section) {
                        let item = ui.add(
                            egui::Button::selectable(
                                picked == Some(slot),
                                big(&letter.to_string()),
                            )
                            .min_size(Vec2::new(TAP * 2.0, TAP)),
                        );
                        if item.clicked() {
                            picked = Some(slot);
                        }
                    }
                });
        });
        if picked != chosen {
            step.sections.set(section, picked);
        }
        // A fill left empty takes its main's last bar, which is worth saying.
        if let Section::Fill(i) = section
            && picked.is_none()
            && step.sections.get(Section::Main(i)).is_some()
        {
            ui.label(tr!(
                "keyboard-fill-from-main",
                letter = variation(i).to_string()
            ));
        }
    }
    ui.add_space(4.0);
    let problem = step.kind.problem(project, &step.sections);
    if let Some(problem) = &problem {
        ui.label(big(&problem_text(problem)).color(ui.visuals().warn_fg_color));
    }
    let finish = match step.goal {
        Goal::File => tr!("keyboard-save"),
        Goal::Send => tr!("keyboard-send"),
    };
    ui.add_enabled(problem.is_none(), widgets::button("", &finish))
        .clicked()
}

fn big(text: &str) -> RichText {
    RichText::new(text).size(TEXT_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project with patterns A (one bar), B (two bars) and C (one bar).
    fn project() -> Project {
        let mut project = Project::new();
        project.add_pattern().unwrap();
        project.add_pattern().unwrap();
        project.add_bar(1).unwrap();
        project
    }

    #[test]
    fn rows_list_each_main_with_its_fill_under_it() {
        let rows = rows(KeyboardKind::Yamaha);
        assert_eq!(rows.first(), Some(&Section::Intro));
        assert_eq!(rows.last(), Some(&Section::Ending));
        let main_b = rows.iter().position(|&s| s == Section::Main(1)).unwrap();
        assert_eq!(rows[main_b + 1], Section::Fill(1));
    }

    #[test]
    fn fills_and_the_break_offer_only_one_bar_patterns() {
        let project = project();
        let letters = |section| -> String {
            choices(&project, KeyboardKind::Yamaha, section)
                .into_iter()
                .map(|(_, letter)| letter)
                .collect()
        };
        assert_eq!(letters(Section::Main(0)), "ABC");
        assert_eq!(letters(Section::Intro), "ABC");
        assert_eq!(letters(Section::Fill(0)), "AC");
        assert_eq!(letters(Section::Break), "AC");
    }

    #[test]
    fn choices_carry_the_engine_slot() {
        let project = project();
        let slots: Vec<usize> = project.patterns().iter().map(|p| p.slot()).collect();
        let offered: Vec<usize> = choices(&project, KeyboardKind::Yamaha, Section::Ending)
            .into_iter()
            .map(|(slot, _)| slot)
            .collect();
        assert_eq!(offered, slots);
    }

    #[test]
    fn the_step_starts_from_the_suggestion() {
        let project = project();
        let step = StyleStep::new(Goal::File, &project);
        assert_eq!(step.sections, StyleSections::suggested(&project));
        assert_eq!(step.kind, KeyboardKind::ALL[0]);
        assert!(step.kind.problem(&project, &step.sections).is_none());
    }

    #[test]
    fn emptying_main_a_is_a_problem() {
        let project = project();
        let mut step = StyleStep::new(Goal::Send, &project);
        step.sections.set(Section::Main(0), None);
        assert_eq!(
            step.kind.problem(&project, &step.sections),
            Some(StyleProblem::NoMainA)
        );
    }

    #[test]
    fn changing_kind_keeps_what_the_kind_has() {
        let project = project();
        let mut step = StyleStep::new(Goal::File, &project);
        let before = step.sections.clone();
        step.set_kind(KeyboardKind::Yamaha);
        assert_eq!(step.sections, before);
    }
}
