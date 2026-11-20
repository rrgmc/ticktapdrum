//! Maps a file's drum keys to the General MIDI keys the app's rows use.

/// Which key layout a file uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMap {
    /// Guess: TD-11 if the file uses keys only that kit sends (22, 26), else General MIDI.
    Auto,
    /// General MIDI, with GS extras kept.
    Gm,
    /// The Roland TD-11 layout that the Groove MIDI Dataset was recorded with.
    Td11,
}

impl KeyMap {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "auto" => Ok(Self::Auto),
            "gm" => Ok(Self::Gm),
            "td11" => Ok(Self::Td11),
            _ => Err(format!("unknown key map {s}")),
        }
    }

    pub fn resolve(self, keys: impl Iterator<Item = u8>) -> Self {
        match self {
            Self::Auto => {
                if keys.into_iter().any(|k| k == 22 || k == 26) {
                    Self::Td11
                } else {
                    Self::Gm
                }
            }
            other => other,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Gm => "gm",
            Self::Td11 => "td11",
        }
    }

    /// The app key for a file key, or `None` if the app has no row for it.
    pub fn map(self, key: u8) -> Option<u8> {
        match self {
            Self::Td11 => td11(key),
            _ => gm(key),
        }
    }
}

/// General MIDI 35 to 81, plus the Roland GS extras 27 to 34 and 82 to 87 that GS banks hold.
fn gm(key: u8) -> Option<u8> {
    (27..=87).contains(&key).then_some(key)
}

/// Whether a key is outside General MIDI's 35 to 81, so it plays only on a GS bank.
pub fn is_gs_extra(key: u8) -> bool {
    (27..=34).contains(&key) || (82..=87).contains(&key)
}

/// The TD-11 layout from the Groove MIDI Dataset's documentation, mapped to the General MIDI
/// sound that matches each pad zone. Keys the TD-11 shares with General MIDI and means the same
/// pass through.
fn td11(key: u8) -> Option<u8> {
    Some(match key {
        36 => 36, // kick
        38 => 38, // snare head
        40 => 38, // snare rim: a rimshot, not GM's electric snare
        37 => 37, // snare cross-stick
        48 => 50, // tom 1 head (high)
        50 => 50, // tom 1 rim
        45 => 47, // tom 2 head (mid)
        47 => 47, // tom 2 rim
        43 => 43, // tom 3 head (floor)
        58 => 43, // tom 3 rim: GM 58 is the vibraslap
        46 => 46, // hi-hat open, bow
        26 => 46, // hi-hat open, edge
        42 => 42, // hi-hat closed, bow
        22 => 42, // hi-hat closed, edge
        44 => 44, // hi-hat pedal
        49 => 49, // crash 1 bow
        55 => 49, // crash 1 edge: GM 55 is the splash
        57 => 57, // crash 2 bow
        52 => 57, // crash 2 edge: GM 52 is the china
        51 => 51, // ride bow
        59 => 51, // ride edge
        53 => 53, // ride bell
        other => return gm(other),
    })
}
