# CLM-ITN

Inverse text normalization for English: spoken-form tokens in, written form out.

```text
meet me at five thirty pm in room four oh two  →  meet me at 5:30 p.m. in room 402
the twenty first of june costs two thousand five dollars  →  the 21st of june costs $2005
january eighth eighteen fifteen  →  January 8, 1815
```

- `Engine::format` formats up to 8 tokens with no heap allocation, within a 1.00 µs p99 budget. `format_line`, which the CLI uses, splits a line into such calls; it allocates per line and is not covered by the latency benchmark.
- Deterministic tables, no models. User corrections reorder a span's candidate outputs through a lock-free background worker; formatting never waits on it.
- No dependencies.

Three tiers per call:

1. **Lexicon lexing:** each word → canonical number symbol + token class, by one cuckoo-table lookup.
2. **Trigram context:** the classes of the words before and after a number span pick its domain (time, identifier, cardinal…).
3. **Phrase n-gram mapping:** the span's shape and domain pick a cell holding its ranked renderings; user corrections reorder them.

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
│   └── english.rs    built-in English tables (generated)
├── engine/       the formatting hot path
│   ├── mod.rs        Engine: format, resolve, correct
│   ├── span.rs       span segmentation
│   └── window.rs     line splitting, format_line
├── learn/        corrections and learning
│   ├── cell.rs       Tier 3: candidate ranks + learning rule
│   ├── ring.rs       correction queue
│   ├── worker.rs     background worker
│   └── corrector.rs  queue + wake in one call
├── builder/      builds and checks tables (build time)
├── main.rs       CLI
└── bin/          clm-bench (latency), clm-eval (accuracy)
build.rs          generates the static English tables
docs/             SPEC.md, TEAM.md, TRAINING.md
tests/            end-to-end, drift, fuzz, static tables
```

## Use

Command line, one sentence per line:

```bash
echo "at five thirty pm" | cargo run --release --bin clm-itn
```

Library:

```rust
use clm_itn::Engine;
use clm_itn::types::{MAX_TOKENS, SpanReceipt};

let engine = Engine::new(clm_itn::english());
let tokens: [&[u8]; 4] = [b"at", b"five", b"thirty", b"pm"];
let mut out = [0u8; 256];
let mut receipts = [SpanReceipt::default(); MAX_TOKENS];
let done = engine
    .format(&tokens, &mut out, &mut receipts)
    .expect("4 tokens fit in 256 bytes");
assert_eq!(&out[..done.len], b"at 5:30 p.m.");
```

- Input is spoken-form text, one sentence per line, lowercase or cased. Digits already in the input pass through unchanged.
- Lines longer than 8 tokens: `clm_itn::format_line` splits them into windows without cutting a number. It also strips punctuation attached to number words and reattaches it ("twenty five," → `25,`). A number spoken in more than 8 words is joined only when it splits after `thousand`, `million`, `billion`, or `trillion` into pieces that each render as digits; a long number with a currency or unit word stays as words.
- Corrections: `clm_itn::Corrector` resolves a correction against the span's candidates and queues it for the worker. It needs the `SpanReceipt` and tokens from `Engine::format`; `format_line` returns no receipts, so corrections are not reachable from it. After `H` (default 4) consecutive corrections, the chosen candidate moves up one rank. Corrections only reorder built-in candidates; a target outside them returns `OutOfSet`.

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

Accuracy against NVIDIA NeMo's English ITN test cases (Apache-2.0; download `tests/nemo_text_processing/en/data_inverse_text_normalization/test_cases_*.txt` from `NVIDIA/NeMo-text-processing` into a directory):

```bash
cargo run --release --bin clm-eval -- <dir>
```

Corpus training (`tools/pairgen/` → `clm-learn`) and its results: [`docs/TRAINING.md`](docs/TRAINING.md).

## Limits

- One call covers at most 8 tokens. `format_line` joins a longer number only as described under Use; otherwise it passes through unchanged.
- Learned corrections live in memory and are lost on restart.
- The CLI replaces invalid UTF-8 bytes with U+FFFD and collapses runs of whitespace to one space. `clm-itn --help` prints usage.
- At most 2047 span shapes, so rare number patterns pass through unchanged.
- Numbers only: emails, URLs, and abbreviations such as "doctor" → "dr." are out of scope.
- The latency benchmark replays a generated corpus, not production logs.
- The aarch64 code compiles and passes clippy but has not run on ARM hardware.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
