//! Sends a style file to a Yamaha arranger keyboard's User drive over MIDI, with the file transfer
//! messages described in `docs/research/arranger-style-export.md`, "Sending a style over MIDI".
//! Tried on the PSR-SX720 over Bluetooth MIDI.
//!
//! `cargo run -p dm-midi --example style_send -- ports`
//! lists the MIDI inputs and outputs.
//!
//! `cargo run -p dm-midi --example style_send -- probe <port>`
//! asks for the model and the drives and lists the User drive's `STYLE` folder. It changes nothing.
//!
//! `cargo run -p dm-midi --example style_send -- send <port> <file.sty>`
//! writes the file into the User drive's `STYLE` folder. If the name is taken, it adds `_1`, `_2`
//! and so on, as StylePlayer.io does.
//!
//! `<port>` is part of the name of both the input and the output that reach the keyboard. Add
//! `--verbose` to print every message sent and received.

#[cfg(target_os = "android")]
fn main() {
    eprintln!("MIDI is not available on Android.");
}

#[cfg(not(target_os = "android"))]
fn main() {
    if let Err(e) = transfer::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "android"))]
mod transfer {
    use std::sync::mpsc::{Receiver, channel};
    use std::time::Duration;

    use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

    /// How long to wait for each reply.
    const TIMEOUT: Duration = Duration::from_secs(3);

    /// File bytes in one data message, before packing.
    const BLOCK: usize = 210;

    /// The folder on the User drive that holds styles.
    const STYLE_FOLDER: &str = "STYLE";

    type Result<T> = std::result::Result<T, String>;

    const USAGE: &str =
        "usage: style_send ports | probe <port> | send <port> <file.sty> [--verbose]";

