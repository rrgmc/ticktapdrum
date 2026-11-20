//! Yamaha's file transfer over MIDI: the encoding, the messages, the replies, and the steps that
//! write a style into an arranger keyboard's User drive.
//!
//! The messages come from StylePlayer.io's page script; Yamaha does not document them. See
//! "Sending a style over MIDI" in `docs/research/arranger-style-export.md`. Tried on the
//! PSR-SX720.

use super::{Fault, Link, show};
use crate::TransferError;

/// File bytes in one data message, before packing.
pub const BLOCK: usize = 210;

/// The folder on the User drive that holds styles.
pub const STYLE_FOLDER: &str = "STYLE";

/// The name of the drive files go to.
const USER_DRIVE: &str = "USER";

/// The most list entries read from one folder, so that a keyboard that never ends the list
/// cannot keep the transfer going for ever.
const MAX_ENTRIES: usize = 4096;

// The fixed messages and replies.
const MODEL: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x07, 0x01, 0xF7];
const MODEL_REPLY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x07, 0x02];
const START: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x00, 0x01, 0xF7];
const START_REPLY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x00, 0x02, 0x01];
const STATE: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x02, 0x01, 0xF7];
const RESET: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x01, 0x01, 0xF7];
const CONNECT_ON: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x01, 0x00, 0x01, 0xF7];
const CONNECT_ON_REPLY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x01, 0x02, 0x01, 0xF7];
/// Ends the session. The reset message gets the same reply.
const CONNECT_OFF: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x01, 0x00, 0x00, 0xF7];
const CONNECT_OFF_REPLY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x00, 0x01, 0x02, 0x00, 0xF7];
const DRIVES: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x05, 0x06, 0x00, 0x00, 0xF7];
const DRIVE_REPLY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x05, 0x0B, 0x01];
const LIST_REPLY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x05];
const LIST_NEXT: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x05, 0x05, 0x00, 0xF7];
const DELETE_REPLY: &[u8] = &[
    0xF0, 0x43, 0x50, 0x00, 0x05, 0x7F, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0xF7,
];
/// The reply to opening a file and to each data message.
const READY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x03, 0x00, 0xF7];
const CLOSE: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x03, 0x02, 0xF7];
const CLOSE_REPLY: &[u8] = &[0xF0, 0x43, 0x50, 0x00, 0x03, 0x03, 0xF7];

/// A number as 7-bit bytes, most significant first, padded to `width` bytes.
pub(crate) fn number(mut value: usize, width: usize) -> Vec<u8> {
    let mut out = Vec::new();
    while value > 0 {
        out.insert(0, (value & 0x7F) as u8);
        value >>= 7;
    }
    while out.len() < width {
        out.insert(0, 0);
    }
    out
}

/// Packs 8-bit data into 7-bit bytes: each group of up to 7 bytes becomes one byte of high bits
/// (bit 6 for the first byte of the group) and the 7 low-bit bytes.
pub(crate) fn pack(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len().div_ceil(7));
    for group in data.chunks(7) {
        let mut high = 0u8;
        for (i, &b) in group.iter().enumerate() {
            if b & 0x80 != 0 {
                high |= 1 << (6 - i);
            }
        }
        out.push(high);
        out.extend(group.iter().map(|b| b & 0x7F));
    }
    out
}

/// Undoes [`pack`]. The keyboard's replies hold whole groups of 7 in practice; a shorter last
/// group is read as [`pack`] writes it.
pub(crate) fn unpack(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for group in data.chunks(8) {
        for (i, &b) in group.iter().enumerate().skip(1) {
            let bit = 7 - i;
            out.push(if group[0] & (1 << bit) != 0 {
                b | 0x80
            } else {
                b
            });
        }
    }
    out
}

/// The text up to the first zero byte.
fn text(data: &[u8]) -> String {
    data.iter()
        .take_while(|&&b| b != 0)
        .map(|&b| char::from(b))
        .collect()
}

/// A text with its ending zero, packed, after its packed length in 2 bytes.
fn packed_text(s: &str) -> Vec<u8> {
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0);
    let packed = pack(&bytes);
    let mut out = number(packed.len(), 2);
    out.extend(packed);
    out
}

/// A message that starts with `head`, then the path as [`packed_text`], then `F7`.
fn with_path(head: &[u8], path: &str) -> Vec<u8> {
    let mut message = head.to_vec();
    message.extend(packed_text(path));
    message.push(0xF7);
    message
}

