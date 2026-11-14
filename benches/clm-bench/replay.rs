use clm_itn::Setting;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Time `Normalizer::format` over a generated corpus and gate on p99.
    Gate {
        n: usize,
        seed: u64,
        tiers: bool,
    },
    /// Score and time `format_line` over corpus files.
    Corpus(CorpusOpts),
    /// Score a `tools/pairgen/` split file per semiotic class.
    Pairs {
        path: String,
        isolated: bool,
    },
    /// Turn the digits of an `id TAB raw TAB final` file back into words.
    Spell(String),
    Accuracy(Vec<crate::accuracy::Spec>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorpusOpts {
    pub path: String,
    pub passes: usize,
    pub tag: Option<String>,
    pub all_failures: bool,
    pub off: Vec<Setting>,
    /// Also time each line cut into sentences, normalized back to back.
    pub sentences: bool,
}

pub const DEFAULT_N: usize = 1_000_000;
pub const DEFAULT_SEED: u64 = 1;
pub const USAGE: &str = "usage: cargo bench --bench clm-bench -- [--n <count>] [--seed <u64>] [--tiers]
       cargo bench --bench clm-bench -- --corpus <file|folder> [--passes <count>] [--tag <t>] [--all-failures] [--sentences] [--no-<setting>]...
       cargo bench --bench clm-bench -- --pairs <pairgen split file> [--isolated]
       cargo bench --bench clm-bench -- --spell <id TAB raw TAB final file>
       cargo bench --bench clm-bench -- --accuracy <en|es|pt|de>[+digits]:<file|folder>...";

fn once<T>(slot: &mut Option<T>, flag: &str, v: T) -> Result<(), String> {
    match slot.replace(v) {
        Some(_) => Err(format!("{flag} given twice\n{USAGE}")),
        None => Ok(()),
    }
}

fn positive(flag: &str, v: &str) -> Result<usize, String> {
    match v.parse::<usize>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("{flag} must be a positive integer, got {v:?}")),
    }
}

pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Mode, String> {
    let (mut n, mut seed, mut tiers) = (None, None, false);
    let (mut corpus, mut passes, mut tag, mut all_failures) = (None, None, None, false);
    let mut sentences = false;
    let mut off: Vec<Setting> = Vec::new();
    let (mut pairs, mut isolated, mut spell) = (None, false, None);
    let mut accuracy = Vec::new();
    let mut it = args.into_iter();
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .filter(|v| !v.starts_with("--"))
                .ok_or(format!("{flag} needs a value\n{USAGE}"))
        };
        match flag.as_str() {
            "--n" => once(&mut n, &flag, positive(&flag, &value()?)?)?,
            "--seed" => {
                let v = value()?;
                let s = v
                    .parse::<u64>()
                    .map_err(|_| format!("--seed must be a u64, got {v:?}"))?;
                once(&mut seed, &flag, s)?;
            }
            "--tiers" => tiers = true,
            // Appended by `cargo bench`.
            "--bench" => {}
            "--corpus" => once(&mut corpus, &flag, value()?)?,
            "--passes" => once(&mut passes, &flag, positive(&flag, &value()?)?)?,
            "--tag" => once(&mut tag, &flag, value()?)?,
            "--all-failures" => all_failures = true,
            "--sentences" => sentences = true,
            "--pairs" => once(&mut pairs, &flag, value()?)?,
            "--isolated" => isolated = true,
            "--spell" => once(&mut spell, &flag, value()?)?,
            "--accuracy" => accuracy.push(crate::accuracy::parse_spec(&value()?)?),
            f => match f.strip_prefix("--no-").and_then(Setting::from_name) {
                Some(s) if !off.contains(&s) => off.push(s),
                Some(_) => return Err(format!("{f} given twice\n{USAGE}")),
                None => return Err(format!("unknown argument {f:?}\n{USAGE}")),
            },
        }
    }
    let gate = n.is_some() || seed.is_some() || tiers;
    let modes = [
        gate,
        corpus.is_some(),
        pairs.is_some(),
        spell.is_some(),
        !accuracy.is_empty(),
    ];
    if modes.iter().filter(|&&m| m).count() > 1 {
        return Err(format!(
            "--n/--seed/--tiers, --corpus, --pairs, --spell and --accuracy are separate modes\n{USAGE}"
        ));
    }
    if !accuracy.is_empty() {
        return Ok(Mode::Accuracy(accuracy));
    }
    let corpus_flags =
        passes.is_some() || tag.is_some() || all_failures || sentences || !off.is_empty();
    if corpus_flags && corpus.is_none() {
        return Err(format!(
            "--passes, --tag, --all-failures, --sentences and --no-<setting> need --corpus\n{USAGE}"
        ));
    }
    if isolated && pairs.is_none() {
        return Err(format!("--isolated needs --pairs\n{USAGE}"));
    }
    Ok(match (corpus, pairs, spell) {
        (Some(path), _, _) => Mode::Corpus(CorpusOpts {
            path,
            passes: passes.unwrap_or(crate::lines::DEFAULT_PASSES),
            tag,
            all_failures,
            off,
            sentences,
        }),
        (_, Some(path), _) => Mode::Pairs { path, isolated },
        (_, _, Some(path)) => Mode::Spell(path),
        _ => Mode::Gate {
            n: n.unwrap_or(DEFAULT_N),
            seed: seed.unwrap_or(DEFAULT_SEED),
            tiers,
        },
    })
}

pub fn net_ticks(t0: u64, t1: u64, overhead: u64) -> u32 {
    let net = t1.wrapping_sub(t0).saturating_sub(overhead);
    u32::try_from(net).unwrap_or(u32::MAX)
}

pub fn percentile(sorted: &[u32], p: f64) -> u32 {
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}