    pub fn run() -> Result<()> {
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        let verbose = args.iter().any(|a| a == "--verbose");
        args.retain(|a| a != "--verbose");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["ports"] => ports(),
            ["probe", port] => {
                let mut k = Keyboard::open(port, verbose)?;
                let drive = k.start()?;
                let names = k.list(&format!("{drive}:\\{STYLE_FOLDER}\\*.*"));
                k.stop()?;
                println!("{STYLE_FOLDER}: {:?}", names?);
                Ok(())
            }
            ["send", port, file] => {
                let data = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
                let name = std::path::Path::new(file)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .ok_or_else(|| format!("{file}: no file name"))?;
                let mut k = Keyboard::open(port, verbose)?;
                let drive = k.start()?;
                let saved = k.send_file(drive, &name, &data);
                k.stop()?;
                println!("saved as USER:\\{STYLE_FOLDER}\\{}", saved?);
                Ok(())
            }
            _ => Err(USAGE.into()),
        }
    }

    fn ports() -> Result<()> {
        let input = MidiInput::new("style_send").map_err(|e| e.to_string())?;
        for p in input.ports() {
            println!("in:  {}", input.port_name(&p).unwrap_or_default());
        }
        let output = MidiOutput::new("style_send").map_err(|e| e.to_string())?;
        for p in output.ports() {
            println!("out: {}", output.port_name(&p).unwrap_or_default());
        }
        Ok(())
    }

    /// Parses bytes written in hex, separated by spaces.
    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|x| u8::from_str_radix(x, 16).expect("valid hex"))
            .collect()
    }

    fn show(d: &[u8]) -> String {
        d.iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A number as 7-bit bytes, most significant first, padded to `width` bytes.
    fn number(mut v: usize, width: usize) -> Vec<u8> {
        let mut r = Vec::new();
        while v > 0 {
            r.insert(0, (v & 0x7f) as u8);
            v >>= 7;
        }
        while r.len() < width {
            r.insert(0, 0);
        }
        r
    }

    /// Packs 8-bit data into 7-bit bytes: each group of up to 7 bytes becomes one byte of high
    /// bits (bit 6 for the first byte) and the 7 low-bit bytes.
    fn pack(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for group in data.chunks(7) {
            let mut high = 0u8;
            for (i, &b) in group.iter().enumerate() {
                if b & 0x80 != 0 {
                    high |= 1 << (6 - i);
                }
            }
            out.push(high);
            out.extend(group.iter().map(|b| b & 0x7f));
        }
        out
    }

    /// Unpacks what [`pack`] made. The keyboard's replies hold whole groups of 7 in practice.
    fn unpack(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for group in data.chunks(8) {
            let n = group.len();
            for (i, &b) in group.iter().enumerate().skip(1) {
                let bit = n - 1 - i;
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
    fn text(d: &[u8]) -> String {
        d.iter()
            .take_while(|&&b| b != 0)
            .map(|&b| b as char)
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

    /// Whether a reply starts with `head` (hex) and ends with F7.
    fn starts(reply: &[u8], head: &str) -> bool {
        let head = hex(head);
        reply.len() > head.len() && reply.starts_with(&head) && reply.last() == Some(&0xf7)
    }

    /// The name StylePlayer.io would give a file in a folder that already holds `taken`.
    fn free_name(name: &str, taken: &[String]) -> String {
        let (stem, ext) = match name.find('.') {
            Some(i) => name.split_at(i),
            None => (name, ""),
        };
        let mut candidate = name.to_string();
        let mut n = 1;
        while taken.contains(&candidate) {
            candidate = format!("{stem}_{n}{ext}");
            n += 1;
        }
        candidate
    }

    struct Keyboard {
        out: MidiOutputConnection,
        replies: Receiver<Vec<u8>>,
        _input: MidiInputConnection<()>,
        verbose: bool,
    }

    impl Keyboard {
        fn open(port: &str, verbose: bool) -> Result<Keyboard> {
            let mut input = MidiInput::new("style_send").map_err(|e| e.to_string())?;
            input.ignore(Ignore::None);
            let in_port = input
                .ports()
                .into_iter()
                .find(|p| input.port_name(p).unwrap_or_default().contains(port))
                .ok_or_else(|| format!("no MIDI input named like {port:?}"))?;
            let (tx, replies) = channel();
            // Only SysEx replies matter; active sensing, clock and channel messages are dropped.
            let input = input
                .connect(
                    &in_port,
                    "style_send",
                    move |_, msg, _| {
                        if msg.first() == Some(&0xf0) {
                            let _ = tx.send(msg.to_vec());
                        }
                    },
                    (),
                )
                .map_err(|e| e.to_string())?;
            let output = MidiOutput::new("style_send").map_err(|e| e.to_string())?;
            let out_port = output
                .ports()
                .into_iter()
                .find(|p| output.port_name(p).unwrap_or_default().contains(port))
                .ok_or_else(|| format!("no MIDI output named like {port:?}"))?;
            let out = output
                .connect(&out_port, "style_send")
                .map_err(|e| e.to_string())?;
            Ok(Keyboard {
                out,
                replies,
                _input: input,
                verbose,
            })
        }

        /// Sends one message and returns the reply. With `expect`, the reply must equal it (hex).
        fn request(&mut self, msg: &[u8], expect: Option<&str>) -> Result<Vec<u8>> {
            while self.replies.try_recv().is_ok() {}
            if self.verbose {
                println!(">> {}", show(msg));
            }
            self.out.send(msg).map_err(|e| e.to_string())?;
            let reply = self
                .replies
                .recv_timeout(TIMEOUT)
                .map_err(|_| format!("no reply to {}", show(&msg[..msg.len().min(12)])))?;
            if self.verbose {
                println!("<< {}", show(&reply));
            }
            if let Some(expect) = expect
                && reply != hex(expect)
            {
                return Err(format!(
                    "unexpected reply {}, wanted {expect}",
                    show(&reply)
                ));
            }
            Ok(reply)
        }

        /// Sends a request whose reply must start with `head`.
        fn query(&mut self, msg: &str, head: &str) -> Result<Vec<u8>> {
            let reply = self.request(&hex(msg), None)?;
            if !starts(&reply, head) {
                return Err(format!(
                    "unexpected reply {}, wanted {head} ...",
                    show(&reply)
                ));
            }
            Ok(reply)
        }

        /// Starts a transfer session and returns the number of the User drive.
        fn start(&mut self) -> Result<u8> {
            let r = self.query("F0 43 50 00 00 07 01 F7", "F0 43 50 00 00 07 02")?;
            println!("model: {}", text(&unpack(&r[8..r.len() - 1])));
            let r = self.query("F0 43 50 00 00 00 01 F7", "F0 43 50 00 00 00 02 01")?;
            let drives = r[8];
            self.request(&hex("F0 43 50 00 00 02 01 F7"), None)?;
            self.request(
                &hex("F0 43 50 00 00 01 01 F7"),
                Some("F0 43 50 00 00 01 02 00 F7"),
            )?;
            self.request(
                &hex("F0 43 50 00 00 01 00 01 F7"),
                Some("F0 43 50 00 00 01 02 01 F7"),
            )?;
            self.request(&hex("F0 43 50 00 05 0B 00 00 F7"), None)?;
            self.request(&hex("F0 43 50 00 05 06 00 00 F7"), None)?;
            for drive in 0..drives {
                let r = self.query(
                    &format!("F0 43 50 00 05 0B 00 {drive:02X} F7"),
                    "F0 43 50 00 05 0B 01",
                )?;
                let name = text(&unpack(&r[11..r.len() - 1]));
                println!("drive {drive}: {name}");
                if name == "USER" {
                    return Ok(drive);
                }
            }
            self.stop()?;
            Err("the keyboard has no USER drive".into())
        }

        /// Ends the transfer session.
        fn stop(&mut self) -> Result<()> {
            self.request(
                &hex("F0 43 50 00 00 01 00 00 F7"),
                Some("F0 43 50 00 00 01 02 00 F7"),
            )?;
            Ok(())
        }

        /// Lists the names matching a path such as `0:\STYLE\*.*`.
        fn list(&mut self, path: &str) -> Result<Vec<String>> {
            let mut msg = hex("F0 43 50 00 05 04 00 3F");
            msg.extend(packed_text(path));
            msg.push(0xf7);
            let next = hex("F0 43 50 00 05 05 00 F7");
            let mut names = Vec::new();
            let mut reply = self.request(&msg, None)?;
            // A short reply ends the list. In an entry, byte 22 is the length of a field before
            // the 2-byte length of the packed name.
            while starts(&reply, "F0 43 50 00 05") && reply.len() > 27 {
                let start = (23 + reply[22] as usize + 2).min(reply.len() - 1);
                names.push(text(&unpack(&reply[start..reply.len() - 1])));
                reply = self.request(&next, None)?;
            }
            Ok(names)
        }

        /// Writes `data` into the User drive's style folder and returns the name used.
        fn send_file(&mut self, drive: u8, name: &str, data: &[u8]) -> Result<String> {
            let taken = self.list(&format!("{drive}:\\{STYLE_FOLDER}\\*.*"))?;
            let name = free_name(name, &taken);
            let path = format!("{drive}:\\{STYLE_FOLDER}\\{name}");
            println!("writing {path}, {} bytes", data.len());

            let done = "F0 43 50 00 03 00 F7";
            let mut open = hex("F0 43 50 00 06 01 00 05");
            open.extend(number(data.len(), 5));
            open.extend(hex("05 00 00 00 00 00"));
            open.extend(packed_text(&path));
            open.push(0xf7);
            self.request(&open, Some(done))?;

            for (i, chunk) in data.chunks(BLOCK).enumerate() {
                let mut block = vec![0x02, 0x02, (i % 128) as u8];
                block.extend(pack(chunk));
                let sum: usize = block.iter().map(|&b| b as usize).sum();
                let checksum = ((128 - sum % 128) % 128) as u8;
                let mut msg = hex("F0 43 50 01");
                msg.extend(number(block.len(), 2));
                msg.extend(block);
                msg.push(checksum);
                msg.push(0xf7);
                self.request(&msg, Some(done))?;
            }
            self.request(&hex("F0 43 50 00 03 02 F7"), Some("F0 43 50 00 03 03 F7"))?;
            Ok(name)
        }
    }
}