/// Asks for the name of drive `drive`.
fn drive_name(drive: u8) -> Vec<u8> {
    vec![0xF0, 0x43, 0x50, 0x00, 0x05, 0x0B, 0x00, drive & 0x7F, 0xF7]
}

/// Asks for the first entry matching a path such as `0:\STYLE\*.*`.
fn list(pattern: &str) -> Vec<u8> {
    with_path(&[0xF0, 0x43, 0x50, 0x00, 0x05, 0x04, 0x00, 0x3F], pattern)
}

/// Deletes the file at a path such as `0:\STYLE\Name.sty`.
fn delete(path: &str) -> Vec<u8> {
    with_path(&[0xF0, 0x43, 0x50, 0x00, 0x05, 0x01, 0x00, 0x00], path)
}

/// Opens the file at `path` for writing `size` bytes.
fn open(path: &str, size: usize) -> Vec<u8> {
    let mut head = vec![0xF0, 0x43, 0x50, 0x00, 0x06, 0x01, 0x00, 0x05];
    head.extend(number(size, 5));
    head.extend([0x05, 0x00, 0x00, 0x00, 0x00, 0x00]);
    with_path(&head, path)
}

/// Data message number `index` (counting from 0), holding up to [`BLOCK`] bytes of the file.
fn data(index: usize, chunk: &[u8]) -> Vec<u8> {
    let mut block = vec![0x02, 0x02, (index % 128) as u8];
    block.extend(pack(chunk));
    let sum: usize = block.iter().map(|&b| usize::from(b)).sum();
    let checksum = ((128 - sum % 128) % 128) as u8;
    let mut message = vec![0xF0, 0x43, 0x50, 0x01];
    message.extend(number(block.len(), 2));
    message.extend(block);
    message.push(checksum);
    message.push(0xF7);
    message
}

/// Whether `reply` starts with `head`, has something after it, and ends with `F7`.
fn starts(reply: &[u8], head: &[u8]) -> bool {
    reply.len() > head.len() && reply.starts_with(head) && reply.last() == Some(&0xF7)
}

/// The model name in a reply to [`MODEL`]: the header, a length byte, the packed name, `F7`.
fn parse_model(reply: &[u8]) -> Option<String> {
    if !starts(reply, MODEL_REPLY) || reply.len() < 10 {
        return None;
    }
    let name = text(&unpack(&reply[8..reply.len() - 1]));
    (!name.is_empty()).then_some(name)
}

/// The number of drives in a reply to [`START`].
fn parse_drive_count(reply: &[u8]) -> Option<u8> {
    if starts(reply, START_REPLY) && reply.len() > 9 {
        Some(reply[8])
    } else {
        None
    }
}

/// The drive name in a reply to [`drive_name`]: the header, 4 bytes, the packed name, `F7`.
fn parse_drive_name(reply: &[u8]) -> Option<String> {
    if starts(reply, DRIVE_REPLY) && reply.len() > 12 {
        Some(text(&unpack(&reply[11..reply.len() - 1])))
    } else {
        None
    }
}

/// One entry of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub folder: bool,
}

/// The entry in a reply to [`list`] or [`LIST_NEXT`], or `None` when the reply ends the list.
///
/// An entry is the header, an attribute byte (`0x10` for a folder), 14 bytes of date, a byte
/// `n`, `n` bytes (the size), the packed name's length in 2 bytes, the packed name and `F7`. The
/// list ends with a short reply, `F0 43 50 00 05 7F ...` on the SX720.
fn parse_entry(reply: &[u8]) -> Option<Entry> {
    if !starts(reply, LIST_REPLY) || reply.len() <= 27 || reply[5] == 0x7F {
        return None;
    }
    let start = 23 + usize::from(reply[22]) + 2;
    if start >= reply.len() {
        return None;
    }
    Some(Entry {
        name: text(&unpack(&reply[start..reply.len() - 1])),
        folder: reply[7] & 0x10 != 0,
    })
}

/// Whether a file name can go on the keyboard as it is: plain ASCII, no path, not empty.
fn usable_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| (0x20..0x7F).contains(&b) && !b"\\/:*?\"<>|".contains(&b))
}

