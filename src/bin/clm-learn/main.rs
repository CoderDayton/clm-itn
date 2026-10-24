use std::path::Path;
use std::process::ExitCode;

use clm_itn::builder::learn::{
    DomainEval, MAX_WRONG, Options, Outcome, Plain, exact_sentences, learn_with,
};
use clm_itn::builder::learned::{self, Learned};
use clm_itn::builder::{Built, build};
use clm_itn::engine::Engine;
use clm_itn::engine::window::{format_line, windows};
use clm_itn::tables::Tables;
use clm_itn::types::TAU;

const USAGE: &str = "usage: clm-learn [--plain=strong|all|off] <pairs dir with train.txt val.txt test.txt> <learned.txt out>";
const MAX_FILE_BYTES: u64 = 2 << 30;
const DOMAIN_NAMES: [&str; 11] = [
    "cardinal",
    "time",
    "identifier",
    "ordinal",
    "date",
    "plain",
    "money",
    "cents",
    "percent",
    "negative",
    "measure",
];

fn runtime_windows(n: usize, numeric: &dyn Fn(usize) -> bool) -> Vec<(usize, usize, bool)> {
    windows(n, numeric)
        .into_iter()
        .map(|w| (w.start, w.end, w.raw))
        .collect()
}

fn runtime_format(t: &Tables, lines: &[String]) -> Vec<String> {
    let engine = Engine::new(t);
    lines
        .iter()
        .map(|l| {
            let mut out = String::new();
            format_line(&engine, l, &mut out);
            out
        })
        .collect()
}

fn read(dir: &Path, name: &str) -> Result<String, String> {
    let path = dir.join(name);
    let len = std::fs::metadata(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if len > MAX_FILE_BYTES {
        return Err(format!("{}: {len} bytes is over the limit", path.display()));
    }
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let plain = match args.iter().find_map(|a| a.strip_prefix("--plain=")) {
        None | Some("strong") => Plain::Strong,
        Some("all") => Plain::All,
        Some("off") => Plain::Off,
        Some(_) => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    args.retain(|a| !a.starts_with("--plain="));
    let [dir, out] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let opts = Options { plain };
    match run(Path::new(dir), Path::new(out), &opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("clm-learn: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(dir: &Path, out: &Path, opts: &Options) -> Result<(), String> {
    let train = read(dir, "train.txt")?;
    let val = read(dir, "val.txt")?;
    let test = read(dir, "test.txt")?;
    let outcome = learn_with(&train, &val, &runtime_windows, &runtime_format, opts)
        .map_err(|e| format!("{e:?}"))?;
    let mut learned = outcome.learned.clone();
    learned.comments = vec![
        format!("tau {}", outcome.tau),
        format!("floor {}", learned.floor.unwrap_or(1000)),
        format!("plain labels {:?}", opts.plain),
    ];
    let text = learned::write(&learned);
    let check = learned::parse(&text).map_err(|e| format!("re-parse: {e:?}"))?;
    let base = build(&Learned::default()).map_err(|e| format!("{e:?}"))?;
    let done = build(&check).map_err(|e| format!("{e:?}"))?;
    std::fs::write(out, &text).map_err(|e| format!("{}: {e}", out.display()))?;
    report(
        &outcome,
        &learned,
        &base,
        &done,
        &[("val", &val), ("test", &test)],
    );
    Ok(())
}

fn report(o: &Outcome, l: &Learned, base: &Built, done: &Built, splits: &[(&str, &str)]) {
    println!(
        "learned: shapes {} pairs {} lefts {} rights {} cells {} cues {}",
        l.shapes.len(),
        l.pairs.len(),
        l.lefts.len(),
        l.rights.len(),
        l.cells.len(),
        l.cues.len()
    );
    println!(
        "tier3: gold spans changed by digit-group comma removal: {}",
        o.canonicalized
    );
    println!("build: before {:?}", base.stats);
    println!("build: after  {:?}", done.stats);
    println!("floor sweep (val exact sentences):");
    for (f, c) in &o.floors {
        let mark = if Some(*f) == l.floor { " <" } else { "" };
        println!("  floor {f:4}  {:.2}%{mark}", c * 100.0);
    }
    println!(
        "tau: picked {} (smallest with wrong-domain <= {:.1}%), types::TAU = {TAU}",
        o.tau,
        MAX_WRONG * 100.0
    );
    println!(
        "val wrong-domain: before {:.2}% at TAU, after {:.2}% at TAU, {:.2}% at picked",
        o.before.wrong_rate(TAU) * 100.0,
        o.after.wrong_rate(TAU) * 100.0,
        o.after.wrong_rate(o.tau) * 100.0
    );
    println!("val per domain at TAU {TAU}: spans  accuracy before/after  abstain before/after");
    for (d, name) in DOMAIN_NAMES.iter().enumerate() {
        let (b, a) = (o.before.domain(d as u8, TAU), o.after.domain(d as u8, TAU));
        if b.spans == 0 {
            continue;
        }
        println!(
            "  {name:10} {:8}  {:6.2}% {:6.2}%  {:6.2}% {:6.2}%",
            b.spans,
            share(b.correct, b),
            share(a.correct, a),
            share(b.abstained, b),
            share(a.abstained, a)
        );
    }
    for (name, text) in splits {
        let (bs, bn) = exact_sentences(&runtime_format, base, text);
        let (ds, dn) = exact_sentences(&runtime_format, done, text);
        println!(
            "{name} exact sentences: before {bs}/{bn} ({:.2}%), after {ds}/{dn} ({:.2}%)",
            pct(bs, bn),
            pct(ds, dn)
        );
    }
}

fn share(k: u32, e: DomainEval) -> f64 {
    pct(u64::from(k), u64::from(e.spans))
}

fn pct(k: u64, n: u64) -> f64 {
    if n == 0 {
        0.0
    } else {
        k as f64 * 100.0 / n as f64
    }
}
