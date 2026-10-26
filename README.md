<div align="center">

# CLM-ITN

Inverse text normalization for English dictation: spoken numbers in, written numbers out, under 1 µs per call.

[![CI](https://github.com/CoderDayton/clm-itn/actions/workflows/ci.yml/badge.svg)](https://github.com/CoderDayton/clm-itn/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Rust 1.92+](https://img.shields.io/badge/rust-1.92%2B-orange.svg)
![no_std](https://img.shields.io/badge/no__std-supported-informational.svg)

</div>

CLM-ITN rewrites the spoken-form text a speech recognizer writes into written form. It covers cardinals, ordinals, decimals, times, dates, money, percents, measures, and digit identifiers. It is pure Rust with no dependencies.

Each call is three lookups over frozen static tables: a cuckoo lexicon gives each word a class, a 256×256 context table picks a number's domain from the words on either side, and a cell table picks a ranked formatting program. There is no model and no allocation. `Engine::format` measures p99 258 ns per call of up to 8 tokens, against a 1.00 µs budget (10⁶ generated utterances, one pinned core of an i9-13900K, rustc 1.97, 3 runs). On a 292-line dictation gold set, 96.9% of lines come out exactly right.

It sees only one word on each side of a number, and today it rewrites numbers only. Cases that need wider context, and text such as emails or URLs, are listed under [When not to use this](#when-not-to-use-this).

```text
meet me at five thirty pm in room four oh two            →  meet me at 5:30 pm in room 402
the twenty first of june costs two thousand five dollars  →  the 21st of june costs $2005
january eighth eighteen fifteen                           →  January 8, 1815
a two point four gigahertz band                           →  a 2.4 gigahertz band
```

## Features

- **Fast path:** `Engine::format` measures p99 257–259 ns and p99.9 305–318 ns per call over 10⁶ calls, with 0 heap allocations. It is gated at 1.00 µs. Method and per-tier split: [docs/PERFORMANCE.md](docs/PERFORMANCE.md).
- **Any line length:** `normalize` measures p99 947 ns per line on the 359-line dictation set, with at most 3 allocations per line when the `String` is reused. It has no gate: 6 of 359 lines exceed 1 µs at p99.
- **Deterministic:** the tables are `static` and read-only, so the same input always gives the same output. An `Engine` is `Copy`, `Send` and `Sync`, takes no locks, and `Engine::new` allocates nothing. How the tiers work: [docs/SPEC.md](docs/SPEC.md).
- **No dependencies, `no_std`:** with default features off, the library builds as `#![no_std]` and `format` needs no heap. The `alloc` feature adds `normalize`.
- **Two number styles:** AP (the default) keeps a lone single number as a word ("eight years"). `Style::SingleDigits` writes it as a digit ("8 years"), as Whisper does. Ordinals and numbers of two or more words are the same in both.
- **Compile-time tables:** `build.rs` builds the English tables from `src/builder/` into static data. A test checks that they match the runtime builder `builder::build_english()`.
- **Bounded shapes:** at most 2047 span shapes (role sequences). Rarer number patterns pass through unchanged.

## Usage

The crate is not on crates.io. Depend on the Git repository:

```toml
[dependencies]
clm-itn = { git = "https://github.com/CoderDayton/clm-itn" }
```

Install the command-line tool:

```bash
cargo install --git https://github.com/CoderDayton/clm-itn --bin clm-itn
echo "at five thirty pm" | clm-itn
```

Library:

```rust
use clm_itn::Engine;

let engine = Engine::english();
assert_eq!(engine.normalize("meet me at five thirty pm"), "meet me at 5:30 pm");

// No allocation: up to 8 pre-split tokens into a byte buffer.
let tokens: [&[u8]; 4] = [b"at", b"five", b"thirty", b"pm"];
let mut out = [0u8; 64];
let done = engine.format(&tokens, &mut out)?;
assert_eq!(&out[..done.len], b"at 5:30 pm");
# Ok::<(), clm_itn::FormatError>(())
```

| Export | What it does | Use it for |
|---|---|---|
| `Engine::english()` | An engine over the built-in English tables | The default engine; `Engine::default()` is the same |
| `Engine::with_style(Style)` | Picks `Style::Ap` or `Style::SingleDigits` | Matching AP or Whisper number style |
| `Engine::normalize(&str) -> String` | Rewrites a line of any length | General use (`alloc`) |
| `Engine::normalize_into(&str, &mut String)` | Same, appending to your buffer | Many lines, one buffer (`alloc`) |
| `format_line(&Engine, &str, &mut String)` | Same as `normalize_into` | Free-function form (`alloc`) |
| `Engine::format(&[&[u8]], &mut [u8])` | Rewrites up to 8 tokens, no allocation | Hot loops, `no_std` |
| `english() -> &'static Tables`, `Engine::new(&Tables)` | The static tables, and an engine over any tables | Custom tables |
| `MAX_TOKENS` | `8`, the token limit of one `format` call | Sizing windows |

| Command | What it does |
|---|---|
| `clm-itn [--single-digits]` | Reads one sentence per line from stdin and writes the written form to stdout |
| `clm-bench` | Latency replay of `Engine::format`; `--tiers` splits it by tier, `--lines <tsv>` times `format_line` |
| `clm-eval --lines <tsv>` | Scores the engine against `spoken`, `written`, optional tag lines |
| `clm-learn <pairs dir> <out>` | Learns table entries from a pair corpus ([docs/TRAINING.md](docs/TRAINING.md)) |

- Input is spoken-form text, one sentence per line, lowercase or cased. Digits already in the input pass through unchanged, so text that already has digits (such as Whisper output) needs no pass.
- `format` takes tokens without punctuation. It returns `FormatError::TooManyTokens` for more than 8 tokens and `FormatError::Overflow` when the buffer is too small.
- `normalize` splits long lines into windows of up to 8 tokens without cutting a number, and keeps punctuation in place ("twenty five," → `25,`). A number spoken in more than 8 words is joined only when it splits after `thousand`, `million`, `billion` or `trillion` into pieces that each render as digits. Otherwise it stays as words.
- The CLI replaces invalid UTF-8 bytes with U+FFFD and collapses runs of whitespace to one space.

| Feature | Adds |
|---|---|
| none | `Engine`, `Engine::format`, `Style`, `FormatError`; no heap |
| `alloc` | `normalize`, `normalize_into`, `format_line` |
| `std` (default) | `alloc`, the table builder, and the four binaries |

Every export and its edge cases: [docs/API.md](docs/API.md). Runnable examples: [docs/examples.md](docs/examples.md).

## When not to use this

| Situation | Use instead | Why |
|---|---|---|
| Languages other than English | NVIDIA NeMo-text-processing ITN | Only English tables ship. |
| Emails, URLs, abbreviations ("doctor" → "dr.") | NeMo-text-processing ITN | Not handled; these pass through unchanged. |
| Unit abbreviations or dotted am/pm ("5 km", "5:30 p.m.") | NeMo-text-processing ITN | Output keeps unit words and spoken am/pm: `5 kilometers`, `5:30 pm`. |
| Meaning that depends on more than one word on each side | A model-based ITN | The context table reads one word's class on each side. "it is ten to four" comes out `it is 10 to four`. |
| A hard latency bound on whole lines | `Engine::format` on your own windows of up to 8 tokens | `normalize` allocates, and 6 of 359 dictation lines exceed 1 µs at p99. |
| Text that already has digits | Nothing | Digits pass through; there is nothing to convert. |

## Benchmarks

| Workload | Calls | p50 | p99 | p99.9 | Allocations |
|---|---|---|---|---|---|
| `Engine::format`, generated utterances of up to 8 tokens | 10⁶ per run, 3 runs | 128 ns | 257–259 ns | 305–318 ns | 0 |
| `format_line`, dictation gold lines | 359 lines × 1000 passes | 514 ns | 947 ns | 1125 ns | ≤ 3 per line, reused `String` |

Method: release build, one P-core pinned with `taskset -c 2` on an i9-13900K, Linux 7.0, rustc 1.97; fenced TSC reads with the timer's own cost subtracted.

| Accuracy set | Lines | Exact | Digits right | Over-format |
|---|---|---|---|---|
| Dictation gold, firm lines | 292 | 96.9% | 97.6% | 3.1% (3 of 96) |
| Dictation gold, all lines | 359 | 90.3% | 92.8% | 4.2% (5 of 119) |

"Firm" leaves out lines tagged debatable (a tag ending in `!`). Over-format counts lines where the output has digits but the gold has none.

```bash
cargo build --release --bin clm-bench
taskset -c 2 target/release/clm-bench
taskset -c 2 target/release/clm-bench --lines tests/data/dictation.tsv
cargo run --release --bin clm-eval -- --lines tests/data/dictation.tsv
```

Full results, cost model and limits: [docs/PERFORMANCE.md](docs/PERFORMANCE.md).

## Contributing

Latency results on other CPUs help most, above all on AArch64: that code compiles and passes clippy but has never run on ARM hardware.

```bash
cargo install cargo-run --version 0.6.0 --locked
cargo script lint-ci        # rustfmt check, clippy -D warnings, no_std and alloc builds
cargo script test           # unit, integration and doc tests
```

> [!WARNING]
> `build.rs` compiles `src/builder/`, `src/engine/`, `src/tables/`, `src/hash.rs` and `src/types.rs` through `#[path]` modules, without crate features. A new file the builder needs must also go into its module list and its `SOURCES` array. Std-only items in those files are gated with `#[cfg(not(clm_no_std))]`, not `#[cfg(feature = "std")]`.

## Docs

- [Specification](docs/SPEC.md): the three tiers, table formats, invariants, and the latency check.
- [API](docs/API.md): every export, method, error, feature, and command.
- [Examples](docs/examples.md): runnable examples, each one a doctest.
- [Performance](docs/PERFORMANCE.md): measured latency, method, cost model, and limits.
- [Training](docs/TRAINING.md): the corpus pipeline and why the shipped tables are hand-built.

## License

Apache License 2.0, see [LICENSE](LICENSE).