/// What went wrong with one step: the link's fault or a reply that was not the one expected.
enum Step {
    Fault(Fault),
    Unexpected(String),
}

impl From<Fault> for Step {
    fn from(fault: Fault) -> Self {
        Step::Fault(fault)
    }
}

/// The error for a step that failed once the session is under way.
fn session_error(step: Step) -> TransferError {
    match step {
        Step::Fault(Fault::NoReply) => TransferError::NoAnswer,
        Step::Fault(Fault::SendFailed(_) | Fault::Cancelled) => TransferError::NotConnected,
        Step::Unexpected(detail) => TransferError::Refused(detail),
    }
}

/// Sends `message` and checks that the reply is exactly `want`.
fn expect(link: &mut impl Link, message: &[u8], want: &[u8]) -> Result<(), Step> {
    let reply = link.request(message)?;
    if reply == want {
        Ok(())
    } else {
        Err(Step::Unexpected(format!(
            "the keyboard answered {} to {}",
            show(&reply),
            show(&message[..message.len().min(12)])
        )))
    }
}

/// Asks for the model. Any well-formed answer will do.
fn ask_model(link: &mut impl Link) -> Result<String, TransferError> {
    let reply = link.request(MODEL).map_err(|fault| match fault {
        Fault::NoReply => TransferError::NoAnswer,
        Fault::SendFailed(_) | Fault::Cancelled => TransferError::NotConnected,
    })?;
    parse_model(&reply).ok_or_else(|| TransferError::UnknownKeyboard(show(&reply)))
}

/// Starts the session, up to and including "connect on", and returns the number of drives.
/// Failures here mean the keyboard does not take files this way, so they are
/// [`TransferError::UnknownKeyboard`] with the model; only a lost connection is reported as such.
/// `connect_sent` is set once "connect on" has gone out.
fn begin(link: &mut impl Link, model: &str, connect_sent: &mut bool) -> Result<u8, TransferError> {
    let not_this_way = |step: Step| match step {
        Step::Fault(Fault::SendFailed(_) | Fault::Cancelled) => TransferError::NotConnected,
        _ => TransferError::UnknownKeyboard(model.to_owned()),
    };
    let reply = link.request(START).map_err(|f| not_this_way(f.into()))?;
    let drives =
        parse_drive_count(&reply).ok_or_else(|| not_this_way(Step::Unexpected(String::new())))?;
    link.request(STATE).map_err(|f| not_this_way(f.into()))?;
    expect(link, RESET, CONNECT_OFF_REPLY).map_err(not_this_way)?;
    *connect_sent = true;
    expect(link, CONNECT_ON, CONNECT_ON_REPLY).map_err(not_this_way)?;
    Ok(drives)
}

/// Asks for the model, then runs `work` inside a session and returns the model with its
/// result. Ends the session whenever "connect on" was sent, even after a failure.
fn session<L: Link, T>(
    link: &mut L,
    work: impl FnOnce(&mut L, u8) -> Result<T, Step>,
) -> Result<(String, T), TransferError> {
    let model = ask_model(link)?;
    log::info!("style transfer: the keyboard is a {model}");
    let mut connect_sent = false;
    let result = match begin(link, &model, &mut connect_sent) {
        Ok(drives) => work(link, drives).map_err(session_error),
        Err(e) => Err(e),
    };
    if connect_sent {
        end(link);
    }
    result.map(|value| (model, value))
}

/// Finds the User drive among `drives` and returns its number.
fn user_drive(link: &mut impl Link, drives: u8) -> Result<u8, Step> {
    link.request(&drive_name(0))?;
    link.request(DRIVES)?;
    for drive in 0..drives {
        let reply = link.request(&drive_name(drive))?;
        let name = parse_drive_name(&reply)
            .ok_or_else(|| Step::Unexpected(format!("drive {drive} answered {}", show(&reply))))?;
        if name == USER_DRIVE {
            return Ok(drive);
        }
    }
    Err(Step::Unexpected(
        "the keyboard has no USER drive".to_owned(),
    ))
}

/// Lists the entries matching a path such as `0:\STYLE\*.*`.
fn list_folder(link: &mut impl Link, pattern: &str) -> Result<Vec<Entry>, Step> {
    let mut entries = Vec::new();
    let mut reply = link.request(&list(pattern))?;
    while let Some(entry) = parse_entry(&reply) {
        entries.push(entry);
        if entries.len() >= MAX_ENTRIES {
            break;
        }
        reply = link.request(LIST_NEXT)?;
    }
    Ok(entries)
}

