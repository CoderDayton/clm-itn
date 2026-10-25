# Examples

Every Rust block here runs as a doctest (`cargo test --doc`).

## Normalize a line

`Engine::normalize` takes any length of spoken-form text and returns a `String`. It never cuts a number and keeps punctuation in place.

```rust
use clm_itn::Engine;

let engine = Engine::english();
assert_eq!(
    engine.normalize("meet me at five thirty pm in room four oh two."),
    "meet me at 5:30 pm in room 402."
);
```

`Engine::default()` is the same engine.

To reuse one buffer, call `normalize_into`. It appends to the `String`, so clear it between lines:

```rust
use clm_itn::Engine;

let engine = Engine::english();
let mut out = String::new();
for (spoken, written) in [
    ("fifty percent off", "50% off"),
    ("born on july fourth nineteen ninety", "born on July 4, 1990"),
    ("it costs five euros", "it costs 5 euros"),
] {
    out.clear();
    engine.normalize_into(spoken, &mut out);
    assert_eq!(out, written);
}
```

## Number styles

`Style::Ap`, the default, follows AP: a lone single number stays a word. `Style::SingleDigits` writes it as a digit. Ordinals ("third") and larger numbers are the same in every style.

```rust
use clm_itn::{Engine, Style};

let ap = Engine::english();
let digits = Engine::english().with_style(Style::SingleDigits);
assert_eq!(ap.style(), Style::Ap);

assert_eq!(ap.normalize("like eight years"), "like eight years");
assert_eq!(digits.normalize("like eight years"), "like 8 years");
assert_eq!(
    digits.normalize("she finished third with twenty five points"),
    "she finished third with 25 points"
);
```

## Fast path: up to 8 tokens, no allocation

`Engine::format` takes pre-split tokens and writes into your byte buffer. It allocates nothing and is the call the 1.00 µs budget covers. `Formatted` says how many bytes were written and how many spans were rewritten.

```rust
use clm_itn::Engine;

let engine = Engine::english();
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
use clm_itn::{Engine, FormatError};

let engine = Engine::english();

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

`Engine` is `Copy`, `Send` and `Sync`. The tables are static and read-only, so build it once and hand a copy to each thread.

```rust
use clm_itn::Engine;

let engine = Engine::english();
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
clm-itn = { version = "0.1", default-features = false }                       # Engine::format only
# clm-itn = { version = "0.1", default-features = false, features = ["alloc"] }
```

| Feature | Adds |
|---|---|
| none | `Engine`, `Engine::format`, `Style`, `FormatError`. No heap. |
| `alloc` | `Engine::normalize`, `Engine::normalize_into`, `format_line` |
| `std` (default) | `alloc`, the table builder, and the `clm-itn`, `clm-bench`, `clm-eval`, `clm-learn` binaries |

## Command line

One sentence per line, from a pipe or a file:

```bash
echo "at five thirty pm" | clm-itn
# at 5:30 pm

echo "like eight years" | clm-itn --single-digits
# like 8 years

clm-itn < spoken.txt > written.txt
```

## Scoring your own data

`clm-eval` scores the engine against a tab-separated file of `spoken`, `written` and an optional tag per line:

```bash
cargo run --release --bin clm-eval -- --lines tests/data/dictation.tsv
cargo run --release --bin clm-eval -- --lines my.tsv --single-digits --all-failures
```

To build a test set from text that already has digits (for example Whisper output), `--spell` reads a file of `id`, `raw`, `final` columns, turns the digits in `final` back into words, and prints `spoken`, `written`, `whisper` columns:

```bash
cargo run --release --bin clm-eval -- --spell pairs.tsv > spelled.tsv
```
