# Examples

Every Rust block here runs as a doctest (`cargo test --doc`).

- [Normalize a line](#normalize-a-line)
- [Settings](#settings)
- [Fast path: up to 8 tokens, no allocation](#fast-path-up-to-8-tokens-no-allocation)
- [Errors](#errors)
- [Share one engine across threads](#share-one-engine-across-threads)
- [Without `std`](#without-std)
- [Command line](#command-line)
- [Scoring your own data](#scoring-your-own-data)

## Normalize a line

`Normalizer::normalize` takes any length of spoken-form text and returns a `String`. It never cuts a number and keeps punctuation in place.

```rust
use clm_itn::Normalizer;

let engine = Normalizer::english();
assert_eq!(
    engine.normalize("meet me at five thirty pm in room four oh two."),
    "meet me at 5:30 pm in room 402."
);
```

`Normalizer::default()` is the same engine.

To reuse one buffer, call `normalize_into`. It appends to the `String`, so clear it between lines:

```rust
use clm_itn::Normalizer;

let engine = Normalizer::english();
let mut out = String::new();
for (spoken, written) in [
    ("fifty percent off", "50% off"),
    ("born on july fourth nineteen ninety", "born on July 4th, 1990"),
    ("it costs five euros", "it costs 5 euros"),
] {
    out.clear();
    engine.normalize_into(spoken, &mut out);
    assert_eq!(out, written);
}
```

## Settings

Every rewrite beyond writing numbers as digits is a `Setting`. All of them are on by default; switch off the ones you don't want.

| Setting | On | Off |
|---|---|---|
| `SingleDigits` | like 8 years | like eight years |
| `Percent` | 50% | 50 percent |
| `Currency` | $5, $4.50 | 5 dollars, 4 dollars and 50 cents |
| `Dates` | July 4th, 1990 | july 4th 1990 |
| `Letters` | 7 am EST | 7 a m e s t |
| `Noon` | noon | 12 noon |
| `Oclock` | 5 o'clock | 5 oclock |
| `Temperature` | 72 degrees Fahrenheit | 72 degrees fahrenheit |

```rust
use clm_itn::{Normalizer, Setting};

let all = Normalizer::english();
assert_eq!(all.normalize("like eight years"), "like 8 years");

let plain = all.without(Setting::SingleDigits).without(Setting::Percent);
assert!(!plain.is_on(Setting::Percent));
assert_eq!(plain.normalize("like eight years"), "like eight years");
assert_eq!(plain.normalize("fifty percent off"), "50 percent off");
```

## Fast path: up to 8 tokens, no allocation

`Normalizer::format` takes pre-split tokens and writes into your byte buffer. It allocates nothing and is the call the 1.00 µs budget covers. `Formatted` says how many bytes were written and how many spans were rewritten.

```rust
use clm_itn::Normalizer;

let engine = Normalizer::english();
let tokens: [&[u8]; 4] = [b"at", b"five", b"thirty", b"pm"];
let mut out = [0u8; 64];

let done = engine.format(&tokens, &mut out)?;
assert_eq!(&out[..done.len], b"at 5:30 pm");
assert_eq!(done.spans, 1);
# Ok::<(), clm_itn::FormatError>(())
```

## Errors

`format` fails in two ways. `FormatError` implements `Display` and `std::error::Error`, so `?` works with `Box<dyn Error>` and with `anyhow`.

```rust
use clm_itn::{Normalizer, FormatError};

let engine = Normalizer::english();

let tokens: [&[u8]; 4] = [b"at", b"five", b"thirty", b"pm"];
let mut small = [0u8; 4];
let err = engine.format(&tokens, &mut small).unwrap_err();
assert_eq!(err, FormatError::Overflow);
assert_eq!(err.to_string(), "output buffer is too small");

let nine: [&[u8]; 9] = [b"one"; 9];
let mut out = [0u8; 64];
let err = engine.format(&nine, &mut out).unwrap_err();
assert_eq!(err, FormatError::TooManyTokens);
assert_eq!(err.to_string(), "more than 8 tokens in one call");
```

`FormatError` is `#[non_exhaustive]`, so a `match` on it needs a `_` arm.

## Share one engine across threads

`Normalizer` is `Copy`, `Send` and `Sync`. The tables are static and read-only, so build it once and hand a copy to each thread.

```rust
use clm_itn::Normalizer;

let engine = Normalizer::english();
let lines = ["at five thirty pm", "room four oh two", "fifty percent off"];

let written: Vec<String> = std::thread::scope(|s| {
    let handles: Vec<_> = lines
        .iter()
        .map(|line| s.spawn(move || engine.normalize(line)))
        .collect();
    handles.into_iter().map(|h| h.join().unwrap()).collect()
});
assert_eq!(written, ["at 5:30 pm", "room 402", "50% off"]);
```

## Without `std`

The library builds with `#![no_std]`. Turn off default features; add `alloc` to keep `normalize`, `normalize_into` and `format_line`:

```toml
[dependencies]
clm-itn = { version = "0.1", default-features = false }                       # Normalizer::format only
# clm-itn = { version = "0.1", default-features = false, features = ["alloc"] }
```

| Feature | Adds |
|---|---|
| none | `Normalizer`, `Normalizer::format`, `Setting`, `FormatError`. No heap. |
| `alloc` | `Normalizer::normalize`, `Normalizer::normalize_into`, `format_line` |
| `std` (default) | `alloc`, the table builder, and the `clm-itn`, `clm-bench`, `clm-eval`, `clm-learn` binaries |

## Command line

One sentence per line, from a pipe or a file:

```bash
echo "at five thirty pm" | clm-itn
# at 5:30 pm

echo "like eight years at fifty percent" | clm-itn --no-single-digits --no-percent
# like eight years at 50 percent

clm-itn < spoken.txt > written.txt
```

## Scoring your own data

`clm-eval` scores the engine against a tab-separated file of `spoken`, `written` and an optional tag per line:

```bash
cargo run --release --bin clm-eval -- --lines tests/data/dictation.tsv
cargo run --release --bin clm-eval -- --lines my.tsv --no-single-digits --all-failures
```

To build a test set from text that already has digits (for example Whisper output), `--spell` reads a file of `id`, `raw`, `final` columns, turns the digits in `final` back into words, and prints `spoken`, `written`, `whisper` columns:

```bash
cargo run --release --bin clm-eval -- --spell pairs.tsv > spelled.tsv
```
