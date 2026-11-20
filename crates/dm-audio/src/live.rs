//! MIDI in: drum hits played on a MIDI input, and where in the beat the player heard them.
//!
//! The MIDI input's thread (in the `dm-midi` crate) pushes each hit into a queue whose sending end
//! [`crate::Engine::take_live_hits`] hands out. The audio thread plays each hit at the start of
//! its next block, and reports it back to the interface with the position of the beat the player
//! was hearing when they hit; see [`crate::Engine::take_heard_hits`]. The interface records from
//! those reports.
//!
//! See "Milestone 14: MIDI in" in `docs/DESIGN.md`.

use std::time::Instant;

/// How many hits each of the two queues holds.
pub const LIVE_QUEUE_CAPACITY: usize = 256;

/// One hit from the MIDI input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveHit {
    /// When the player hit the key, on this process's clock.
    pub at: Instant,
    /// The General MIDI drum key, 35..=81.
    pub key: u8,
    /// The velocity, 1..=127.
    pub velocity: u8,
    /// Whether the app should sound the hit. False when the input is the same device as the MIDI
    /// output, because that device already plays its own keys.
    pub sound: bool,
}

/// The sending end of the live hit queue, for the MIDI input's thread.
pub type LiveHits = rtrb::Producer<LiveHit>;

/// Where in the beat a hit was heard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeardPosition {
    /// The pattern slot that was sounding.
    pub pattern: usize,
    /// In song mode, the index of the song entry; `None` in pattern mode.
    pub song_index: Option<usize>,
    /// The position within the pattern, in steps from its start, in straight time: step `n`
    /// starts at `n as f64` as if there were no swing. From 0 up to the pattern's length.
    pub step: f64,
}

/// A live hit, as the audio thread played it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeardHit {
    /// The General MIDI drum key.
    pub key: u8,
    /// The velocity the player hit with, before any row volume.
    pub velocity: u8,
    /// What the player was hearing at the moment of the hit, or `None` when the project was not
    /// playing (stopped, or the preview was playing).
    pub position: Option<HeardPosition>,
}
