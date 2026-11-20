//! TickTapDrum's project: rows, patterns with their bars, and the song.
//!
//! The project is plain data with no audio in it. Every operation that changes what plays returns
//! the [`Change`]s the audio engine needs, so the interface applies them and nothing is forgotten.
//!
//! Two safety rules from the design live here:
//!
//! - **Reducing a pattern's bar count never deletes notes.** The bars beyond the count are hidden
//!   and kept. Only [`Project::clear_hidden_bars`] removes them.
//! - **Taking a pattern out of the song never deletes the pattern.** Only
//!   [`Project::delete_pattern`] does, and it refuses to delete the last one.
//!
//! [`to_json`] and [`from_json`] save and load a project as a JSON file with the extension
//! [`FILE_EXTENSION`]. The file holds the hidden bars too, so they survive saving and loading.
//!
//! A project also holds the song's swing ([`Project::set_swing`]) and which loudness levels a
//! tap on a step goes through ([`Project::set_step_levels`], [`next_level`]): on and off; on,
//! soft and off; or on, loud, soft and off.
//!
//! [`to_midi`] writes the song as a Standard MIDI File for other music software, and a
//! [`KeyboardKind`] writes the patterns as the drum part of an arranger keyboard's style, with
//! the sections chosen in [`StyleSections`]. [`presets`] with [`preset_project`] give beginners ready-made
//! beats to start from.

mod file;
mod import;
mod midi;
mod presets;
mod style;

pub use file::{FILE_EXTENSION, FILE_FORMAT, InvalidReason, LoadError, from_json, to_json};
pub use import::{BarHit, Imported, repeat_unit};
pub use midi::{MIDI_EXTENSION, TICKS_PER_QUARTER, TICKS_PER_STEP, to_midi};
pub use presets::{Preset, preset_project, presets};
pub use style::{KeyboardKind, Section, StyleProblem, StyleSections};

/// Steps in one bar: four beats of four steps.
pub const STEPS_PER_BAR: usize = 16;
/// The most bars a pattern can have.
pub const MAX_BARS: usize = 16;
/// The most steps a pattern can hold, hidden bars included.
pub const MAX_STEPS: usize = STEPS_PER_BAR * MAX_BARS;
/// The most patterns a project can have. They are named A to Z.
pub const MAX_PATTERNS: usize = 26;
/// The most entries the song can have.
pub const MAX_SONG: usize = 256;
/// The velocity a step gets when it is turned on. With loudness levels on, this is "normal".
pub const DEFAULT_VELOCITY: u8 = 100;
/// The velocity of a soft step, with loudness levels on.
pub const SOFT_VELOCITY: u8 = 60;
/// The velocity of an accented step, with loudness levels on.
pub const ACCENT_VELOCITY: u8 = 127;
/// The most swing: every odd step is delayed by half a step.
pub const MAX_SWING: f32 = 0.5;

/// Which loudness levels a tap on a step goes through. A setting saved with the project; it
/// changes only what a tap does, and steps keep their loudness whichever is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepLevels {
    /// A tap turns a step on or off.
    #[default]
    OnOff,
    /// On, soft, off.
    Soft,
    /// On, loud, soft, off.
    LoudAndSoft,
}

impl StepLevels {
    /// The next choice, for a button that cycles through them: on and off, then soft, then
    /// loud and soft, then back.
    pub fn next(self) -> Self {
        match self {
            Self::OnOff => Self::Soft,
            Self::Soft => Self::LoudAndSoft,
            Self::LoudAndSoft => Self::OnOff,
        }
    }
}

