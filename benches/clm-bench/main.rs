mod alloc;
mod clock;
mod corpus;
mod gold;
mod lines;
mod pairs;
// Built under `test` on every host so the unit tests cover it.
#[cfg(any(test, all(target_arch = "aarch64", target_os = "linux")))]
#[cfg_attr(
    not(all(target_arch = "aarch64", target_os = "linux")),
    allow(dead_code)
)]
mod pmu;
mod replay;
mod score;
mod spell;
mod tiers;

use std::hint::black_box;
use std::process::ExitCode;

use clm_itn::engine::Normalizer;
use clm_itn::format_line;
use clm_itn::types::MAX_TOKENS;

use replay::{CorpusOpts, Mode};

const OUT_BYTES: usize = 256;
const P99_LIMIT_NS: f64 = 1000.0;
const SAMPLES: usize = 5;

fn main() -> ExitCode {
    let mode = match replay::parse_args(std::env::args().skip(1)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    match mode {
        Mode::Pairs { path, isolated } => pairs::run(&path, isolated),
        Mode::Spell(path) => spell::run(&path),
        Mode::Corpus(opts) => match lines::load(&opts.path) {
            Ok(files) => {
                let (ns_per_tick, overhead, _) = timer();
                run_corpus(&opts, &files, overhead, ns_per_tick)
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::FAILURE
            }
        },
        Mode::Gate { n, seed, tiers } => {
            let (ns_per_tick, overhead, gate) = timer();
            run_gate(n, seed, tiers, overhead, ns_per_tick, gate)
        }
    }
}

fn timer() -> (f64, u64, bool) {
    let ns_per_tick = clock::ns_per_tick();
    let overhead = clock::overhead_ticks();
    let gate = clock::gate_passes();
    print_header(ns_per_tick, overhead, gate);
    (ns_per_tick, overhead, gate)
}

fn run_gate(
    n: usize,
    seed: u64,
    tiers: bool,
    overhead: u64,
    ns_per_tick: f64,
    gate: bool,
) -> ExitCode {
    let engine = Normalizer::new(clm_itn::english());
    let corpus = corpus::Corpus::generate(n, seed);
    let n = corpus.len();
    let mut ticks = vec![0u32; n];
    let mut buf: [&[u8]; MAX_TOKENS] = [&[]; MAX_TOKENS];
    let mut out = [0u8; OUT_BYTES];
    println!("corpus:         {n} utterances, seed {seed}");

    for i in 0..n {
        let k = corpus.utterance(i, &mut buf);
        let _ = black_box(engine.format(&buf[..k], &mut out));
    }

    let mut errors = 0usize;
    let allocs_before = alloc::allocations();
    for (i, slot) in ticks.iter_mut().enumerate() {
        let k = corpus.utterance(i, &mut buf);
        let t0 = clock::start();
        let r = engine.format(black_box(&buf[..k]), &mut out);
        let t1 = clock::end();
        errors += usize::from(black_box(r).is_err());
        *slot = replay::net_ticks(t0, t1, overhead);
    }
    let allocs = alloc::allocations() - allocs_before;

    ticks.sort_unstable();
    let ns = |p: f64| clock::ticks_to_ns(replay::percentile(&ticks, p) as u64, ns_per_tick);
    let (p50, p99, p999, max) = (ns(0.50), ns(0.99), ns(0.999), ns(1.0));
    println!("p50:            {p50:.1} ns");
    println!("p99:            {p99:.1} ns");
    println!("p99.9:          {p999:.1} ns");
    println!("max:            {max:.1} ns");
    println!(
        "errors:         {errors} of {n} ({:.4}%)",
        100.0 * errors as f64 / n as f64
    );
    println!("allocations:    {allocs} across the timed loop (must be 0)");
    let misses = clock::pmu_misses();
    if misses > 0 {
        println!("pmu misses:     {misses} reads had no live counter (pin to one core)");
    }

    print_samples(&engine, &corpus);

    if tiers {
        print_tiers(&engine, &corpus, overhead, ns_per_tick);
    }

    let pass = p99 <= P99_LIMIT_NS && allocs == 0 && errors == 0;
    let verdict = match (pass, gate) {
        (true, true) => "PASS",
        (true, false) => "PASS (coarse: cannot sign off)",
        (false, _) => "FAIL",
    };
    println!("result:         {verdict} (p99 {p99:.1} ns vs limit {P99_LIMIT_NS} ns)");
    if pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn run_corpus(
    opts: &CorpusOpts,
    files: &[lines::CorpusFile],
    overhead: u64,
    ns_per_tick: f64,
) -> ExitCode {
    let engine = opts
        .off
        .iter()
        .fold(Normalizer::english(), |e, &s| e.without(s));
    if !opts.off.is_empty() {
        let names: Vec<&str> = opts.off.iter().map(|s| s.name()).collect();
        println!("settings off:   {}", names.join(", "));
    }
    for f in files {
        println!("\ncorpus file:    {}", f.name);
        let wanted = |c: &&lines::Case| {
            opts.tag
                .as_deref()
                .is_none_or(|t| gold::tag_matches(&c.tag, t))
        };
        let cases: Vec<&lines::Case> = f.cases.iter().filter(wanted).collect();
        if let Some(t) = &opts.tag {
            println!("tag filter:     {t} ({} lines)", cases.len());
        }
        if cases.is_empty() {
            continue;
        }
        gold::print(
            &gold::score(&engine, &cases, opts.all_failures),
            f.malformed,
        );
        let spoken: Vec<&str> = cases.iter().map(|c| c.spoken.as_str()).collect();
        report_lines(&engine, &spoken, opts.passes, overhead, ns_per_tick);
    }
    ExitCode::SUCCESS
}

fn report_lines(
    engine: &Normalizer,
    spoken: &[&str],
    passes: usize,
    overhead: u64,
    ns_per_tick: f64,
) {
    let n = spoken.len();
    println!("lines:          {n} x {passes} passes (format_line, one reused String)");

    let cold = line_allocations(engine, spoken, false);
    let warm = line_allocations(engine, spoken, true);
    let ticks = time_lines(engine, spoken, passes, overhead);

    let ns = |t: u32| clock::ticks_to_ns(t as u64, ns_per_tick);
    let p99_line = lines::per_line(&ticks, n, 0.99);
    let max_line = lines::per_line(&ticks, n, 1.0);
    let mut all = ticks;
    all.sort_unstable();
    let pct = |p: f64| ns(replay::percentile(&all, p));
    let (p50, p99, p999, max) = (pct(0.50), pct(0.99), pct(0.999), pct(1.0));
    println!("p50:            {p50:.1} ns");
    println!("p99:            {p99:.1} ns");
    println!("p99.9:          {p999:.1} ns");
    println!("max:            {max:.1} ns");
    let over = p99_line.iter().filter(|&&t| ns(t) > P99_LIMIT_NS).count();
    println!("lines with p99 over {P99_LIMIT_NS} ns: {over} of {n}");
    println!("slowest lines by per-line p99 (max):");
    for i in lines::worst(&p99_line, lines::WORST) {
        println!(
            "  {:>8.1} ns ({:>8.1}) {:?}",
            ns(p99_line[i]),
            ns(max_line[i]),
            spoken[i]
        );
    }
    for (label, counts) in [("cold String", &cold), ("reused String", &warm)] {
        let a = lines::alloc_summary(counts);
        println!(
            "allocations ({label}): {} over {} calls, max {} per call, {} lines allocate",
            a.total, a.calls, a.max, a.lines_allocating
        );
        if let Some(i) = (0..n).filter(|&i| counts[i] > 0).max_by_key(|&i| counts[i]) {
            println!("  most: {} in {:?}", counts[i], spoken[i]);
        }
    }
    println!("result:         REPORT ONLY (p99 {p99:.1} ns vs budget {P99_LIMIT_NS} ns)");
}

fn line_allocations(engine: &Normalizer, spoken: &[&str], reuse: bool) -> Vec<u64> {
    let mut out = String::new();
    if reuse {
        for s in spoken {
            out.clear();
            format_line(engine, s, &mut out);
        }
    }
    spoken
        .iter()
        .map(|s| {
            if reuse {
                out.clear();
            } else {
                out = String::new();
            }
            let before = alloc::allocations();
            format_line(engine, s, &mut out);
            alloc::allocations() - before
        })
        .collect()
}

fn time_lines(engine: &Normalizer, spoken: &[&str], passes: usize, overhead: u64) -> Vec<u32> {
    let mut out = String::new();
    let mut ticks = vec![0u32; spoken.len() * passes];
    for pass in ticks.chunks_mut(spoken.len()) {
        for (slot, s) in pass.iter_mut().zip(spoken) {
            out.clear();
            let t0 = clock::start();
            format_line(engine, black_box(s), &mut out);
            let t1 = clock::end();
            black_box(&out);
            *slot = replay::net_ticks(t0, t1, overhead);
        }
    }
    ticks
}

fn print_tiers(engine: &Normalizer, corpus: &corpus::Corpus, overhead: u64, ns_per_tick: f64) {
    let src = tiers::source(engine);
    let c = tiers::collect(&src, corpus, overhead);
    let stats = tiers::stage_stats(c.stages.each_ref().map(Vec::as_slice), ns_per_tick);
    let (timed, skipped) = (corpus.len() - c.skipped, c.skipped);
    println!("tiers:          {timed} timed, {skipped} skipped");
    let why = tiers::StageSource::unavailable(&src);
    print!("{}", tiers::format_report(&stats, why));
}

fn print_header(ns_per_tick: f64, overhead: u64, gate: bool) {
    let cores = std::thread::available_parallelism().map_or(0, |n| n.get());
    println!("backend:        {}", clock::backend());
    if let Some(note) = clock::note() {
        println!("timer note:     {note}");
    }
    println!("ns_per_tick:    {ns_per_tick:.4}");
    println!(
        "overhead:       {overhead} ticks = {:.2} ns (median of {} pairs, subtracted)",
        clock::ticks_to_ns(overhead, ns_per_tick),
        clock::OVERHEAD_PAIRS
    );
    println!("invariant_tsc:  {}", clock::invariant_tsc());
    println!("gate:           {}", if gate { "pass" } else { "coarse" });
    if cores == 1 {
        println!("cores:          1 usable (pinned)");
    } else {
        println!(
            "cores:          {cores} usable (NOT pinned; sign-off needs `taskset -c <core> cargo bench --bench clm-bench`)"
        );
    }
}

fn print_samples(engine: &Normalizer, corpus: &corpus::Corpus) {
    let mut buf: [&[u8]; MAX_TOKENS] = [&[]; MAX_TOKENS];
    let mut out = [0u8; OUT_BYTES];
    let mut seen = Vec::new();
    let picks = (0..corpus.len()).filter(|&i| {
        let c = corpus.category(i);
        c != corpus::Category::Filler && !seen.contains(&c) && {
            seen.push(c);
            true
        }
    });
    for i in picks.take(SAMPLES) {
        let k = corpus.utterance(i, &mut buf);
        let input = join(&buf[..k]);
        let cat = corpus.category(i);
        match engine.format(&buf[..k], &mut out) {
            Ok(f) => println!(
                "sample {i}:       [{cat:?}] {input:?} -> {:?}",
                String::from_utf8_lossy(&out[..f.len])
            ),
            Err(e) => println!("sample {i}:       [{cat:?}] {input:?} -> Err({e:?})"),
        }
    }
}

fn join(tokens: &[&[u8]]) -> String {
    tokens
        .iter()
        .map(|t| String::from_utf8_lossy(t))
        .collect::<Vec<_>>()
        .join(" ")
}
