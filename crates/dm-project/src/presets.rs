//! Preset beats: ready-made projects a beginner can start from.
//!
//! Each preset is written as step strings, one per row, so it reads like a drum chart. A string
//! has 16 characters per bar, one per step; `|` between bars is only for reading.
//!
//! - `.` is off.
//! - `x` is normal ([`DEFAULT_VELOCITY`]).
//! - `X` is an accent ([`ACCENT_VELOCITY`]).
//! - `o` is soft ([`SOFT_VELOCITY`]), used for ghost notes and quiet hi-hats.
//!
//! Every preset uses the default rows, so it behaves like any other new project.

use crate::{ACCENT_VELOCITY, DEFAULT_VELOCITY, Project, SOFT_VELOCITY, STEPS_PER_BAR, StepLevels};

/// A preset beat as the picker shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// The style, such as "Rock". Also the name [`preset_project`] takes.
    pub name: &'static str,
    /// One plain sentence describing how it sounds.
    pub description: &'static str,
}

// Positions of the default rows (see `DEFAULT_ROWS`).
const KICK: usize = 0;
const SNARE: usize = 1;
const HAT: usize = 2;
const OPEN_HAT: usize = 3;
const CLAP: usize = 4;
const LOW_TOM: usize = 5;
const MID_TOM: usize = 6;
const HIGH_TOM: usize = 7;

/// One pattern: (row, steps) for every row that plays.
type PatternDef = &'static [(usize, &'static str)];

/// Everything a preset sets besides its name and description.
struct Groove {
    tempo: f32,
    swing: f32,
    /// Patterns A, B and so on, in order.
    patterns: &'static [PatternDef],
    /// The song as pattern positions: 0 is A, 1 is B.
    song: &'static [usize],
}

static PRESETS: [Preset; 7] = [
    Preset {
        name: "Rock",
        description: "A steady straight beat with the kick on 1 and 3, the snare on 2 and 4, \
                      and a drum fill every fourth bar.",
    },
    Preset {
        name: "Pop",
        description: "A bright, even beat with snare and clap together on 2 and 4 over busy \
                      hi-hats.",
    },
    Preset {
        name: "Hip-hop",
        description: "A laid-back boom bap groove with a heavy kick, a hard snare and a little \
                      swing.",
    },
    Preset {
        name: "Funk",
        description: "A busy groove with syncopated kicks, accented hi-hats and quiet ghost \
                      notes on the snare.",
    },
    Preset {
        name: "Bossa nova",
        description: "A gentle Brazilian rhythm with a rocking kick and a soft snare clave \
                      over two bars.",
    },
    Preset {
        name: "House",
        description: "A dance beat with the kick on every beat, claps on 2 and 4 and open \
                      hi-hats in between.",
    },
    Preset {
        name: "Reggae",
        description: "A relaxed one drop with kick and snare together on beat 3 and shuffled \
                      hi-hats.",
    },
];