/// The velocity a tap gives a step that has `velocity` now.
///
/// - [`StepLevels::OnOff`]: off becomes on ([`DEFAULT_VELOCITY`]); anything else becomes off.
/// - [`StepLevels::Soft`]: off, on, soft ([`SOFT_VELOCITY`]), off. A loud step becomes soft.
/// - [`StepLevels::LoudAndSoft`]: off, on, loud ([`ACCENT_VELOCITY`]), soft, off.
///
/// With loud and soft, a velocity that is none of these, such as one from a file, moves to the
/// quietest level that is louder than it: 1 to 59 become soft, 61 to 99 normal and 101 to 126
/// loud. Anything above [`ACCENT_VELOCITY`] counts as loud and becomes soft. So a tap never turns
/// a note off unless it was soft.
pub fn next_level(velocity: u8, levels: StepLevels) -> u8 {
    match levels {
        StepLevels::OnOff => {
            if velocity == 0 {
                DEFAULT_VELOCITY
            } else {
                0
            }
        }
        StepLevels::Soft => match velocity {
            0 => DEFAULT_VELOCITY,
            v if v > SOFT_VELOCITY => SOFT_VELOCITY,
            _ => 0,
        },
        StepLevels::LoudAndSoft => loud_and_soft_level(velocity),
    }
}

fn loud_and_soft_level(velocity: u8) -> u8 {
    match velocity {
        0 => DEFAULT_VELOCITY,
        DEFAULT_VELOCITY => ACCENT_VELOCITY,
        SOFT_VELOCITY => 0,
        v if v < SOFT_VELOCITY => SOFT_VELOCITY,
        v if v < DEFAULT_VELOCITY => DEFAULT_VELOCITY,
        v if v < ACCENT_VELOCITY => ACCENT_VELOCITY,
        _ => SOFT_VELOCITY,
    }
}
/// The tempo a new project starts at.
pub const DEFAULT_TEMPO: f32 = 100.0;

/// The rows a new project starts with: a name and a General MIDI drum key.
pub const DEFAULT_ROWS: [(&str, u8); 10] = [
    ("Kick", 36),
    ("Snare", 38),
    ("Closed hat", 42),
    ("Open hat", 46),
    ("Clap", 39),
    ("Low tom", 45),
    ("Mid tom", 47),
    ("High tom", 50),
    ("Crash", 49),
    ("Ride", 51),
];

/// The most drum rows a project can have.
pub const MAX_ROWS: usize = 32;
/// A row's volume when it is added, in percent.
pub const DEFAULT_VOLUME: u8 = 100;
/// The kit a new project starts with: General MIDI's Standard kit.
pub const DEFAULT_KIT: u8 = 0;

/// The General MIDI percussion keys and their names, 35 to 81.
pub const GM_DRUMS: [(u8, &str); 47] = [
    (35, "Bass drum 2"),
    (36, "Kick"),
    (37, "Side stick"),
    (38, "Snare"),
    (39, "Clap"),
    (40, "Electric snare"),
    (41, "Low floor tom"),
    (42, "Closed hat"),
    (43, "High floor tom"),
    (44, "Pedal hat"),
    (45, "Low tom"),
    (46, "Open hat"),
    (47, "Mid tom"),
    (48, "High mid tom"),
    (49, "Crash"),
    (50, "High tom"),
    (51, "Ride"),
    (52, "China cymbal"),
    (53, "Ride bell"),
    (54, "Tambourine"),
    (55, "Splash"),
    (56, "Cowbell"),
    (57, "Crash 2"),
    (58, "Vibraslap"),
    (59, "Ride 2"),
    (60, "High bongo"),
    (61, "Low bongo"),
    (62, "Mute high conga"),
    (63, "Open high conga"),
    (64, "Low conga"),
    (65, "High timbale"),
    (66, "Low timbale"),
    (67, "High agogo"),
    (68, "Low agogo"),
    (69, "Cabasa"),
    (70, "Maracas"),
    (71, "Short whistle"),
    (72, "Long whistle"),
    (73, "Short guiro"),
    (74, "Long guiro"),
    (75, "Claves"),
    (76, "High wood block"),
    (77, "Low wood block"),
    (78, "Mute cuica"),
    (79, "Open cuica"),
    (80, "Mute triangle"),
    (81, "Open triangle"),
];