/// Whether a file of this name is in the list. The keyboard's drive ignores case.
fn holds(entries: &[Entry], name: &str) -> bool {
    entries
        .iter()
        .any(|e| !e.folder && e.name.eq_ignore_ascii_case(name))
}

/// Ends the session. Called whenever "connect on" was sent, even after a failure.
fn end(link: &mut impl Link) {
    match link.request(CONNECT_OFF) {
        Ok(reply) if reply == CONNECT_OFF_REPLY => {}
        Ok(reply) => log::warn!("style transfer: connect off answered {}", show(&reply)),
        Err(fault) => log::warn!("style transfer: connect off failed: {fault:?}"),
    }
}

/// Writes a style into the User drive's `STYLE` folder, replacing a file of the same name, and
/// returns the name saved. `progress` hears the share of data blocks sent, from 0 to 1.
pub fn send_style(
    link: &mut impl Link,
    name: &str,
    file: &[u8],
    progress: &mut dyn FnMut(f32),
) -> Result<String, TransferError> {
    if !usable_name(name) {
        return Err(TransferError::Refused(format!(
            "{name:?} cannot be a file name on the keyboard"
        )));
    }
    if file.is_empty() {
        return Err(TransferError::Refused("the file is empty".to_owned()));
    }
    progress(0.0);
    session(link, |link, drives| {
        write(link, drives, name, file, progress)
    })?;
    Ok(name.to_owned())
}

/// The part of [`send_style`] between "connect on" and "connect off".
fn write(
    link: &mut impl Link,
    drives: u8,
    name: &str,
    file: &[u8],
    progress: &mut dyn FnMut(f32),
) -> Result<(), Step> {
    let drive = user_drive(link, drives)?;
    let pattern = format!("{drive}:\\{STYLE_FOLDER}\\*.*");
    let path = format!("{drive}:\\{STYLE_FOLDER}\\{name}");
    if holds(&list_folder(link, &pattern)?, name) {
        log::info!("style transfer: deleting the old {path}");
        let reply = link.request(&delete(&path))?;
        if reply != DELETE_REPLY {
            return Err(Step::Unexpected(format!(
                "the keyboard refused to delete {path}: it answered {}",
                show(&reply)
            )));
        }
        if holds(&list_folder(link, &pattern)?, name) {
            return Err(Step::Unexpected(format!(
                "{path} is still there after deleting it"
            )));
        }
    }

    log::info!("style transfer: writing {path}, {} bytes", file.len());
    expect(link, &open(&path, file.len()), READY)?;
    let blocks = file.len().div_ceil(BLOCK);
    for (i, chunk) in file.chunks(BLOCK).enumerate() {
        expect(link, &data(i, chunk), READY)?;
        progress((i + 1) as f32 / blocks as f32);
    }
    expect(link, CLOSE, CLOSE_REPLY)?;
    Ok(())
}

/// What [`probe`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub model: String,
    /// The drive names, up to and including the User drive.
    pub drives: Vec<String>,
    /// The entries in the User drive's `STYLE` folder.
    pub styles: Vec<Entry>,
}

/// Asks for the model and the drives and lists the User drive's `STYLE` folder. Changes nothing.
pub fn probe(link: &mut impl Link) -> Result<Probe, TransferError> {
    let (model, (drives, styles)) = session(link, look)?;
    Ok(Probe {
        model,
        drives,
        styles,
    })
}