/// The grooves, in the same order as [`PRESETS`].
static GROOVES: [Groove; 7] = [
    // Rock: eighth-note hats with the beats accented; B ends in a snare and tom fill.
    Groove {
        tempo: 110.0,
        swing: 0.0,
        patterns: &[
            &[
                (HAT, "X.x.X.x.X.x.X.x."),
                (SNARE, "....x.......x..."),
                (KICK, "x.......x.x....."),
            ],
            &[
                (HAT, "X.x.X.x........."),
                (SNARE, "....x...xxxx...."),
                (HIGH_TOM, "............xx.."),
                (MID_TOM, "..............x."),
                (LOW_TOM, "...............x"),
                (KICK, "x.......x......."),
            ],
        ],
        song: &[0, 0, 0, 1],
    },
    // Pop: sixteenth hats alternating normal and soft; B adds an open hat and a kick.
    Groove {
        tempo: 100.0,
        swing: 0.0,
        patterns: &[
            &[
                (HAT, "xoxoxoxoxoxoxoxo"),
                (CLAP, "....x.......x..."),
                (SNARE, "....x.......x..."),
                (KICK, "x.....x.x......."),
            ],
            &[
                (HAT, "xoxoxoxoxoxoxo.."),
                (OPEN_HAT, "..............x."),
                (CLAP, "....x.......x..."),
                (SNARE, "....x.......x..."),
                (KICK, "x.....x.x.x....."),
            ],
        ],
        song: &[0, 0, 0, 1],
    },
    // Hip-hop: boom bap with accented snares; swing pushes the odd sixteenths late.
    Groove {
        tempo: 90.0,
        swing: 0.16,
        patterns: &[
            &[
                (HAT, "x.x.x.x.x.x.x.xo"),
                (SNARE, "....X.......X..."),
                (KICK, "x......x..x....."),
            ],
            &[
                (HAT, "x.x.x.x.x.x.x.x."),
                (SNARE, "....X.......X..o"),
                (KICK, "x......x..x..x.."),
            ],
        ],
        song: &[0, 0, 0, 1],
    },
    // Funk: sixteenth hats accented on the beat, ghost notes around the backbeat.
    Groove {
        tempo: 100.0,
        swing: 0.0,
        patterns: &[
            &[
                (HAT, "XoxoXoxoXoxoXoxo"),
                (SNARE, "....X..o.o..X..o"),
                (KICK, "x.x.......x..x.."),
            ],
            &[
                (HAT, "XoxoXoxoXoxoXo.."),
                (OPEN_HAT, "..............x."),
                (SNARE, "....X..o.o..X.o."),
                (KICK, "x.x.......x.x..."),
            ],
        ],
        song: &[0, 0, 0, 1],
    },
    // Bossa nova: the kick on 1, the "and" of 2, 3 and the "and" of 4; the snare plays the
    // 3-2 bossa clave softly, standing in for a side stick.
    Groove {
        tempo: 130.0,
        swing: 0.0,
        patterns: &[&[
            (HAT, "x.o.x.o.x.o.x.o.|x.o.x.o.x.o.x.o."),
            (SNARE, "o.....o.....o...|....o.....o....."),
            (KICK, "x.....x.x.....x.|x.....x.x.....x."),
        ]],
        song: &[0],
    },
    // House: four on the floor, open hats on the off-beats, soft closed hats between; B ends
    // with a snare roll that grows louder.
    Groove {
        tempo: 124.0,
        swing: 0.0,
        patterns: &[
            &[
                (HAT, ".o.o.o.o.o.o.o.o"),
                (OPEN_HAT, "..x...x...x...x."),
                (CLAP, "....x.......x..."),
                (KICK, "x...x...x...x..."),
            ],
            &[
                (HAT, ".o.o.o.o.o.o.o.o"),
                (OPEN_HAT, "..x...x...x...x."),
                (CLAP, "....x.......x..."),
                (SNARE, "............oxXX"),
                (KICK, "x...x...x...x..."),
            ],
        ],
        song: &[0, 0, 0, 1],
    },
    // Reggae: one drop on beat 3; with triplet swing the soft fourth sixteenth of each beat
    // lands on the last triplet, giving the hats a shuffle.
    Groove {
        tempo: 76.0,
        swing: 0.33,
        patterns: &[&[
            (HAT, "x.xox.xox.xox.xo"),
            (SNARE, "........x......."),
            (KICK, "........x......."),
        ]],
        song: &[0],
    },
];

/// The preset beats, in the order the picker shows them.
pub fn presets() -> &'static [Preset] {
    &PRESETS
}

/// A new project holding the preset beat named `name` (ASCII case is ignored): the default rows,
/// its patterns, song, tempo and swing. Its step levels are the fewest that reach every note it
/// uses: on and off, soft, or loud and soft. `None` if no preset has that name.
pub fn preset_project(name: &str) -> Option<Project> {
    let index = PRESETS
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(name))?;
    Some(build(&GROOVES[index]))
}

fn build(groove: &Groove) -> Project {
    let mut project = Project::new();
    project.tempo = groove.tempo;
    project.set_swing(groove.swing);
    let (mut soft, mut loud) = (false, false);
    for (index, rows) in groove.patterns.iter().enumerate() {
        if index > 0 {
            project.add_pattern().expect("presets have few patterns");
        }
        let parsed: Vec<(usize, Vec<u8>)> = rows
            .iter()
            .map(|&(row, text)| (row, velocities(text)))
            .collect();
        let steps = parsed[0].1.len();
        assert!(
            steps > 0
                && steps.is_multiple_of(STEPS_PER_BAR)
                && parsed.iter().all(|(_, v)| v.len() == steps),
            "every row of a preset pattern has the same whole number of bars"
        );
        for _ in 1..steps / STEPS_PER_BAR {
            project.add_bar(index).expect("presets have few bars");
        }
        for (row, velocities) in parsed {
            for (step, velocity) in velocities.into_iter().enumerate() {
                if velocity > 0 {
                    soft |= velocity < DEFAULT_VELOCITY;
                    loud |= velocity > DEFAULT_VELOCITY;
                    project
                        .set_step(index, row, step, velocity)
                        .expect("preset steps are in range");
                }
            }
        }
    }
    project.song = groove.song.to_vec();
    project.set_step_levels(if loud {
        StepLevels::LoudAndSoft
    } else if soft {
        StepLevels::Soft
    } else {
        StepLevels::OnOff
    });
    project
}

/// The velocities a step string describes. Panics on a character it does not know, since the
/// strings are fixed in the code and the tests build every preset.
fn velocities(text: &str) -> Vec<u8> {
    text.chars()
        .filter(|&c| c != '|')
        .map(|c| match c {
            '.' => 0,
            'x' => DEFAULT_VELOCITY,
            'X' => ACCENT_VELOCITY,
            'o' => SOFT_VELOCITY,
            other => panic!("unknown step character {other:?} in a preset"),
        })
        .collect()
}

#[cfg(test)]
#[path = "presets_tests.rs"]
mod tests;
