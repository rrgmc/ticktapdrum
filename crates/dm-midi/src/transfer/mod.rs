//! Writing a file into a keyboard over MIDI, apart from any MIDI device.
//!
//! Each brand has its own module with its own messages and steps. They all run over a [`Link`],
//! which sends one message and returns the keyboard's reply, so the tests can stand in for a
//! keyboard. [`send_style`] picks the brand's module from the [`KeyboardTarget`].
//!
//! Public only for the `style_send` example; not part of the app's interface.

use std::time::Duration;

use crate::{KeyboardTarget, TransferError};

pub mod yamaha;

/// How long to wait for each reply.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(3);

/// Why a request got no usable reply, as the link sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// Nothing came back within [`REPLY_TIMEOUT`].
    NoReply,
    /// The message could not be sent; the text is the system's reason.
    SendFailed(String),
    /// The transfer was called off, because the app is closing or the output changed.
    Cancelled,
}

/// The connection to the keyboard: its output and the input of the same name.
pub trait Link {
    /// Sends one message and returns the next SysEx message that comes back.
    fn request(&mut self, message: &[u8]) -> Result<Vec<u8>, Fault>;
}

/// Writes a style file into the keyboard where its styles belong, replacing a file of the same
/// name, and returns the name saved. `progress` hears the share sent, from 0 to 1.
pub fn send_style(
    link: &mut impl Link,
    target: KeyboardTarget,
    name: &str,
    file: &[u8],
    progress: &mut dyn FnMut(f32),
) -> Result<String, TransferError> {
    match target {
        KeyboardTarget::Yamaha => yamaha::send_style(link, name, file, progress),
    }
}

/// Bytes as hex, for the log.
pub fn show(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
