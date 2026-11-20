//! Import of MIDI groove packs: a `.mid` file, or a `.zip` of them, becomes grooves on the app's
//! 16-step grid, with a reason for every file refused.
//!
//! Only short drum loops in 4/4 are accepted, as `docs/DESIGN.md` ("Groove import") explains:
//! not songs, live recordings or files made for another drum program. The rule and the snapping
//! follow the groove spike (`tools/groove-spike`).
//!
//! A Yamaha keyboard style is read too: each of its sections becomes a groove or a refusal (see
//! the `style` module and "Import grooves from a keyboard style" in `docs/DESIGN.md`).
//!
//! This crate has no interface and no platform code. The groove list keeps accepted grooves as
//! JSON ([`pack_to_json`]), so it never needs the original files again. [`download`] fetches a
//! file from a link for [`import`].

mod groove;
mod json;
mod link;
mod midi;
mod style;
mod zipped;

pub use dm_project::BarHit;
pub use groove::{MAX_GROOVE_BARS, MAX_MIDI_BYTES, snap};
pub use json::PACK_FORMAT;
pub use link::{LinkError, download, link_address};

/// The largest file [`import`] reads, in bytes.
pub const MAX_INPUT_BYTES: usize = 30 * 1024 * 1024;
/// The most entries a zip may hold, folders included.
pub const MAX_ZIP_ENTRIES: usize = 2_000;
/// The most bytes a zip's entries may hold in total, unpacked.
pub const MAX_ZIP_UNPACKED_BYTES: u64 = 100 * 1024 * 1024;

/// One accepted groove.
#[derive(Debug, Clone, PartialEq)]
pub struct Groove {
    /// The file's name without its extension, such as `Chilled Beat 1 Clap A`. For a style's
    /// section, the style's name and the section's, such as `Bossa Main A`.
    pub name: String,
    /// The folder inside the pack, with `/` between parts; empty for a single file or the top.
    pub folder: String,
    /// The file's tempo, rounded to a whole number.
    pub bpm: f32,
    /// Bars of hits on the 16-step grid, after snapping. At most 16 bars, at least one.
    pub bars: Vec<Vec<BarHit>>,
    /// For a style's section, the keyboard's kit: the kit of channel 10 and of channel 9, joined
    /// by ` + ` when both play in the section, such as `PopDrumKit + PopPercussionKit`. Only kits
    /// the key map knows are named. `None` for a MIDI file. Information only: the app plays its
    /// own kit.
    pub kit: Option<String>,
    /// For a style's section, the General MIDI program of the app's kit nearest to the
    /// keyboard's: channel 10's, or channel 9's when channel 10's kit gives none.
    pub similar_kit: Option<u8>,
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
    /// Each refused file's path inside the pack (or its name), and why. A style's refused
    /// section is named like its groove would be, with the style's path inside the pack in
    /// front: `Styles/Bossa Main A`.
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
    /// A zip with no MIDI files or styles in it.
    NoMidiFiles,
}

/// A saved collection of grooves from one file or pack, as the groove list keeps it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pack {
    /// Shown as the pack's heading: the imported file's name without its extension.
    pub name: String,
    pub grooves: Vec<Groove>,
}

/// Reads a MIDI file, a Yamaha style or a zip of them. `name` is the file's name, used for the
/// result's names.
///
/// The kind of file comes from its content, not its name. A single MIDI file gives one groove or
/// one refusal, named after `name` without its extension. A style gives one groove or one
/// refusal per section, named after `name` without its extension and the section, such as
/// `Bossa Main A`. A zip gives the results of every MIDI file and style in it; see the crate's
/// `zipped` module for which files are read and how their folders are named. A style's grooves
/// go in a folder of their own, named after the style.
pub fn import(name: &str, bytes: &[u8]) -> Result<Import, ImportError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(ImportError::TooLarge);
    }
    let mut import = Import::default();
    if bytes.starts_with(b"MThd") {
        let stem = zipped::stem(name);
        match style::sections(bytes) {
            Some(sections) => add_style(&mut import, stem, stem, "", sections),
            None => add(&mut import, name, stem, "", Some(bytes)),
        }
    } else if bytes.starts_with(b"PK\x03\x04") {
        let contents = zipped::read(bytes)?;
        import.other_mappings_skipped = contents.other_mappings_skipped;
        for entry in &contents.entries {
            match entry.bytes.as_deref().and_then(style::sections) {
                Some(sections) => {
                    let folder = match entry.folder.as_str() {
                        "" => entry.name.clone(),
                        parent => format!("{parent}/{}", entry.name),
                    };
                    // The style's path without its extension.
                    let path = match entry.path.rsplit_once('/') {
                        Some((parent, _)) => format!("{parent}/{}", entry.name),
                        None => entry.name.clone(),
                    };
                    add_style(&mut import, &entry.name, &path, &folder, sections);
                }
                None => add(
                    &mut import,
                    &entry.path,
                    &entry.name,
                    &entry.folder,
                    entry.bytes.as_deref(),
                ),
            }
        }
    } else {
        return Err(ImportError::NotGrooves);
    }
    Ok(import)
}

/// Whether the bytes are a Yamaha keyboard style: a Standard MIDI File whose first track has a
/// section marker such as `Main A`. [`import`] reads such a file one section at a time.
pub fn is_style(bytes: &[u8]) -> bool {
    style::is_style(bytes)
}

/// Checks one MIDI file and adds it as a groove or a refusal. `bytes` is `None` for a file too
/// large to read.
fn add(import: &mut Import, path: &str, name: &str, folder: &str, bytes: Option<&[u8]>) {
    match bytes.ok_or(Refusal::NotMidi).and_then(groove::analyze) {
        Ok(fitted) => import.grooves.push(Groove {
            name: name.to_owned(),
            folder: folder.to_owned(),
            bpm: fitted.bpm,
            bars: fitted.bars,
            kit: None,
            similar_kit: None,
        }),
        Err(refusal) => import.refused.push((path.to_owned(), refusal)),
    }
}

/// Adds a style's sections as grooves or refusals. A groove is named `<name> <marker>`, and a
/// refusal `<path> <marker>`.
fn add_style(
    import: &mut Import,
    name: &str,
    path: &str,
    folder: &str,
    sections: Vec<style::Section>,
) {
    for section in sections {
        match section.result {
            Ok(found) => import.grooves.push(Groove {
                name: format!("{name} {}", section.marker),
                folder: folder.to_owned(),
                bpm: found.fitted.bpm,
                bars: found.fitted.bars,
                kit: found.kit,
                similar_kit: found.similar_kit,
            }),
            Err(refusal) => import
                .refused
                .push((format!("{path} {}", section.marker), refusal)),
        }
    }
}

/// Writes a pack as JSON for the groove list.
pub fn pack_to_json(pack: &Pack) -> String {
    json::to_json(pack)
}

/// Reads a pack written by [`pack_to_json`].
pub fn pack_from_json(text: &str) -> Result<Pack, String> {
    json::from_json(text)
}

#[cfg(test)]
mod tests;
