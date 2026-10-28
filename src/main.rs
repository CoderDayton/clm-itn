use std::fs::File;
use std::io::{self, BufRead, BufReader, ErrorKind, IsTerminal, Write};
use std::process::ExitCode;

use clm_itn::{Normalizer, Setting};

const USAGE: &str = "Usage:
  clm-itn [OPTIONS] [TEXT]...
  clm-itn [OPTIONS] -f <FILE>...
  <command> | clm-itn [OPTIONS]";

fn about(setting: Setting) -> &'static str {
    match setting {
        Setting::SingleDigits => "write a lone 0-9 as a digit: \"like 8 years\"",
        Setting::Percent => "\"fifty percent\" stays \"50 percent\"",
        Setting::Currency => "\"five dollars\" stays \"5 dollars\"",
        Setting::Dates => "no capital or comma: \"july 4th 1990\"",
        Setting::Letters => "spelled letters stay apart: \"7 a m e s t\"",
        Setting::Noon => "\"twelve noon\" stays \"12 noon\"",
        Setting::Oclock => "\"oclock\" is not rewritten to \"o'clock\"",
        Setting::Temperature => "\"fahrenheit\" and \"centigrade\" stay as spoken",
        _ => "",
    }
}

fn short(setting: Setting) -> char {
    match setting {
        Setting::SingleDigits => 'd',
        Setting::Percent => 'p',
        Setting::Currency => 'c',
        Setting::Dates => 'D',
        Setting::Letters => 'l',
        Setting::Noon => 'n',
        Setting::Oclock => 'o',
        Setting::Temperature => 't',
        _ => '\0',
    }
}

fn help() -> String {
    let mut s = format!(
        "clm-itn {}
Inverse text normalization: spoken numbers, times, dates and units to written form.

{USAGE}

Arguments:
  [TEXT]...             Spoken-form text; each argument is one sentence and
                        prints on its own line. Reads stdin when no TEXT or
                        FILE is given.

Options:
  -f, --file <FILE>     Normalize every line of FILE; \"-\" is stdin. Repeatable.
      --json            Write one JSON object per line: {{\"input\",\"output\"}}.
                        Errors go to stderr as {{\"error\":\"...\"}}.
      --                End of options; the rest is text.
  -h, --help            Print this help.
  -V, --version         Print the version.

Settings:
  Each setting has a long flag and a short flag that flips it from its
  default. Short flags combine, as in -dp. A later flag wins.",
        env!("CARGO_PKG_VERSION")
    );
    for setting in Setting::ALL {
        let flag = if Normalizer::english().is_on(*setting) {
            format!("--no-{}", setting.name())
        } else {
            format!("--{}", setting.name())
        };
        s.push_str(&format!(
            "\n  -{}, {flag:<20} {}",
            short(*setting),
            about(*setting)
        ));
    }
    s
}

enum Input<'a> {
    Text(&'a str),
    File(&'a str),
}

struct Options<'a> {
    inputs: Vec<Input<'a>>,
    settings: Vec<(Setting, bool)>,
    json: bool,
}

enum Parsed<'a> {
    Run(Options<'a>),
    Help,
    Version,
    Usage(String),
}

fn parse(args: &[String]) -> Parsed<'_> {
    let mut o = Options {
        inputs: Vec::new(),
        settings: Vec::new(),
        json: false,
    };
    let mut flags = true;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        i += 1;
        if !flags || !a.starts_with('-') || a == "-" {
            o.inputs.push(Input::Text(a));
            continue;
        }
        match a {
            "--" => flags = false,
            "-h" | "--help" => return Parsed::Help,
            "-V" | "--version" => return Parsed::Version,
            "--json" => o.json = true,
            "-f" | "--file" => match args.get(i) {
                Some(path) => {
                    o.inputs.push(Input::File(path));
                    i += 1;
                }
                None => return Parsed::Usage(format!("{a} needs a file")),
            },
            _ => {
                if let Some(path) = a.strip_prefix("--file=") {
                    o.inputs.push(Input::File(path));
                } else if let Some(s) = a.strip_prefix("--no-").and_then(Setting::from_name) {
                    o.settings.push((s, false));
                } else if let Some(s) = a.strip_prefix("--").and_then(Setting::from_name) {
                    o.settings.push((s, true));
                } else if let Some(cluster) = shorts(a) {
                    o.settings
                        .extend(cluster.into_iter().map(|s| (s, !is_default_on(s))));
                } else {
                    return Parsed::Usage(format!("unknown option {a}"));
                }
            }
        }
    }
    Parsed::Run(o)
}

fn is_default_on(setting: Setting) -> bool {
    Normalizer::english().is_on(setting)
}

fn shorts(arg: &str) -> Option<Vec<Setting>> {
    let letters = arg
        .strip_prefix('-')
        .filter(|l| !l.is_empty() && !l.starts_with('-'))?;
    letters
        .chars()
        .map(|c| Setting::ALL.iter().copied().find(|s| short(*s) == c))
        .collect()
}

fn json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct Writer<'a> {
    normalizer: Normalizer<'static>,
    json: bool,
    out: io::StdoutLock<'a>,
    text: String,
    line: String,
}

impl Writer<'_> {
    fn line(&mut self, input: &str) -> io::Result<()> {
        self.text.clear();
        self.normalizer.normalize_into(input, &mut self.text);
        if self.json {
            self.line.clear();
            self.line.push_str("{\"input\":");
            json_string(input, &mut self.line);
            self.line.push_str(",\"output\":");
            json_string(&self.text, &mut self.line);
            self.line.push('}');
            writeln!(self.out, "{}", self.line)
        } else {
            writeln!(self.out, "{}", self.text)
        }
    }

    fn lines(&mut self, mut input: impl BufRead) -> io::Result<()> {
        let mut raw = Vec::new();
        loop {
            raw.clear();
            if input.read_until(b'\n', &mut raw)? == 0 {
                return Ok(());
            }
            let line = raw.strip_suffix(b"\n").unwrap_or(&raw);
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            self.line(&String::from_utf8_lossy(line))?;
        }
    }
}

