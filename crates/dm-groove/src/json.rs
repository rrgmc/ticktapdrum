//! The groove list's file format: a pack as JSON.
//!
//! ```json
//! {"format":1,"name":"Beat Farm","grooves":[
//!   {"name":"Rock 1","folder":"Rock","bpm":120,"bars":[[[36,0,100],[42,0,90]]]}
//! ]}
//! ```
//!
//! Each hit is `[key, step, velocity]`, and each groove holds one list of hits per bar.
//!
//! A groove from a keyboard style may also have `"kit"` (the keyboard's kit names) and
//! `"similar_kit"` (a General MIDI program). Both are left out when empty. They were added
//! within format 1: an older reader ignores them, and a file without them reads as before.

use serde::{Deserialize, Serialize};

use crate::groove::MAX_GROOVE_BARS;
use crate::{BarHit, Groove, Pack};

/// The format this version writes, and the newest it reads.
pub const PACK_FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct PackFile {
    format: u32,
    name: String,
    grooves: Vec<GrooveFile>,
}

#[derive(Serialize, Deserialize)]
struct GrooveFile {
    name: String,
    folder: String,
    bpm: f32,
    bars: Vec<Vec<(u8, usize, u8)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    kit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    similar_kit: Option<u8>,
}

/// Only the format number, read first so a newer file gets a clear message.
#[derive(Deserialize)]
struct Header {
    format: u32,
}

pub fn to_json(pack: &Pack) -> String {
    let file = PackFile {
        format: PACK_FORMAT,
        name: pack.name.clone(),
        grooves: pack
            .grooves
            .iter()
            .map(|g| GrooveFile {
                name: g.name.clone(),
                folder: g.folder.clone(),
                bpm: g.bpm,
                bars: g
                    .bars
                    .iter()
                    .map(|bar| bar.iter().map(|h| (h.key, h.step, h.velocity)).collect())
                    .collect(),
                kit: g.kit.clone(),
                similar_kit: g.similar_kit,
            })
            .collect(),
    };
    serde_json::to_string(&file).expect("a pack always converts to JSON")
}

pub fn from_json(text: &str) -> Result<Pack, String> {
    let header: Header =
        serde_json::from_str(text).map_err(|e| format!("not a groove pack: {e}"))?;
    if header.format == 0 || header.format > PACK_FORMAT {
        return Err(format!(
            "groove pack format {} is not supported; this version reads format {PACK_FORMAT}",
            header.format
        ));
    }
    let file: PackFile =
        serde_json::from_str(text).map_err(|e| format!("damaged groove pack: {e}"))?;
    let mut grooves = Vec::with_capacity(file.grooves.len());
    for (index, g) in file.grooves.into_iter().enumerate() {
        let damaged = |why: &str| format!("damaged groove pack: groove {} {why}", index + 1);
        if !g.bpm.is_finite() || g.bpm <= 0.0 {
            return Err(damaged("has no usable tempo"));
        }
        if g.bars.is_empty() || g.bars.len() > MAX_GROOVE_BARS {
            return Err(damaged("has no bars or too many"));
        }
        if g.similar_kit.is_some_and(|program| program > 127) {
            return Err(damaged("has a kit out of range"));
        }
        let mut bars = Vec::with_capacity(g.bars.len());
        for bar in g.bars {
            let mut hits = Vec::with_capacity(bar.len());
            for (key, step, velocity) in bar {
                if key > 127 || step >= dm_project::STEPS_PER_BAR || velocity == 0 || velocity > 127
                {
                    return Err(damaged("has a hit out of range"));
                }
                hits.push(BarHit {
                    key,
                    step,
                    velocity,
                });
            }
            bars.push(hits);
        }
        grooves.push(Groove {
            name: g.name,
            folder: g.folder,
            bpm: g.bpm,
            bars,
            kit: g.kit,
            similar_kit: g.similar_kit,
        });
    }
    Ok(Pack {
        name: file.name,
        grooves,
    })
}
