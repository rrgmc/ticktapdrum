//! Keyboard style export: the project as the drum part of an arranger keyboard's style.
//!
//! An arranger keyboard plays a style: a backing with several sections (intro, main variations,
//! fills, a break, an ending) that the player switches between. This module holds what every
//! kind of keyboard shares: the sections, which pattern plays each, the app's suggestion and the
//! checks. Each [`KeyboardKind`] adds its own rules and its own file writer. Only Yamaha exists
//! now; `docs/research/arranger-style-export.md` says what Korg would need.

use crate::{Pattern, Project, STEPS_PER_BAR};

#[path = "style_yamaha.rs"]
mod yamaha;

/// A kind of arranger keyboard the app can write styles for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardKind {
    /// Yamaha's PSR-SX, PSR-S, Genos and Tyros keyboards: a `.sty` file.
    Yamaha,
}

/// One section of a style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Intro,
    /// Main A to D, as 0 to 3.
    Main(usize),
    /// The fill that leads out of Main A to D, as 0 to 3.
    Fill(usize),
    Break,
    Ending,
}

impl Section {
    /// Every section, in the order a style file holds them and a list shows them.
    pub const ALL: [Section; 11] = [
        Section::Intro,
        Section::Main(0),
        Section::Fill(0),
        Section::Main(1),
        Section::Fill(1),
        Section::Main(2),
        Section::Fill(2),
        Section::Main(3),
        Section::Fill(3),
        Section::Break,
        Section::Ending,
    ];
}

/// Which pattern plays each section of the style, by engine slot ([`Pattern::slot`]). `None`
/// leaves the section out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleSections {
    pub intro: Option<usize>,
    /// Main A to D. Main A is required.
    pub mains: [Option<usize>; 4],
    /// The fill for each main. A main without a fill uses its own last bar as its fill.
    pub fills: [Option<usize>; 4],
    pub fill_break: Option<usize>,
    pub ending: Option<usize>,
}

/// Why a style cannot be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleProblem {
    /// Main A has no pattern. Some keyboards refuse a style without it.
    NoMainA,
    /// A section that must be one bar long, such as a fill, uses a longer pattern.
    NotOneBar,
    /// A section names a slot that holds no pattern.
    NoSuchPattern,
}

impl StyleSections {
    /// The app's suggestion, from the song.
    ///
    /// The song's patterns are taken in order, each once; without a song, the project's patterns
    /// in their own order. A one-bar pattern that comes right after a main which has no fill yet
    /// becomes that main's fill. Any other pattern becomes the next free main, up to Main D; the
    /// rest are left out. The Intro, the Break and the Ending start empty.
    pub fn suggested(project: &Project) -> Self {
        let order: Vec<usize> = if project.song.is_empty() {
            project.patterns.iter().map(|p| p.slot).collect()
        } else {
            project.song.clone()
        };
        let mut sections = Self::default();
        let mut seen = Vec::new();
        let mut previous = None;
        for slot in order {
            let Some(pattern) = find(project, slot) else {
                continue;
            };
            if !seen.contains(&slot) {
                seen.push(slot);
                let after_main = previous
                    .and_then(|before| sections.mains.iter().position(|&m| m == Some(before)));
                match after_main {
                    Some(main) if pattern.bars() == 1 && sections.fills[main].is_none() => {
                        sections.fills[main] = Some(slot);
                    }
                    _ => {
                        if let Some(free) = sections.mains.iter_mut().find(|m| m.is_none()) {
                            *free = Some(slot);
                        }
                    }
                }
            }
            previous = Some(slot);
        }
        sections
    }

    /// The pattern slot that plays a section.
    pub fn get(&self, section: Section) -> Option<usize> {
        match section {
            Section::Intro => self.intro,
            Section::Main(i) => self.mains.get(i).copied().flatten(),
            Section::Fill(i) => self.fills.get(i).copied().flatten(),
            Section::Break => self.fill_break,
            Section::Ending => self.ending,
        }
    }

    /// Sets the pattern slot that plays a section. An index past Main D or Fill D is ignored.
    pub fn set(&mut self, section: Section, slot: Option<usize>) {
        let place = match section {
            Section::Intro => &mut self.intro,
            Section::Main(i) => match self.mains.get_mut(i) {
                Some(place) => place,
                None => return,
            },
            Section::Fill(i) => match self.fills.get_mut(i) {
                Some(place) => place,
                None => return,
            },
            Section::Break => &mut self.fill_break,
            Section::Ending => &mut self.ending,
        };
        *place = slot;
    }
}