enum Failure {
    Io(Option<String>, io::Error),
    Out(io::Error),
}

fn run(o: &Options, w: &mut Writer) -> Result<(), Failure> {
    if o.inputs.is_empty() {
        return w.lines(io::stdin().lock()).map_err(Failure::Out);
    }
    for input in &o.inputs {
        match input {
            Input::Text(t) => w.line(t).map_err(Failure::Out)?,
            Input::File("-") => w.lines(io::stdin().lock()).map_err(Failure::Out)?,
            Input::File(path) => {
                let file =
                    File::open(path).map_err(|e| Failure::Io(Some((*path).to_owned()), e))?;
                w.lines(BufReader::new(file)).map_err(|e| {
                    if e.kind() == ErrorKind::BrokenPipe {
                        Failure::Out(e)
                    } else {
                        Failure::Io(Some((*path).to_owned()), e)
                    }
                })?;
            }
        }
    }
    Ok(())
}

fn report(json: bool, message: &str) {
    if json {
        let mut s = String::from("{\"error\":");
        json_string(message, &mut s);
        s.push('}');
        eprintln!("{s}");
    } else {
        eprintln!("clm-itn: {message}");
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let o = match parse(&args) {
        Parsed::Help => {
            println!("{}", help());
            return ExitCode::SUCCESS;
        }
        Parsed::Version => {
            println!("clm-itn {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Parsed::Usage(message) => {
            eprintln!("clm-itn: {message}\n\n{USAGE}\n\nTry 'clm-itn --help'.");
            return ExitCode::from(2);
        }
        Parsed::Run(o) => o,
    };
    if o.inputs.is_empty() && io::stdin().is_terminal() {
        println!("{}", help());
        return ExitCode::SUCCESS;
    }
    let mut normalizer = Normalizer::english();
    for &(setting, on) in &o.settings {
        normalizer = if on {
            normalizer.with(setting)
        } else {
            normalizer.without(setting)
        };
    }
    let mut w = Writer {
        normalizer,
        json: o.json,
        out: io::stdout().lock(),
        text: String::new(),
        line: String::new(),
    };
    match run(&o, &mut w) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Out(e)) if e.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(Failure::Out(e)) => {
            report(o.json, &e.to_string());
            ExitCode::FAILURE
        }
        Err(Failure::Io(path, e)) => {
            let message = match path {
                Some(p) => format!("{p}: {e}"),
                None => e.to_string(),
            };
            report(o.json, &message);
            ExitCode::FAILURE
        }
    }
}