/// The drums offered first when choosing one, by General MIDI key: the sounds most beats use.
/// The rest of [`GM_DRUMS`] are one tap further away.
pub const COMMON_DRUMS: [u8; 12] = [36, 38, 37, 39, 42, 44, 46, 45, 50, 49, 51, 56];

/// A family of drums that sound alike. The interface gives each family its own colour, so the
/// rows can be told apart at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumFamily {
    Kick,
    /// Snares, the side stick and the clap.
    Snare,
    HiHat,
    Tom,
    Cymbal,
    /// Everything else: hand drums, bells, shakers, whistles and wood.
    Percussion,
}

/// The family of a General MIDI percussion key.
pub fn drum_family(key: u8) -> DrumFamily {
    match key {
        35 | 36 => DrumFamily::Kick,
        37..=40 => DrumFamily::Snare,
        42 | 44 | 46 => DrumFamily::HiHat,
        41 | 43 | 45 | 47 | 48 | 50 => DrumFamily::Tom,
        49 | 51 | 52 | 53 | 55 | 57 | 59 => DrumFamily::Cymbal,
        _ => DrumFamily::Percussion,
    }
}

/// The name General MIDI gives a percussion key, or a generic one.
pub fn drum_name(key: u8) -> String {
    GM_DRUMS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, name)| (*name).to_owned())
        .unwrap_or_else(|| format!("Drum {key}"))
}

/// Something the audio engine must be told after an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// A step's velocity in a pattern slot; 0 is off.
    Step {
        slot: usize,
        row: usize,
        step: usize,
        velocity: u8,
    },
    /// How many steps a pattern slot loops over.
    Length { slot: usize, steps: usize },
    /// The whole song order, as pattern slots.
    Song(Vec<usize>),
    /// Which drum a row plays.
    RowKey { row: usize, key: u8 },
    /// How loud a row plays, in percent; 0 is silent. Mute and solo are already applied.
    RowGain { row: usize, percent: u8 },
    /// Every step of a pattern slot becomes 0. Sent before a slot is filled again.
    ClearSlot { slot: usize },
    /// The drum kit, a program number in the percussion bank.
    Kit(u8),
    /// The swing in thousandths of a step, 0 to 500: how late every odd step starts.
    Swing(u16),
}

/// Why an operation was refused. Nothing changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The pattern already has [`MAX_BARS`] bars.
    TooManyBars,
    /// The pattern has only one bar.
    LastBar,
    /// Copying would push notes off the end of the pattern: its last possible bar holds notes.
    NoRoomToCopy,
    /// The project already has [`MAX_PATTERNS`] patterns.
    TooManyPatterns,
    /// The project has only one pattern.
    LastPattern,
    /// The song already has [`MAX_SONG`] entries.
    SongFull,
    /// The index does not exist.
    OutOfRange,
    /// The project already has [`MAX_ROWS`] rows.
    TooManyRows,
    /// The project has only one row.
    LastRow,
}

/// One drum row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub name: String,
    /// The General MIDI drum key, such as 36 for the kick.
    pub key: u8,
    /// How loud the row plays, 0 to 100 percent.
    pub volume: u8,
    /// A muted row is silent; its steps are kept.
    pub mute: bool,
    /// While any row is soloed, only soloed rows play.
    pub solo: bool,
}

impl Row {
    fn new(name: &str, key: u8) -> Self {
        Self {
            name: name.to_owned(),
            key,
            volume: DEFAULT_VOLUME,
            mute: false,
            solo: false,
        }
    }
}

/// One pattern. Its slot is a fixed position in the audio engine. Its letter is what the user
/// sees; it starts as the slot's letter but can be changed ([`Project::rename_pattern`]), and no
/// two patterns share one.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    slot: usize,
    name: char,
    bars: usize,
    /// One array per row, covering every possible step. Steps at and beyond `bars *
    /// STEPS_PER_BAR` are the hidden bars.
    steps: Vec<[u8; MAX_STEPS]>,
}

