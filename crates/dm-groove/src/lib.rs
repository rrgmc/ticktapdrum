//! Placeholder with the agreed interface. Replaced by the real implementation.

pub use dm_project::BarHit;

/// One accepted groove.
#[derive(Debug, Clone, PartialEq)]
pub struct Groove {
    /// The file's name without its extension, such as `Chilled Beat 1 Clap A`.
    pub name: String,
    /// The folder inside the pack, with `/` between parts; empty for a single file or the top.
    pub folder: String,
    /// The file's tempo, rounded to a whole number.
    pub bpm: f32,
    /// Bars of hits on the 16-step grid, after snapping. At most 16 bars, at least one.
    pub bars: Vec<Vec<BarHit>>,
}

/// Why a MIDI file is not a groove the app accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Not a Standard MIDI File, or a damaged one.
    NotMidi,
    /// No notes to play.
    NoNotes,
    /// Notes on more than one channel: a song, not a drum groove.
    SeveralChannels,
    /// Not in 4/4, or the time signature changes.
    NotFourFour,
    /// The tempo changes.
    TempoChanges,
    /// Longer than 16 bars.
    TooLong,
    /// More than 10% of the notes are far from the 16-step grid: live playing, triplets, rolls.
    OffGrid,
    /// Fewer than 90% of the notes are on drum-kit keys, or keys outside General MIDI: made for
    /// another drum program.
    OtherDrumProgram,
}

/// The result of importing a file: the grooves accepted and the files refused.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Import {
    pub grooves: Vec<Groove>,
    /// Each refused file's path inside the pack (or its name), and why.
    pub refused: Vec<(String, Refusal)>,
    /// MIDI files skipped because the pack also holds a General MIDI version of them.
    pub other_mappings_skipped: usize,
}

/// Why a file could not be imported at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// Neither a MIDI file nor a zip.
    NotGrooves,
    /// Larger than the limits.
    TooLarge,
    /// A damaged zip.
    BrokenZip,
    /// A zip with no MIDI files in it.
    NoMidiFiles,
}

/// A saved collection of grooves from one file or pack, as the groove list keeps it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pack {
    /// Shown as the pack's heading: the imported file's name without its extension.
    pub name: String,
    pub grooves: Vec<Groove>,
}

/// Reads a MIDI file or a zip of them. `name` is the file's name, used for the result's names.
pub fn import(name: &str, bytes: &[u8]) -> Result<Import, ImportError> {
    let _ = (name, bytes);
    todo!("dm-groove is being written")
}

/// Writes a pack as JSON for the groove list.
pub fn pack_to_json(pack: &Pack) -> String {
    let _ = pack;
    todo!("dm-groove is being written")
}

/// Reads a pack written by [`pack_to_json`].
pub fn pack_from_json(text: &str) -> Result<Pack, String> {
    let _ = text;
    todo!("dm-groove is being written")
}