impl KeyboardKind {
    /// Every kind, in the order a list shows them.
    pub const ALL: &'static [KeyboardKind] = &[KeyboardKind::Yamaha];

    /// The style file's extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            KeyboardKind::Yamaha => yamaha::EXTENSION,
        }
    }

    /// A file name for a style called `name`, safe to write on the keyboard: plain ASCII, no
    /// character a file name may not hold, and this kind's extension.
    pub fn file_name(self, name: &str) -> String {
        let stem: String = yamaha::style_name(name)
            .chars()
            .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
            .collect();
        format!("{}.{}", stem.trim(), self.extension())
    }

    /// Whether this kind of style has the section.
    pub fn has(self, section: Section) -> bool {
        match self {
            KeyboardKind::Yamaha => yamaha::has(section),
        }
    }

    /// Whether the section must be a one-bar pattern.
    pub fn one_bar(self, section: Section) -> bool {
        match self {
            KeyboardKind::Yamaha => yamaha::one_bar(section),
        }
    }

    /// The first problem that stops the export, if any. A fill whose main is left out is not
    /// written, so it is not checked; neither is a section this kind does not have.
    pub fn problem(self, project: &Project, sections: &StyleSections) -> Option<StyleProblem> {
        self.parts(project, sections).err()
    }

    /// Writes the style file, with `name` as the style's name. Refused with the first
    /// [`StyleProblem`]. Each section plays its pattern's playing bars; hidden bars are left
    /// out. A main without a fill gets its own last bar as the fill. Mute, solo, row volume and
    /// swing apply as in [`crate::to_midi`].
    pub fn write(
        self,
        project: &Project,
        sections: &StyleSections,
        name: &str,
    ) -> Result<Vec<u8>, StyleProblem> {
        let parts = self.parts(project, sections)?;
        Ok(match self {
            KeyboardKind::Yamaha => yamaha::write(project, &parts, name),
        })
    }

    /// The parts to write, in [`Section::ALL`] order, each with the steps it plays.
    fn parts<'a>(
        self,
        project: &'a Project,
        sections: &StyleSections,
    ) -> Result<Vec<Part<'a>>, StyleProblem> {
        if sections.mains[0].is_none() {
            return Err(StyleProblem::NoMainA);
        }
        let mut parts = Vec::new();
        for section in Section::ALL {
            if !self.has(section) {
                continue;
            }
            // A fill goes with its main: without the main it is left out, and without a fill of
            // its own the main lends its last bar.
            let main = match section {
                Section::Fill(i) => match sections.mains[i] {
                    Some(main) => Some(main),
                    None => continue,
                },
                _ => None,
            };
            let part = match (sections.get(section), main) {
                (Some(slot), _) => {
                    let pattern = find(project, slot).ok_or(StyleProblem::NoSuchPattern)?;
                    if self.one_bar(section) && pattern.bars() != 1 {
                        return Err(StyleProblem::NotOneBar);
                    }
                    Part::whole(section, pattern)
                }
                (None, Some(main)) => {
                    let pattern = find(project, main).ok_or(StyleProblem::NoSuchPattern)?;
                    Part {
                        section,
                        pattern,
                        first: pattern.length() - STEPS_PER_BAR,
                        steps: STEPS_PER_BAR,
                    }
                }
                (None, None) => continue,
            };
            parts.push(part);
        }
        Ok(parts)
    }
}

/// One section of the file: which section it is and the steps of a pattern it plays.
struct Part<'a> {
    section: Section,
    pattern: &'a Pattern,
    /// The first step, at the start of a bar.
    first: usize,
    /// The number of steps, a whole number of bars.
    steps: usize,
}

impl<'a> Part<'a> {
    /// A part that plays every playing bar of the pattern.
    fn whole(section: Section, pattern: &'a Pattern) -> Self {
        Self {
            section,
            pattern,
            first: 0,
            steps: pattern.length(),
        }
    }
}

/// The pattern in an engine slot.
fn find(project: &Project, slot: usize) -> Option<&Pattern> {
    project.patterns.iter().find(|p| p.slot == slot)
}

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
