<div align="center">

# CLM-ITN

Inverse text normalization for English: spoken numbers in, written numbers out.

[![CI](https://github.com/CoderDayton/clm-itn/actions/workflows/ci.yml/badge.svg)](https://github.com/CoderDayton/clm-itn/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Rust 1.92+](https://img.shields.io/badge/rust-1.92%2B-orange.svg)

</div>

Three lookups over static tables, no model, no dependencies. `Normalizer::format` runs at p99 258 ns per call of up to 8 tokens ([performance](docs/PERFORMANCE.md)).

## Features

- Cardinals, ordinals, decimals, times, dates, money, percents, measures, and identifiers
- `Normalizer::format`: up to 8 tokens, no allocation, p99 258 ns
- `normalize`: any line length, keeps punctuation, never cuts a number
- Settings: every rewrite beyond digits (`%`, `$`, dates, single digits, and more) is on by default and can be switched off
- Deterministic static tables; `Normalizer` is `Copy`, `Send`, `Sync`
- No dependencies; `no_std` with default features off

## Usage

```toml
[dependencies]
clm-itn = { git = "https://github.com/CoderDayton/clm-itn" }
```

```rust
let engine = clm_itn::Normalizer::english();
assert_eq!(engine.normalize("at five thirty pm"), "at 5:30 pm");
```

## Docs

- [API](docs/API.md)
- [Examples](docs/examples.md)
- [Performance](docs/PERFORMANCE.md)
- [Training](docs/TRAINING.md)

## License

Apache-2.0
