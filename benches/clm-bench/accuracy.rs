use std::process::ExitCode;

use clm_itn::{Normalizer, Setting};

use crate::lines::{self, CorpusFile};
use crate::score::value_runs;

const SHOWN: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub lang: String,
    pub digits: bool,
    pub path: String,
}

pub fn parse_spec(v: &str) -> Result<Spec, String> {
    let (head, path) = v
        .split_once(':')
        .filter(|(_, p)| !p.is_empty())
        .ok_or(format!(
            "--accuracy wants <lang>[+digits]:<path>, got {v:?}"
        ))?;
    let (lang, digits) = match head.strip_suffix("+digits") {
        Some(l) => (l, true),
        None => (head, false),
    };
    normalizer(lang).ok_or(format!("unknown language {lang:?} in {v:?}"))?;
    Ok(Spec {
        lang: lang.to_string(),
        digits,
        path: path.to_string(),
    })
}

fn normalizer(lang: &str) -> Option<Normalizer<'static>> {
    Some(match lang {
        "en" => Normalizer::english(),
        "es" => Normalizer::spanish(),
        "pt" => Normalizer::portuguese(),
        #[cfg(feature = "german")]
        "de" => Normalizer::german(),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub lines: usize,
    pub exact: usize,
    pub values: usize,
}

impl Tally {
    pub fn add(&mut self, other: Tally) {
        self.lines += other.lines;
        self.exact += other.exact;
        self.values += other.values;
    }
}

pub fn pct(n: usize, of: usize) -> f64 {
    if of == 0 {
        0.0
    } else {
        100.0 * n as f64 / of as f64
    }
}

pub fn tally(engine: &Normalizer, file: &CorpusFile, failures: &mut Vec<String>) -> Tally {
    let mut t = Tally::default();
    for case in &file.cases {
        let Some(want) = &case.written else { continue };
        let got = engine.normalize(&case.spoken);
        t.lines += 1;
        t.exact += usize::from(got == *want);
        if value_runs(&got) == value_runs(want) {
            t.values += 1;
        } else if failures.len() < SHOWN {
            failures.push(format!("{} → {got}  (want {want})", case.spoken));
        }
    }
    t
}

pub fn run(specs: &[Spec]) -> ExitCode {
    println!("exact: output equals the written form");
    println!(
        "values: same numbers in the same order; currency and unit placement, case and . or , inside numbers ignored"
    );
    let mut averages = Vec::new();
    for spec in specs {
        let Some(mut engine) = normalizer(&spec.lang) else {
            return ExitCode::FAILURE;
        };
        if spec.digits {
            engine = engine.with(Setting::SingleDigits);
        }
        let files = match lines::load(&spec.path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
        let digits = if spec.digits { " (+digits)" } else { "" };
        println!("\n{}{digits}: {}", spec.lang, spec.path);
        let (mut total, mut failures) = (Tally::default(), Vec::new());
        for file in &files {
            let t = tally(&engine, file, &mut failures);
            if t.lines == 0 {
                continue;
            }
            let name = file.name.rsplit('/').next().unwrap_or(&file.name);
            println!(
                "  {name:32} {:5} lines  exact {:5.1}%  values {:5.1}%",
                t.lines,
                pct(t.exact, t.lines),
                pct(t.values, t.lines)
            );
            total.add(t);
        }
        let (exact, values) = (
            pct(total.exact, total.lines),
            pct(total.values, total.lines),
        );
        println!(
            "  {:32} {:5} lines  exact {exact:5.1}%  values {values:5.1}%",
            "all", total.lines
        );
        for f in &failures {
            println!("    {f}");
        }
        averages.push((spec.lang.as_str(), exact, values));
    }
    if averages.len() > 1 {
        let n = averages.len() as f64;
        let exact = averages.iter().map(|a| a.1).sum::<f64>() / n;
        let values = averages.iter().map(|a| a.2).sum::<f64>() / n;
        let langs: Vec<&str> = averages.iter().map(|a| a.0).collect();
        println!(
            "\naverage over {}: exact {exact:.1}%  values {values:.1}%",
            langs.join(", ")
        );
    }
    ExitCode::SUCCESS
}
