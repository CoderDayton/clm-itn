# CLM-ITN

Inverse text normalization for English: spoken-form tokens in, written form out.

```text
meet me at five thirty pm in room four oh two  →  meet me at 5:30 pm in room 402
the twenty first of june costs two thousand five dollars  →  the 21st of june costs $2005
january eighth eighteen fifteen  →  January 8, 1815
```

- `Engine::format` formats up to 8 tokens with no heap allocation, within a 1.00 µs p99 budget. `format_line`, which the CLI uses, splits a line into such calls; it allocates per line and is not covered by the latency benchmark.
- Deterministic tables, no models. The tables are static data, so an `Engine` is `Sync`, takes no locks, and `Engine::new` allocates nothing.
- No dependencies, and `#![no_std]` with default features off. `Engine::format` needs no heap; the `alloc` feature adds `normalize`. See [`docs/examples.md`](docs/examples.md#without-std).

Three tiers per call:

1. **Lexicon lexing:** each word → canonical number symbol + token class, by one cuckoo-table lookup.
2. **Trigram context:** the classes of the words before and after a number span pick its domain (time, identifier, cardinal…).
3. **Phrase n-gram mapping:** the span's shape and domain pick a cell holding its ranked renderings; rank 0 is written.

The design and its invariants are in [`docs/SPEC.md`](docs/SPEC.md).

## Layout

```text
src/
├── lib.rs        modules, root re-exports
├── types.rs      shared IDs, constants, records
├── hash.rs       SPEC hash + seed generator
├── tables/       read-only data the engine reads
│   ├── bundle.rs     Tables, Aligned
│   ├── lexicon.rs    Tier 1: word → class
│   ├── shape.rs      shape key + shape table
│   ├── context.rs    Tier 2: context → domain
│   ├── candidate.rs  output programs
│   ├── cell.rs       Tier 3: cell word pack/unpack
│   └── english.rs    built-in English tables (generated)
├── engine/       the formatting hot path
│   ├── mod.rs        Engine: format, normalize
│   ├── span.rs       span segmentation
│   └── window.rs     line splitting, format_line
├── builder/      builds and checks tables (build time)
├── main.rs       CLI
└── bin/          clm-bench (latency), clm-eval (accuracy)
build.rs          generates the static English tables
docs/             SPEC.md, examples.md, TRAINING.md
tests/            public API, CLI, dictation, end-to-end, fuzz, static tables
```

## Use

Command line, one sentence per line:

```bash
echo "at five thirty pm" | cargo run --release --bin clm-itn
```

Two number styles. The default follows AP: a lone single number stays a word ("she has two cats", "eight years"). `--single-digits` also writes lone single numbers as digits ("2 cats", "8 years"), as Whisper does. In code: `Engine::english().with_style(Style::SingleDigits)`.

Library:

```rust
use clm_itn::Engine;

let engine = Engine::english();
assert_eq!(engine.normalize("meet me at five thirty pm"), "meet me at 5:30 pm");
```

`normalize` returns a `String`; `normalize_into` appends to one you own. For a hot loop with no allocation, `Engine::format` takes up to 8 pre-split tokens and writes into a byte buffer:

```rust
use clm_itn::Engine;

let engine = Engine::english();
let tokens: [&[u8]; 4] = [b"at", b"five", b"thirty", b"pm"];
let mut out = [0u8; 64];
let done = engine.format(&tokens, &mut out)?;
assert_eq!(&out[..done.len], b"at 5:30 pm");
# Ok::<(), clm_itn::FormatError>(())
```

- Input is spoken-form text, one sentence per line, lowercase or cased. Digits already in the input pass through unchanged.
- Lines longer than 8 tokens: `normalize` splits them into windows without cutting a number. It also strips punctuation attached to number words and reattaches it ("twenty five," → `25,`). A number spoken in more than 8 words is joined only when it splits after `thousand`, `million`, `billion`, or `trillion` into pieces that each render as digits; a long number with a currency or unit word stays as words.
- More library examples (lines, styles, threads, errors): [`docs/examples.md`](docs/examples.md).

## Tables

English tables are built at compile time by `build.rs` from `src/builder/` and ship as static data (`clm_itn::english()`). `builder::build_english()` builds the same tables at runtime; a test checks the two are identical.

## Checks

The scripts in `Scripts.toml` run through `cargo-run` (`cargo install cargo-run --version 0.6.0 --locked`):

```bash
cargo script lint-ci        # rustfmt check + clippy -D warnings
cargo script test           # all unit and integration tests
```

Latency (SPEC §4.1). Pin to one core; on hybrid CPUs, a performance core:

```bash
cargo build --release --bin clm-bench
taskset -c 2 target/release/clm-bench
```

Accuracy against the dictation gold set (`spoken`, `written`, optional tag per line; tags ending in `!` mark debatable lines, left out of the "firm" score):

```bash
cargo run --release --bin clm-eval -- --lines tests/data/dictation.tsv
```

Corpus training (`tools/pairgen/` → `clm-learn`) and its results: [`docs/TRAINING.md`](docs/TRAINING.md).

## Limits

- One call covers at most 8 tokens. `format_line` joins a longer number only as described under Use; otherwise it passes through unchanged.
- The CLI replaces invalid UTF-8 bytes with U+FFFD and collapses runs of whitespace to one space. `clm-itn --help` prints usage.
- At most 2047 span shapes, so rare number patterns pass through unchanged.
- Numbers only: emails, URLs, and abbreviations such as "doctor" → "dr." are out of scope.
- The latency benchmark replays a generated corpus, not production logs.
- The aarch64 code compiles and passes clippy but has not run on ARM hardware.

## License

Licensed under the Apache License, Version 2.0 (see the `LICENSE` file).