impl Pattern {
    fn new(slot: usize, name: char, rows: usize) -> Self {
        Self {
            slot,
            name,
            bars: 1,
            steps: vec![[0; MAX_STEPS]; rows],
        }
    }

    /// The engine slot, 0 to 25.
    pub fn slot(&self) -> usize {
        self.slot
    }

    /// The pattern's letter, A to Z.
    pub fn name(&self) -> char {
        self.name
    }

    /// The number of bars that play.
    pub fn bars(&self) -> usize {
        self.bars
    }

    /// The number of steps that play.
    pub fn length(&self) -> usize {
        self.bars * STEPS_PER_BAR
    }

    /// The number of rows, the same as the project's.
    pub fn rows(&self) -> usize {
        self.steps.len()
    }

    /// A step's velocity; 0 is off.
    pub fn velocity(&self, row: usize, step: usize) -> u8 {
        self.steps
            .get(row)
            .and_then(|r| r.get(step))
            .copied()
            .unwrap_or(0)
    }

    /// Whether any row has a note in this bar.
    pub fn bar_has_notes(&self, bar: usize) -> bool {
        let range = bar * STEPS_PER_BAR..(bar + 1) * STEPS_PER_BAR;
        bar < MAX_BARS
            && self
                .steps
                .iter()
                .any(|r| r[range.clone()].iter().any(|&v| v > 0))
    }

    /// Whether the hidden bars hold any notes.
    pub fn has_hidden_notes(&self) -> bool {
        (self.bars..MAX_BARS).any(|bar| self.bar_has_notes(bar))
    }

    /// The `Step` changes that make an engine slot hold exactly this pattern, plus its length.
    fn full_sync(&self, changes: &mut Vec<Change>) {
        changes.push(Change::Length {
            slot: self.slot,
            steps: self.length(),
        });
        for (row, steps) in self.steps.iter().enumerate() {
            for (step, &velocity) in steps.iter().enumerate() {
                if velocity > 0 {
                    changes.push(Change::Step {
                        slot: self.slot,
                        row,
                        step,
                        velocity,
                    });
                }
            }
        }
    }
}

/// The letter at a position in the alphabet, 0 for A to 25 for Z.
pub fn letter(index: usize) -> char {
    (b'A' + (index % MAX_PATTERNS) as u8) as char
}

/// A whole project.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    rows: Vec<Row>,
    /// In the order the user sees them.
    patterns: Vec<Pattern>,
    /// Pattern slots, in play order.
    song: Vec<usize>,
    pub song_loop: bool,
    pub tempo: f32,
    kit: u8,
    /// The fraction of a step every odd step is delayed, 0 to [`MAX_SWING`].
    swing: f32,
    /// Which loudness levels a tap goes through. Only the interface uses it.
    step_levels: StepLevels,
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