/// The part of [`probe`] between "connect on" and "connect off".
fn look(link: &mut impl Link, count: u8) -> Result<(Vec<String>, Vec<Entry>), Step> {
    let drive = user_drive(link, count)?;
    let mut drives = Vec::new();
    for d in 0..=drive {
        let reply = link.request(&drive_name(d))?;
        drives.push(parse_drive_name(&reply).unwrap_or_default());
    }
    let styles = list_folder(link, &format!("{drive}:\\{STYLE_FOLDER}\\*.*"))?;
    Ok((drives, styles))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses bytes written in hex, separated by spaces.
    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|x| u8::from_str_radix(x, 16).unwrap())
            .collect()
    }

    #[test]
    fn numbers_are_7_bit_most_significant_first() {
        assert_eq!(number(0, 2), vec![0, 0]);
        assert_eq!(number(15, 2), vec![0, 15]);
        assert_eq!(number(541, 2), vec![0x04, 0x1D]);
        assert_eq!(number(5000, 5), vec![0, 0, 0, 0x27, 0x08]);
    }

    #[test]
    fn packing_round_trips_and_matches_the_keyboard() {
        let data: Vec<u8> = (0..=255).collect();
        assert_eq!(unpack(&pack(&data)), data);
        // A path the keyboard accepted, as style_send sent it.
        assert_eq!(
            packed_text("0:\\STYLE\\*.*"),
            hex("00 0F 00 30 3A 5C 53 54 59 4C 00 45 5C 2A 2E 2A 00")
        );
        assert_eq!(
            pack(&[0x80, 0x01, 0xFF]),
            vec![0b101_0000, 0x00, 0x01, 0x7F]
        );
    }

    #[test]
    fn replies_from_the_sx720_parse() {
        let model = hex("F0 43 50 00 00 07 02 0C 00 50 53 52 2D 53 58 37 00 32 30 00 F7");
        assert_eq!(parse_model(&model).as_deref(), Some("PSR-SX720"));
        assert_eq!(parse_model(&hex("F0 43 50 00 00 07 02 F7")), None);
        assert_eq!(
            parse_drive_count(&hex("F0 43 50 00 00 00 02 01 02 F7")),
            Some(2)
        );
        let drive = hex("F0 43 50 00 05 0B 01 42 00 00 05 00 55 53 45 52 F7");
        assert_eq!(parse_drive_name(&drive).as_deref(), Some("USER"));

        let folder = hex(
            "F0 43 50 00 05 05 01 10 32 30 32 34 20 31 20 31 20 30 34 38 33 34 02 40 00 00 0E \
             00 53 74 79 6C 65 50 6C 00 61 79 65 72 00 00 F7",
        );
        assert_eq!(
            parse_entry(&folder),
            Some(Entry {
                name: "StylePlayer".to_owned(),
                folder: true
            })
        );
        let file = hex(
            "F0 43 50 00 05 05 01 00 32 30 32 34 20 31 20 31 20 30 33 34 32 38 02 04 1D 00 12 \
             00 54 54 44 54 65 73 74 00 31 5F 31 2E 73 74 79 00 00 00 F7",
        );
        assert_eq!(
            parse_entry(&file),
            Some(Entry {
                name: "TTDTest1_1.sty".to_owned(),
                folder: false
            })
        );
        assert_eq!(
            parse_entry(&hex("F0 43 50 00 05 7F 01 01 01 42 00 00 F7")),
            None
        );
    }

    #[test]
    fn data_messages_carry_a_checksum_that_makes_the_block_sum_a_multiple_of_128() {
        let message = data(130, &[0x80, 0x10, 0x7F]);
        assert_eq!(&message[..4], &[0xF0, 0x43, 0x50, 0x01]);
        let len = usize::from(message[4]) * 128 + usize::from(message[5]);
        let block = &message[6..6 + len];
        assert_eq!(&block[..3], &[0x02, 0x02, 2]);
        let checksum = message[6 + len];
        let sum: usize =
            block.iter().map(|&b| usize::from(b)).sum::<usize>() + usize::from(checksum);
        assert_eq!(sum % 128, 0);
        assert_eq!(message.last(), Some(&0xF7));
    }

    #[test]
    fn names_with_paths_or_other_characters_are_refused() {
        assert!(usable_name("TTD Agent.sty"));
        for bad in [
            "",
            ".",
            "..",
            "a\\b.sty",
            "a/b.sty",
            "Caf\u{e9}.sty",
            "x?.sty",
        ] {
            assert!(!usable_name(bad), "{bad:?}");
        }
    }

    /// A keyboard that answers like the SX720, with a User drive holding `files` in `STYLE`.
    struct Keyboard {
        files: Vec<String>,
        /// Whether delete works. If not, the reply is a refusal.
        delete_works: bool,
        /// Whether the keyboard answers at all.
        answers: bool,
        /// After this many requests, every request goes unanswered.
        silent_after: Option<usize>,
        sent: Vec<Vec<u8>>,
        /// Entries still to come in the current listing.
        listing: Vec<String>,
        writing: Option<(String, usize)>,
        written: Vec<u8>,
    }

    impl Keyboard {
        fn new(files: &[&str]) -> Self {
            Keyboard {
                files: files.iter().map(|&f| f.to_owned()).collect(),
                delete_works: true,
                answers: true,
                silent_after: None,
                sent: Vec::new(),
                listing: Vec::new(),
                writing: None,
                written: Vec::new(),
            }
        }

        fn entry(head: u8, name: &str) -> Vec<u8> {
            let mut reply = vec![0xF0, 0x43, 0x50, 0x00, 0x05, head, 0x01, 0x00];
            reply.extend(b"2024 1 1 03428");
            reply.extend([0x02, 0x04, 0x1D]);
            reply.extend(packed_text(name));
            reply.push(0xF7);
            reply
        }

        fn next_entry(&mut self, head: u8) -> Vec<u8> {
            if self.listing.is_empty() {
                hex("F0 43 50 00 05 7F 01 01 01 42 00 00 F7")
            } else {
                let name = self.listing.remove(0);
                Self::entry(head, &name)
            }
        }

        fn path_name(message: &[u8], at: usize) -> String {
            let path = text(&unpack(&message[at + 2..message.len() - 1]));
            path.rsplit('\\').next().unwrap().to_owned()
        }

        fn sent_count(&self, message: &[u8]) -> usize {
            self.sent.iter().filter(|m| m.as_slice() == message).count()
        }
    }

    impl Link for Keyboard {
        fn request(&mut self, m: &[u8]) -> Result<Vec<u8>, Fault> {
            self.sent.push(m.to_vec());
            if !self.answers || self.silent_after.is_some_and(|n| self.sent.len() > n) {
                return Err(Fault::NoReply);
            }
            let reply = match m {
                _ if m == MODEL => {
                    hex("F0 43 50 00 00 07 02 0C 00 50 53 52 2D 53 58 37 00 32 30 00 F7")
                }
                _ if m == START => hex("F0 43 50 00 00 00 02 01 02 F7"),
                _ if m == STATE => hex("F0 43 50 00 00 02 02 33 00 F7"),
                _ if m == RESET || m == CONNECT_OFF => CONNECT_OFF_REPLY.to_vec(),
                _ if m == CONNECT_ON => CONNECT_ON_REPLY.to_vec(),
                _ if m == DRIVES => hex("F0 43 50 00 05 06 01 05 03 6F 75 60 00 F7"),
                [0xF0, 0x43, 0x50, 0x00, 0x05, 0x0B, 0x00, d, 0xF7] => {
                    let name = if *d == 0 { "USER" } else { "USB1" };
                    let mut r = hex("F0 43 50 00 05 0B 01 42 00 00");
                    let packed = pack(name.as_bytes());
                    r.push(packed.len() as u8);
                    r.extend(packed);
                    r.push(0xF7);
                    r
                }
                [0xF0, 0x43, 0x50, 0x00, 0x05, 0x04, ..] => {
                    self.listing = vec![".".to_owned(), "..".to_owned()];
                    self.listing.extend(self.files.iter().cloned());
                    self.next_entry(0x04)
                }
                _ if m == LIST_NEXT => self.next_entry(0x05),
                [0xF0, 0x43, 0x50, 0x00, 0x05, 0x01, 0x00, 0x00, ..] => {
                    if self.delete_works {
                        let name = Self::path_name(m, 8);
                        self.files.retain(|f| !f.eq_ignore_ascii_case(&name));
                        DELETE_REPLY.to_vec()
                    } else {
                        hex("F0 43 50 00 05 7F 01 01 01 42 00 00 F7")
                    }
                }
                [0xF0, 0x43, 0x50, 0x00, 0x06, 0x01, ..] => {
                    let name = Self::path_name(m, 19);
                    let size = m[8..13].iter().fold(0, |n, &b| n * 128 + usize::from(b));
                    self.writing = Some((name, size));
                    READY.to_vec()
                }
                [0xF0, 0x43, 0x50, 0x01, ..] => {
                    let len = usize::from(m[4]) * 128 + usize::from(m[5]);
                    self.written.extend(unpack(&m[9..6 + len]));
                    READY.to_vec()
                }
                _ if m == CLOSE => {
                    let (name, size) = self.writing.take().unwrap();
                    assert_eq!(self.written.len(), size);
                    if self.files.iter().any(|f| f.eq_ignore_ascii_case(&name)) {
                        panic!("{name} written over without deleting");
                    }
                    self.files.push(name);
                    CLOSE_REPLY.to_vec()
                }
                _ => panic!("unexpected message {}", show(m)),
            };
            Ok(reply)
        }
    }

    fn file() -> Vec<u8> {
        (0..500u32).map(|i| (i * 37 % 256) as u8).collect()
    }

    #[test]
    fn a_new_style_is_written_and_progress_reaches_one() {
        let mut keyboard = Keyboard::new(&["Other.sty"]);
        let mut seen = Vec::new();
        let saved = send_style(&mut keyboard, "TTD Agent.sty", &file(), &mut |p| {
            seen.push(p)
        });
        assert_eq!(saved, Ok("TTD Agent.sty".to_owned()));
        assert_eq!(keyboard.files, vec!["Other.sty", "TTD Agent.sty"]);
        assert_eq!(keyboard.written, file());
        assert_eq!(seen, vec![0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
        assert_eq!(keyboard.sent.last().unwrap(), CONNECT_OFF);
        assert_eq!(keyboard.sent_count(CONNECT_OFF), 1);
    }

    #[test]
    fn a_style_of_the_same_name_is_deleted_first() {
        let mut keyboard = Keyboard::new(&["ttd agent.STY", "Other.sty"]);
        let saved = send_style(&mut keyboard, "TTD Agent.sty", &file(), &mut |_| {});
        assert_eq!(saved, Ok("TTD Agent.sty".to_owned()));
        assert_eq!(keyboard.files, vec!["Other.sty", "TTD Agent.sty"]);
        let delete = delete("0:\\STYLE\\TTD Agent.sty");
        assert_eq!(keyboard.sent_count(&delete), 1);
    }

    #[test]
    fn a_refused_delete_fails_without_writing_a_copy() {
        let mut keyboard = Keyboard::new(&["TTD Agent.sty"]);
        keyboard.delete_works = false;
        let saved = send_style(&mut keyboard, "TTD Agent.sty", &file(), &mut |_| {});
        assert!(matches!(saved, Err(TransferError::Refused(_))), "{saved:?}");
        assert!(keyboard.written.is_empty());
        assert_eq!(keyboard.sent.last().unwrap(), CONNECT_OFF);
    }

    #[test]
    fn a_keyboard_that_does_not_answer_is_no_answer() {
        let mut keyboard = Keyboard::new(&[]);
        keyboard.answers = false;
        let saved = send_style(&mut keyboard, "TTD Agent.sty", &file(), &mut |_| {});
        assert_eq!(saved, Err(TransferError::NoAnswer));
        // Nothing was started, so there is nothing to end.
        assert_eq!(keyboard.sent, vec![MODEL.to_vec()]);
    }

    #[test]
    fn a_keyboard_that_knows_its_model_but_not_the_transfer_is_unknown() {
        let mut keyboard = Keyboard::new(&[]);
        keyboard.silent_after = Some(1);
        let saved = send_style(&mut keyboard, "TTD Agent.sty", &file(), &mut |_| {});
        assert_eq!(
            saved,
            Err(TransferError::UnknownKeyboard("PSR-SX720".to_owned()))
        );
        // "Connect on" never went out, so there is no "connect off" either.
        assert_eq!(keyboard.sent_count(CONNECT_OFF), 0);
    }

    #[test]
    fn a_failure_after_connect_on_still_ends_the_session() {
        // Model, start, state, reset, connect on, then the drive questions go unanswered.
        let mut keyboard = Keyboard::new(&[]);
        keyboard.silent_after = Some(6);
        let saved = send_style(&mut keyboard, "TTD Agent.sty", &file(), &mut |_| {});
        assert_eq!(saved, Err(TransferError::NoAnswer));
        assert_eq!(keyboard.sent.last().unwrap(), CONNECT_OFF);
    }

    #[test]
    fn probe_lists_the_style_folder() {
        let mut keyboard = Keyboard::new(&["A.sty"]);
        let found = probe(&mut keyboard).unwrap();
        assert_eq!(found.model, "PSR-SX720");
        assert_eq!(found.drives, vec!["USER"]);
        let names: Vec<_> = found.styles.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec![".", "..", "A.sty"]);
        assert!(keyboard.files == vec!["A.sty"] && keyboard.written.is_empty());
    }
}
