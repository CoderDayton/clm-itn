use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use clm_itn::engine::Engine;
use clm_itn::engine::window::format_line;

const USAGE: &str = "Usage: clm-itn [--single-digits] [--help | --version]

Reads spoken-form text from stdin, one sentence per line, and writes the
written form to stdout. Invalid UTF-8 bytes become U+FFFD.

--single-digits  write lone single numbers as digits (\"8 years\") instead of AP
                 words (\"eight years\")";

fn run(digits: bool) -> io::Result<()> {
    let engine = Engine::new(clm_itn::english()).with_single_digits(digits);
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
        format_line(&engine, &String::from_utf8_lossy(line), &mut text);
        writeln!(stdout, "{text}")?;
    }
}

fn start(digits: bool) -> ExitCode {
    match run(digits) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("clm-itn: {e}");
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => start(false),
        [a] if a == "--single-digits" => start(true),
        [a] if a == "--help" || a == "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        [a] if a == "--version" || a == "-V" => {
            println!("clm-itn {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
