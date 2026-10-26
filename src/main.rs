use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use clm_itn::{Normalizer, Setting};

const ABOUT: &str = "Usage: clm-itn [--no-<setting>]... [--help | --version]

Reads spoken-form text from stdin, one sentence per line, and writes the
written form to stdout. Invalid UTF-8 bytes become U+FFFD.

Every setting is on. Switch one off with its flag:";

fn usage() -> String {
    let mut s = String::from(ABOUT);
    for setting in Setting::ALL {
        s.push_str("\n  --no-");
        s.push_str(setting.name());
    }
    s
}

fn run(engine: Normalizer) -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    let mut raw = Vec::new();
    let mut text = String::new();
    loop {
        raw.clear();
        if input.read_until(b'\n', &mut raw)? == 0 {
            return Ok(());
        }
        let line = raw.strip_suffix(b"\n").unwrap_or(&raw);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        text.clear();
        engine.normalize_into(&String::from_utf8_lossy(line), &mut text);
        writeln!(stdout, "{text}")?;
    }
}

fn start(engine: Normalizer) -> ExitCode {
    match run(engine) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("clm-itn: {e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flags: [&str; 2]| args.iter().any(|a| flags.contains(&a.as_str()));
    if has(["--help", "-h"]) {
        println!("{}", usage());
        return ExitCode::SUCCESS;
    }
    if has(["--version", "-V"]) {
        println!("clm-itn {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let mut engine = Normalizer::english();
    for a in &args {
        match a.strip_prefix("--no-").and_then(Setting::from_name) {
            Some(setting) => engine = engine.without(setting),
            None => {
                eprintln!("{}", usage());
                return ExitCode::from(2);
            }
        }
    }
    start(engine)
}
