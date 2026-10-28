<div align="center">

# CLM-ITN

Inverse text normalization for English: spoken numbers in, written numbers out.

[![CI](https://github.com/CoderDayton/clm-itn/actions/workflows/ci.yml/badge.svg)](https://github.com/CoderDayton/clm-itn/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/CoderDayton/clm-itn/blob/main/LICENSE)
![Rust 1.92+](https://img.shields.io/badge/rust-1.92%2B-orange.svg)

</div>

Three lookups over static tables, no model, no dependencies. `normalize` runs at p99 841–847 ns per utterance of up to 8 tokens, and p50 12.5 µs on a 168-word paragraph ([performance](https://github.com/CoderDayton/clm-itn/blob/main/docs/PERFORMANCE.md)).

## Features

- Cardinals, ordinals, decimals, times (12- and 24-hour), dates, money, percents, measures, and identifiers
- One call, `normalize`: any line length, keeps punctuation, never cuts a number, cannot fail
- Flat memory: 76,736 B of heap per call from 10⁴ to 10⁷ words, and 78.5–85.6 ns per word on a 10⁶-word dense line
- Settings: every rewrite beyond digits (`%`, `$`, dates, letter joins, and more) is on by default and can be switched off; single digits ("8" for "eight") are opt-in
- Deterministic static tables; `Normalizer` is `Copy`, `Send`, `Sync`
- No dependencies; `no_std` (with `alloc`) when default features are off

## Usage

```toml
[dependencies]
clm-itn = "0.1.0"
```

```rust
let normalizer = clm_itn::Normalizer::english();
assert_eq!(normalizer.normalize("at five thirty pm"), "at 5:30 pm");
```

Command line:

```bash
cargo install clm-itn

clm-itn "at five thirty pm"                 # text arguments
clm-itn -f notes.txt > written.txt          # whole files
echo "fifty percent off" | clm-itn          # pipes
clm-itn -p "fifty percent off"              # switch a setting off
clm-itn --json "room four oh two"           # JSON output
clm-itn --help
```

Every command is in [COMMANDS.md](https://github.com/CoderDayton/clm-itn/blob/main/docs/COMMANDS.md).

## Docs

- [API](https://github.com/CoderDayton/clm-itn/blob/main/docs/API.md)
- [Commands](https://github.com/CoderDayton/clm-itn/blob/main/docs/COMMANDS.md)
- [Examples](https://github.com/CoderDayton/clm-itn/blob/main/docs/examples.md)
- [Performance](https://github.com/CoderDayton/clm-itn/blob/main/docs/PERFORMANCE.md)
- [Training](https://github.com/CoderDayton/clm-itn/blob/main/docs/TRAINING.md)

## License

Apache-2.0
