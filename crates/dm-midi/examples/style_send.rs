//! Sends a style file to a Yamaha arranger keyboard's User drive over MIDI, with the file transfer
//! messages described in `docs/research/arranger-style-export.md`, "Sending a style over MIDI".
//! The messages and steps are the library's (`dm_midi::transfer::yamaha`). Tried on the PSR-SX720
//! over Bluetooth MIDI.
//!
//! `cargo run -p dm-midi --example style_send -- ports`
//! lists the MIDI inputs and outputs.
//!
//! `cargo run -p dm-midi --example style_send -- probe <port>`
//! asks for the model and the drives and lists the User drive's `STYLE` folder. It changes nothing.
//!
//! `cargo run -p dm-midi --example style_send -- send <port> <file.sty>`
//! writes the file into the User drive's `STYLE` folder through `MidiOut::send_style`, as the app
//! does. A file of the same name is replaced.
//!
//! `cargo run -p dm-midi --example style_send -- watch <port> [seconds]`
//! chooses the output as the app does and prints each change of its status, for checking that a
//! keyboard switched off and on again is noticed. The default is 120 seconds.
//!
//! `<port>` is part of the name of both the input and the output that reach the keyboard. Add
//! `--verbose` to print every message sent and received.

#[cfg(target_os = "android")]
fn main() {
    eprintln!("MIDI is not available on Android.");
}

#[cfg(not(target_os = "android"))]
fn main() {
    if let Err(e) = run::run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "android"))]
mod run {
    use std::sync::mpsc::{Receiver, channel};
    use std::time::{Duration, Instant};

    use dm_midi::transfer::{Fault, Link, REPLY_TIMEOUT, show, yamaha};
    use dm_midi::{KeyboardTarget, MidiOut, MidiStatus, TransferStatus};
    use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

    type Result<T> = std::result::Result<T, String>;

    const USAGE: &str = "usage: style_send ports | probe <port> | send <port> <file.sty> \
                         | watch <port> [seconds] [--verbose]";

    pub fn run() -> Result<()> {
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        let verbose = args.iter().any(|a| a == "--verbose");
        args.retain(|a| a != "--verbose");
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["ports"] => ports(),
            ["probe", port] => probe(port, verbose),
            ["send", port, file] => send(port, file, verbose),
            ["watch", port] => watch(port, 120),
            ["watch", port, seconds] => watch(port, seconds.parse().map_err(|_| USAGE)?),
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

    fn probe(port: &str, verbose: bool) -> Result<()> {
        let mut keyboard = Keyboard::open(port, verbose)?;
        let found = yamaha::probe(&mut keyboard).map_err(|e| format!("{e:?}"))?;
        println!("model: {}", found.model);
        for (i, drive) in found.drives.iter().enumerate() {
            println!("drive {i}: {drive}");
        }
        let names: Vec<String> = found
            .styles
            .iter()
            .map(|e| {
                if e.folder {
                    format!("{}\\", e.name)
                } else {
                    e.name.clone()
                }
            })
            .collect();
        println!("{}: {names:?}", yamaha::STYLE_FOLDER);
        Ok(())
    }

    /// Starts the sender and chooses the output whose name contains `port`.
    fn choose(port: &str) -> Result<(MidiOut, String)> {
        let (_producer, consumer) = rtrb::RingBuffer::new(16);
        let mut midi = MidiOut::start(consumer);
        let output = midi
            .outputs()
            .into_iter()
            .find(|n| n.contains(port))
            .ok_or_else(|| format!("no MIDI output named like {port:?}"))?;
        midi.select(Some(output.clone()));
        Ok((midi, output))
    }

    fn watch(port: &str, seconds: u64) -> Result<()> {
        log::set_logger(&PRINT).map_err(|e| e.to_string())?;
        log::set_max_level(log::LevelFilter::Debug);
        let (midi, output) = choose(port)?;
        println!("watching {output} for {seconds} s");
        let started = Instant::now();
        let mut last = None;
        while started.elapsed() < Duration::from_secs(seconds) {
            let status = midi.status();
            if last.as_ref() != Some(&status) {
                println!("{:6.1} s  {status:?}", started.elapsed().as_secs_f32());
                last = Some(status);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }

    fn send(port: &str, file: &str, verbose: bool) -> Result<()> {
        if verbose {
            log::set_logger(&PRINT).map_err(|e| e.to_string())?;
            log::set_max_level(log::LevelFilter::Trace);
        }
        let data = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        let name = std::path::Path::new(file)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| format!("{file}: no file name"))?;

        let (mut midi, output) = choose(port)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while midi.status() != MidiStatus::Connected {
            if Instant::now() > deadline {
                return Err(format!("{output}: {:?}", midi.status()));
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        println!("writing {name}, {} bytes, to {output}", data.len());
        let started = Instant::now();
        midi.send_style(KeyboardTarget::Yamaha, name, data);
        let mut shown = -1;
        loop {
            match midi.transfer() {
                TransferStatus::Idle => {}
                TransferStatus::Running(share) => {
                    let percent = (share * 100.0) as i32;
                    if percent / 10 != shown / 10 {
                        println!("{percent}%");
                        shown = percent;
                    }
                }
                TransferStatus::Done(saved) => {
                    println!(
                        "saved as USER:\\{}\\{saved} in {:.1} s",
                        yamaha::STYLE_FOLDER,
                        started.elapsed().as_secs_f32()
                    );
                    return Ok(());
                }
                TransferStatus::Failed(e) => return Err(format!("{e:?}")),
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Prints the library's log, including each message of the transfer.
    struct Print;

    static PRINT: Print = Print;

    impl log::Log for Print {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }

        fn log(&self, record: &log::Record) {
            if record.target().starts_with("dm_midi") {
                println!("{}", record.args());
            }
        }

        fn flush(&self) {}
    }

    /// The keyboard's input and output, opened directly, for `probe`.
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
    }

    impl Link for Keyboard {
        fn request(&mut self, message: &[u8]) -> std::result::Result<Vec<u8>, Fault> {
            while self.replies.try_recv().is_ok() {}
            if self.verbose {
                println!(">> {}", show(message));
            }
            self.out
                .send(message)
                .map_err(|e| Fault::SendFailed(e.to_string()))?;
            let reply = self
                .replies
                .recv_timeout(REPLY_TIMEOUT)
                .map_err(|_| Fault::NoReply)?;
            if self.verbose {
                println!("<< {}", show(&reply));
            }
            Ok(reply)
        }
    }
}
