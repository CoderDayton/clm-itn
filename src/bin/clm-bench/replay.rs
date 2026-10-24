#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Args {
    pub n: usize,
    pub seed: u64,
    pub tiers: bool,
    pub lines: Option<String>,
    pub passes: usize,
}

pub const DEFAULT_N: usize = 1_000_000;
pub const DEFAULT_SEED: u64 = 1;
pub const USAGE: &str =
    "usage: clm-bench [--n <count>] [--seed <u64>] [--tiers] | --lines <tsv> [--passes <count>]";

pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Args, String> {
    let mut out = Args {
        n: DEFAULT_N,
        seed: DEFAULT_SEED,
        tiers: false,
        lines: None,
        passes: crate::lines::DEFAULT_PASSES,
    };
    let (mut replay_flag, mut passes_flag) = (false, false);
    let mut it = args.into_iter();
    while let Some(flag) = it.next() {
        replay_flag |= matches!(flag.as_str(), "--n" | "--seed" | "--tiers");
        passes_flag |= flag == "--passes";
        let value = |v: Option<String>| v.ok_or(format!("{flag} needs a value\n{USAGE}"));
        match flag.as_str() {
            "--n" => {
                let v = value(it.next())?;
                out.n = match v.parse::<usize>() {
                    Ok(n) if n > 0 => n,
                    _ => return Err(format!("--n must be a positive integer, got {v:?}")),
                };
            }
            "--tiers" => out.tiers = true,
            "--lines" => out.lines = Some(value(it.next())?),
            "--passes" => {
                let v = value(it.next())?;
                out.passes = match v.parse::<usize>() {
                    Ok(n) if n > 0 => n,
                    _ => return Err(format!("--passes must be a positive integer, got {v:?}")),
                };
            }
            "--seed" => {
                let v = value(it.next())?;
                out.seed = v
                    .parse()
                    .map_err(|_| format!("--seed must be a u64, got {v:?}"))?;
            }
            _ => return Err(format!("unknown argument {flag:?}\n{USAGE}")),
        }
    }
    match (&out.lines, replay_flag, passes_flag) {
        (Some(_), true, _) => Err(format!(
            "--lines cannot be combined with --n, --seed or --tiers\n{USAGE}"
        )),
        (None, _, true) => Err(format!("--passes needs --lines\n{USAGE}")),
        _ => Ok(out),
    }
}

pub fn net_ticks(t0: u64, t1: u64, overhead: u64) -> u32 {
    let net = t1.wrapping_sub(t0).saturating_sub(overhead);
    u32::try_from(net).unwrap_or(u32::MAX)
}

pub fn percentile(sorted: &[u32], p: f64) -> u32 {
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(v: &[&str]) -> Result<Args, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_without_args() {
        assert_eq!(
            parse(&[]),
            Ok(Args {
                n: DEFAULT_N,
                seed: DEFAULT_SEED,
                tiers: false,
                lines: None,
                passes: crate::lines::DEFAULT_PASSES,
            })
        );
    }

    #[test]
    fn parses_n_and_seed_in_any_order() {
        let want = Ok(Args {
            n: 500,
            seed: 9,
            tiers: false,
            lines: None,
            passes: crate::lines::DEFAULT_PASSES,
        });
        assert_eq!(parse(&["--n", "500", "--seed", "9"]), want);
        assert_eq!(parse(&["--seed", "9", "--n", "500"]), want);
    }

    #[test]
    fn parses_tiers_flag_without_a_value() {
        let a = parse(&["--tiers", "--n", "7"]).unwrap();
        assert!(a.tiers);
        assert_eq!(a.n, 7);
        assert!(parse(&["--n", "7", "--tiers"]).unwrap().tiers);
        assert!(!parse(&["--n", "7"]).unwrap().tiers);
        assert!(parse(&["--tiers=1"]).is_err());
    }

    #[test]
    fn parses_a_lines_file_and_passes() {
        let a = parse(&["--lines", "d.tsv"]).unwrap();
        assert_eq!(a.lines.as_deref(), Some("d.tsv"));
        assert_eq!(a.passes, crate::lines::DEFAULT_PASSES);
        let a = parse(&["--passes", "3", "--lines", "d.tsv"]).unwrap();
        assert_eq!((a.lines.as_deref(), a.passes), (Some("d.tsv"), 3));
        assert!(parse(&["--lines"]).is_err());
        assert!(parse(&["--lines", "d.tsv", "--passes", "0"]).is_err());
        assert!(parse(&["--passes", "3"]).is_err());
        assert!(parse(&["--lines", "d.tsv", "--tiers"]).is_err());
        assert!(parse(&["--lines", "d.tsv", "--n", "5"]).is_err());
        assert!(parse(&["--seed", "2", "--lines", "d.tsv"]).is_err());
    }

    #[test]
    fn rejects_bad_args() {
        assert!(parse(&["--n"]).is_err());
        assert!(parse(&["--n", "x"]).is_err());
        assert!(parse(&["--n", "0"]).is_err());
        assert!(parse(&["--seed", "-1"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
    }

    #[test]
    fn net_ticks_subtracts_overhead_and_saturates() {
        assert_eq!(net_ticks(100, 150, 30), 20);
        assert_eq!(net_ticks(100, 120, 30), 0);
        assert_eq!(net_ticks(0, u64::MAX, 0), u32::MAX);
    }

    #[test]
    fn percentile_is_nearest_rank() {
        let v: Vec<u32> = (1..=1000).collect();
        assert_eq!(percentile(&v, 0.50), 500);
        assert_eq!(percentile(&v, 0.99), 990);
        assert_eq!(percentile(&v, 0.999), 999);
        assert_eq!(percentile(&v, 1.0), 1000);
        assert_eq!(percentile(&[7], 0.99), 7);
    }
}
