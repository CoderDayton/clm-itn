# Examples

Every Rust block here runs as a doctest (`cargo test --doc`).

- [Normalize a line](#normalize-a-line)
- [Settings](#settings)
- [Share one engine across threads](#share-one-engine-across-threads)
- [Without `std`](#without-std)
- [Command line](#command-line)
- [Scoring and timing your own data](#scoring-and-timing-your-own-data)

## Normalize a line

`Normalizer::normalize` takes any length of spoken-form text and returns a `String`. It never cuts a number, keeps punctuation in place, and cannot fail.

```rust
use clm_itn::Normalizer;

let engine = Normalizer::english();
assert_eq!(
    engine.normalize("meet me at five thirty pm in room four oh two."),
    "meet me at 5:30 pm in room 402."
);
```

`Normalizer::default()` is the same engine.

24-hour times and ordinals after hundreds:

```rust
use clm_itn::Normalizer;

let engine = Normalizer::english();
for (spoken, written) in [
    ("oh seven thirty", "07:30"),
    ("twenty three forty five hours", "23:45 hours"),
    ("oh eight hundred hours", "0800 hours"),
    ("one hundred and first", "101st"),
] {
    assert_eq!(engine.normalize(spoken), written);
}
```

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
| `Letters` | 7 am EST, 4k, b12 | 7 a m e s t, 4 k, b 12 |
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

Turn off default features and the library builds with `#![no_std]`. It needs a global allocator (`alloc`); the API is the same.

```toml
[dependencies]
clm-itn = { version = "0.1", default-features = false }
```

## Command line

One sentence per line, from a pipe or a file:

```bash
echo "at five thirty pm" | clm-itn
# at 5:30 pm

echo "like eight years at fifty percent" | clm-itn --no-single-digits --no-percent
# like eight years at 50 percent

clm-itn < spoken.txt > written.txt
```

## Scoring and timing your own data

`--corpus` runs a corpus file, or every `.jsonl`, `.tsv` and `.txt` file in a folder, with one report per file: right and wrong counts when lines carry a written form, then `normalize_into` timing. A `.tsv` line is `spoken [TAB written [TAB tag]]`. A `.jsonl` line is an object; the spoken text is its first `spoken`, `input`, `raw`, `text` or `sentence` string field, the written form `written` or `gold`, the tag `tag` or `category`:

```json
{"id": 1, "spoken": "set an alarm for seven thirty am", "written": "set an alarm for 7:30 am"}
{"id": 2, "raw": "it costs twenty five dollars"}
```

```bash
cargo bench --bench clm-bench -- --corpus tests/data/dictation.tsv
cargo bench --bench clm-bench -- --corpus my.tsv --no-single-digits --all-failures
CORPUS=my-corpora/ cargo script bench-corpus
```

To build a test set from text that already has digits (for example Whisper output), `--spell` reads a file of `id`, `raw`, `final` columns, turns the digits in `final` back into words, and prints `spoken`, `written`, `whisper` columns:

```bash
cargo bench --bench clm-bench -- --spell pairs.tsv > spelled.tsv
```