impl Project {
    /// A new project: the default rows, pattern A of one bar, and a song that plays A.
    pub fn new() -> Self {
        let rows: Vec<Row> = DEFAULT_ROWS
            .iter()
            .map(|(name, key)| Row::new(name, *key))
            .collect();
        let patterns = vec![Pattern::new(0, 'A', rows.len())];
        Self {
            rows,
            patterns,
            song: vec![0],
            song_loop: true,
            tempo: DEFAULT_TEMPO,
            kit: DEFAULT_KIT,
            swing: 0.0,
            step_levels: StepLevels::OnOff,
        }
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The drum kit, a program number in the percussion bank.
    pub fn kit(&self) -> u8 {
        self.kit
    }

    /// Whether a row is heard, after mute and solo.
    pub fn audible(&self, row: usize) -> bool {
        let any_solo = self.rows.iter().any(|r| r.solo);
        self.rows
            .get(row)
            .is_some_and(|r| !r.mute && (!any_solo || r.solo))
    }

    /// A row's effective volume in percent: 0 when mute or solo silence it.
    pub fn gain(&self, row: usize) -> u8 {
        if self.audible(row) {
            self.rows[row].volume
        } else {
            0
        }
    }

    /// Whether any pattern has a note in this row, hidden bars included.
    pub fn row_has_notes(&self, row: usize) -> bool {
        self.patterns
            .iter()
            .any(|p| p.steps.get(row).is_some_and(|r| r.iter().any(|&v| v > 0)))
    }

    /// The effective volume of every row.
    fn gains(&self) -> Vec<Change> {
        (0..self.rows.len())
            .map(|row| Change::RowGain {
                row,
                percent: self.gain(row),
            })
            .collect()
    }

    /// Everything about rows and patterns, re-sent after rows are removed or reordered. Each
    /// slot is cleared first, so steps that moved away do not stay behind.
    fn resync_rows(&self) -> Vec<Change> {
        let mut changes = Vec::new();
        for (row, info) in self.rows.iter().enumerate() {
            changes.push(Change::RowKey { row, key: info.key });
        }
        changes.extend(self.gains());
        for pattern in &self.patterns {
            changes.push(Change::ClearSlot { slot: pattern.slot });
            pattern.full_sync(&mut changes);
        }
        changes
    }

    /// Adds a row at the bottom playing `key`, named after it.
    pub fn add_row(&mut self, key: u8) -> Result<Vec<Change>, Refused> {
        if self.rows.len() >= MAX_ROWS {
            return Err(Refused::TooManyRows);
        }
        self.rows.push(Row::new(&drum_name(key), key));
        for pattern in &mut self.patterns {
            pattern.steps.push([0; MAX_STEPS]);
        }
        let row = self.rows.len() - 1;
        let mut changes = vec![Change::RowKey { row, key }];
        // A new solo-free row may change nothing else, but a soloed project silences it.
        changes.extend(self.gains());
        Ok(changes)
    }

    /// Removes a row and its notes in every pattern. The interface asks first when
    /// [`Project::row_has_notes`] is true. Refused for the last row.
    pub fn remove_row(&mut self, row: usize) -> Result<Vec<Change>, Refused> {
        if row >= self.rows.len() {
            return Err(Refused::OutOfRange);
        }
        if self.rows.len() == 1 {
            return Err(Refused::LastRow);
        }
        self.rows.remove(row);
        for pattern in &mut self.patterns {
            pattern.steps.remove(row);
        }
        Ok(self.resync_rows())
    }

    /// Moves a row to another position, notes and all.
    pub fn move_row(&mut self, from: usize, to: usize) -> Result<Vec<Change>, Refused> {
        if from >= self.rows.len() || to >= self.rows.len() {
            return Err(Refused::OutOfRange);
        }
        if from == to {
            return Ok(Vec::new());
        }
        let info = self.rows.remove(from);
        self.rows.insert(to, info);
        for pattern in &mut self.patterns {
            let steps = pattern.steps.remove(from);
            pattern.steps.insert(to, steps);
        }
        Ok(self.resync_rows())
    }

    /// Renames a row. Nothing changes in the sound.
    pub fn rename_row(&mut self, row: usize, name: &str) -> Result<(), Refused> {
        let info = self.rows.get_mut(row).ok_or(Refused::OutOfRange)?;
        name.trim().clone_into(&mut info.name);
        Ok(())
    }

    /// Changes which drum a row plays. Its notes stay where they are.
    pub fn set_row_key(&mut self, row: usize, key: u8) -> Result<Vec<Change>, Refused> {
        let info = self.rows.get_mut(row).ok_or(Refused::OutOfRange)?;
        info.key = key;
        Ok(vec![Change::RowKey { row, key }])
    }

    /// Sets a row's volume, 0 to 100 percent.
    pub fn set_volume(&mut self, row: usize, percent: u8) -> Result<Vec<Change>, Refused> {
        let info = self.rows.get_mut(row).ok_or(Refused::OutOfRange)?;
        info.volume = percent.min(100);
        Ok(vec![Change::RowGain {
            row,
            percent: self.gain(row),
        }])
    }

    /// Mutes or unmutes a row.
    pub fn toggle_mute(&mut self, row: usize) -> Result<Vec<Change>, Refused> {
        let info = self.rows.get_mut(row).ok_or(Refused::OutOfRange)?;
        info.mute = !info.mute;
        Ok(self.gains())
    }

    /// Solos or unsolos a row. Solo affects every row, so every row's gain is sent.
    pub fn toggle_solo(&mut self, row: usize) -> Result<Vec<Change>, Refused> {
        let info = self.rows.get_mut(row).ok_or(Refused::OutOfRange)?;
        info.solo = !info.solo;
        Ok(self.gains())
    }

    /// Chooses the drum kit.
    pub fn set_kit(&mut self, program: u8) -> Vec<Change> {
        self.kit = program;
        vec![Change::Kit(program)]
    }

    /// The swing: the fraction of a step every odd step is delayed. 0 is straight timing,
    /// about 0.33 is a triplet shuffle and [`MAX_SWING`] is the most.
    pub fn swing(&self) -> f32 {
        self.swing
    }

    /// Sets the swing, clamped to 0 to [`MAX_SWING`]. A value that is not a number counts as 0.
    pub fn set_swing(&mut self, amount: f32) -> Vec<Change> {
        self.swing = if amount.is_nan() {
            0.0
        } else {
            amount.clamp(0.0, MAX_SWING)
        };
        vec![self.swing_change()]
    }

    fn swing_change(&self) -> Change {
        Change::Swing((self.swing * 1000.0).round().clamp(0.0, 500.0) as u16)
    }

    /// Which loudness levels a tap on a step goes through (see [`next_level`]). Saved with the
    /// project; the sound is unchanged.
    pub fn step_levels(&self) -> StepLevels {
        self.step_levels
    }

    /// Chooses which loudness levels a tap goes through. Steps keep their velocities.
    pub fn set_step_levels(&mut self, levels: StepLevels) {
        self.step_levels = levels;
    }

    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    /// The pattern at a position in [`Project::patterns`].
    pub fn pattern(&self, index: usize) -> Option<&Pattern> {
        self.patterns.get(index)
    }

    /// The position in [`Project::patterns`] of the pattern in `slot`.
    pub fn index_of_slot(&self, slot: usize) -> Option<usize> {
        self.patterns.iter().position(|p| p.slot == slot)
    }

    /// The letter of the pattern in `slot`, such as a song entry's, or `?` if no pattern is
    /// there.
    pub fn slot_name(&self, slot: usize) -> char {
        self.patterns
            .iter()
            .find(|p| p.slot == slot)
            .map_or('?', |p| p.name)
    }

    /// The song, as pattern slots in play order.
    pub fn song(&self) -> &[usize] {
        &self.song
    }

    /// Every change needed to bring a freshly started engine in line with this project: rows,
    /// kit, swing, patterns and song. Tempo and loop are not included; the caller sends those
    /// directly.
    pub fn full_sync(&self) -> Vec<Change> {
        let mut changes = vec![Change::Kit(self.kit), self.swing_change()];
        for (row, info) in self.rows.iter().enumerate() {
            changes.push(Change::RowKey { row, key: info.key });
        }
        changes.extend(self.gains());
        for pattern in &self.patterns {
            pattern.full_sync(&mut changes);
        }
        changes.push(Change::Song(self.song.clone()));
        changes
    }

    /// Sets one step. Returns the change, or nothing if the step already had that velocity.
    pub fn set_step(
        &mut self,
        index: usize,
        row: usize,
        step: usize,
        velocity: u8,
    ) -> Result<Vec<Change>, Refused> {
        let pattern = self.patterns.get_mut(index).ok_or(Refused::OutOfRange)?;
        let cell = pattern
            .steps
            .get_mut(row)
            .and_then(|r| r.get_mut(step))
            .ok_or(Refused::OutOfRange)?;
        if *cell == velocity {
            return Ok(Vec::new());
        }
        *cell = velocity;
        Ok(vec![Change::Step {
            slot: pattern.slot,
            row,
            step,
            velocity,
        }])
    }

    /// Adds a bar at the end. If that bar was hidden, its notes come back unchanged.
    pub fn add_bar(&mut self, index: usize) -> Result<Vec<Change>, Refused> {
        let pattern = self.patterns.get_mut(index).ok_or(Refused::OutOfRange)?;
        if pattern.bars >= MAX_BARS {
            return Err(Refused::TooManyBars);
        }
        pattern.bars += 1;
        Ok(vec![Change::Length {
            slot: pattern.slot,
            steps: pattern.length(),
        }])
    }

    /// Hides the last bar. Its notes are kept.
    pub fn remove_bar(&mut self, index: usize) -> Result<Vec<Change>, Refused> {
        let pattern = self.patterns.get_mut(index).ok_or(Refused::OutOfRange)?;
        if pattern.bars <= 1 {
            return Err(Refused::LastBar);
        }
        pattern.bars -= 1;
        Ok(vec![Change::Length {
            slot: pattern.slot,
            steps: pattern.length(),
        }])
    }

    /// Inserts a copy of `bar` right after it. Every later bar, hidden ones included, moves one
    /// bar later, so no note is overwritten. Refused if the last possible bar holds notes, because
    /// they would have nowhere to go.
    pub fn copy_bar(&mut self, index: usize, bar: usize) -> Result<Vec<Change>, Refused> {
        let pattern = self.patterns.get_mut(index).ok_or(Refused::OutOfRange)?;
        if bar >= pattern.bars {
            return Err(Refused::OutOfRange);
        }
        if pattern.bars >= MAX_BARS {
            return Err(Refused::TooManyBars);
        }
        if pattern.bar_has_notes(MAX_BARS - 1) {
            return Err(Refused::NoRoomToCopy);
        }
        let before = pattern.clone();
        let start = (bar + 1) * STEPS_PER_BAR;
        for row in pattern.steps.iter_mut() {
            row.copy_within(start..MAX_STEPS - STEPS_PER_BAR, start + STEPS_PER_BAR);
            row.copy_within(bar * STEPS_PER_BAR..start, start);
        }
        pattern.bars += 1;
        Ok(diff(&before, pattern))
    }

    /// Removes the notes in the hidden bars. Bars that play are untouched.
    pub fn clear_hidden_bars(&mut self, index: usize) -> Result<Vec<Change>, Refused> {
        let pattern = self.patterns.get_mut(index).ok_or(Refused::OutOfRange)?;
        let before = pattern.clone();
        let start = pattern.length();
        for row in pattern.steps.iter_mut() {
            row[start..].fill(0);
        }
        Ok(diff(&before, pattern))
    }

    /// Adds an empty one-bar pattern in the first free slot, named with the first free letter.
    /// Returns its index and the changes.
    pub fn add_pattern(&mut self) -> Result<(usize, Vec<Change>), Refused> {
        let slot = self.free_slot().ok_or(Refused::TooManyPatterns)?;
        let pattern = Pattern::new(slot, self.free_name(), self.rows.len());
        let mut changes = Vec::new();
        pattern.full_sync(&mut changes);
        self.patterns.push(pattern);
        Ok((self.patterns.len() - 1, changes))
    }

    /// Adds a copy of a pattern, hidden bars included, in the first free slot, named with the
    /// first free letter. The copy goes right after the original. Returns its index and the
    /// changes.
    pub fn copy_pattern(&mut self, index: usize) -> Result<(usize, Vec<Change>), Refused> {
        let original = self.patterns.get(index).ok_or(Refused::OutOfRange)?;
        let slot = self.free_slot().ok_or(Refused::TooManyPatterns)?;
        let mut copy = original.clone();
        copy.slot = slot;
        copy.name = self.free_name();
        let mut changes = Vec::new();
        copy.full_sync(&mut changes);
        self.patterns.insert(index + 1, copy);
        Ok((index + 1, changes))
    }

    /// Deletes a pattern and its entries in the song. This is the only way a pattern is deleted.
    /// Refused for the last pattern.
    pub fn delete_pattern(&mut self, index: usize) -> Result<Vec<Change>, Refused> {
        if index >= self.patterns.len() {
            return Err(Refused::OutOfRange);
        }
        if self.patterns.len() == 1 {
            return Err(Refused::LastPattern);
        }
        let removed = self.patterns.remove(index);
        let mut changes = Vec::new();
        // Leave the engine slot empty, so a later pattern in the same slot starts clean.
        diff_into(
            &removed,
            &Pattern::new(removed.slot, removed.name, self.rows.len()),
            &mut changes,
        );
        let before = self.song.len();
        self.song.retain(|&slot| slot != removed.slot);
        if self.song.len() != before {
            changes.push(Change::Song(self.song.clone()));
        }
        Ok(changes)
    }

    /// Gives a pattern another letter, A to Z. If another pattern has that letter, the two swap
    /// letters. Only the names change: the song keeps its patterns, so nothing changes in the
    /// sound. Refused with [`Refused::OutOfRange`] for a bad index or a character that is not
    /// a capital letter.
    pub fn rename_pattern(&mut self, index: usize, name: char) -> Result<(), Refused> {
        if index >= self.patterns.len() || !name.is_ascii_uppercase() {
            return Err(Refused::OutOfRange);
        }
        let old = self.patterns[index].name;
        if let Some(other) = self.patterns.iter_mut().find(|p| p.name == name) {
            other.name = old;
        }
        self.patterns[index].name = name;
        Ok(())
    }

    /// Adds a pattern to the end of the song.
    pub fn song_append(&mut self, index: usize) -> Result<Vec<Change>, Refused> {
        let slot = self.patterns.get(index).ok_or(Refused::OutOfRange)?.slot;
        if self.song.len() >= MAX_SONG {
            return Err(Refused::SongFull);
        }
        self.song.push(slot);
        Ok(vec![Change::Song(self.song.clone())])
    }

    /// Takes one entry out of the song. The pattern itself stays.
    pub fn song_remove(&mut self, entry: usize) -> Result<Vec<Change>, Refused> {
        if entry >= self.song.len() {
            return Err(Refused::OutOfRange);
        }
        self.song.remove(entry);
        Ok(vec![Change::Song(self.song.clone())])
    }

    /// Moves a song entry to another position.
    pub fn song_move(&mut self, from: usize, to: usize) -> Result<Vec<Change>, Refused> {
        if from >= self.song.len() || to >= self.song.len() {
            return Err(Refused::OutOfRange);
        }
        let slot = self.song.remove(from);
        self.song.insert(to, slot);
        Ok(vec![Change::Song(self.song.clone())])
    }

    fn free_slot(&self) -> Option<usize> {
        (0..MAX_PATTERNS).find(|slot| self.patterns.iter().all(|p| p.slot != *slot))
    }

    /// The first letter no pattern has. Call it only when there is room for another pattern.
    fn free_name(&self) -> char {
        (0..MAX_PATTERNS)
            .map(letter)
            .find(|&name| self.patterns.iter().all(|p| p.name != name))
            .unwrap_or('?')
    }
}

/// The changes that turn engine slot contents `before` into `after` (same slot).
fn diff(before: &Pattern, after: &Pattern) -> Vec<Change> {
    let mut changes = Vec::new();
    diff_into(before, after, &mut changes);
    changes
}

fn diff_into(before: &Pattern, after: &Pattern, changes: &mut Vec<Change>) {
    if before.bars != after.bars {
        changes.push(Change::Length {
            slot: after.slot,
            steps: after.length(),
        });
    }
    for (row, (a, b)) in before.steps.iter().zip(&after.steps).enumerate() {
        for step in 0..MAX_STEPS {
            if a[step] != b[step] {
                changes.push(Change::Step {
                    slot: after.slot,
                    row,
                    step,
                    velocity: b[step],
                });
            }
        }
    }
}

#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod tests;
