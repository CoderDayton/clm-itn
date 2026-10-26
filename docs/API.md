# API

Every public export of the `clm_itn` crate, its Cargo features, and its command-line tools.

- [Rules that apply everywhere](#rules-that-apply-everywhere)
- [Normalizer](#normalizer)
- [Setting](#setting)
- [Formatted](#formatted)
- [FormatError](#formaterror)
- [format_line](#format_line)
- [Tables and english](#tables-and-english)
- [MAX_TOKENS](#max_tokens)
- [Features](#features)
- [Commands](#commands)

## Rules that apply everywhere

- Input is spoken-form English, lowercase or cased. Words not in the lexicon, and digits already in the input, pass through with their original bytes and case.
- Output is deterministic: the same input and settings always give the same output.
- `format` reports errors as `FormatError` and does not panic on random token windows (`tests/fuzz.rs`).
- `Formatted`, `Setting` and `FormatError` are `#[non_exhaustive]`. A `match` on `Setting` or `FormatError` needs a `_` arm.
- Modules not listed here (`builder`, `engine`, `tables`, `types`, `hash`) are public only for the binaries and tests and are hidden from the docs. They can change without notice.

## Normalizer

`Normalizer<'t>` turns spoken-form text into written form. It is a small value that borrows read-only tables. It is `Copy`, `Send` and `Sync` and takes no locks, so build it once and share it across threads. Every `Setting` starts on. `Debug` lists the settings that are off.

### Normalizer::english

```rust
pub fn english() -> Normalizer<'static>
```

A normalizer over the built-in English tables, which are compiled into the binary. Allocates nothing. `Normalizer::default()` is the same normalizer.

### Normalizer::new

```rust
pub fn new(tables: &'t Tables) -> Normalizer<'t>
```

A normalizer over the given tables, with every setting on. Allocates nothing.

### Normalizer::without

```rust
pub const fn without(self, setting: Setting) -> Self
```

Returns the normalizer with `setting` off.

### Normalizer::with

```rust
pub const fn with(self, setting: Setting) -> Self
```

Returns the normalizer with `setting` back on.

### Normalizer::is_on

```rust
pub const fn is_on(&self, setting: Setting) -> bool
```

Whether `setting` is on.

### Normalizer::normalize

```rust
pub fn normalize(&self, text: &str) -> String   // feature "alloc"
```

Rewrites a line of any length and returns a new `String`. It is `normalize_into` into a fresh buffer.

### Normalizer::normalize_into

```rust
pub fn normalize_into(&self, text: &str, out: &mut String)   // feature "alloc"
```

Rewrites a line of any length and appends the result to `out`. Clear `out` between lines to reuse it. Behavior:

- **Windows:** the line is split into windows of at most 8 tokens. A cut never falls inside a run of number words. When a run would cross the limit, the next window starts on the word before the run, so the run keeps its left context ("room", "at").
- **Long numbers:** a number spoken in more than 8 words is split after `thousand`, `million`, `billion` or `trillion`. The pieces are formatted alone and joined only when each renders as digits ("one hundred forty seven thousand four hundred fifty one" → `147451`). Otherwise the whole run stays as words.
- **Punctuation:** leading `"'([{` and trailing `.,;:!?"')]}` are stripped from a lexicon word, treated as a window edge, and put back ("twenty five," → `25,`). A trailing period is not doubled after `p.m.`.
- **Lone words:** "one", "first", "second" and "two" through "nine" stay words when alone between punctuation or line edges ("First, we go", "one, two, three").
- **Run guard:** a run of number words stays as written when a filler (`uh`, `um`, `er`, `ah`, `uhm`, `hmm`) sits inside it, when the next word is a counting word plus `s` or `'s` ("twenty fives"), or when a digit token touches it ("twenty 5").
- **Word sense:** a lone "one" to "nine" that is not a count stays a word with every setting: "one" after `no`, `which`, `this`, `that` or `the`; "one" before `of`, `more`, `another`, `at`, `way`, `thing` or `sec` when no number follows; "one day" opening a clause; "in one piece"; "one in a million"; "one by one"; "one another"; a number before `of us`, `of them` or `of you`; a number before a fraction word (`half`, `third`, `thirds`, `quarter`, `quarters`, `halves`); and "cloud nine", "six of one", "two heads are better than one". "one of five", "in one day" and "one in the morning" still give digits.
- **Relative times:** a minute word (`five`, `ten`, `twenty`, `twenty five`, `quarter`, `half`), then `to`, `till`, `past` or `after`, then an hour word (`one` to `twelve`) stays as spoken with every setting ("it is ten to four"). This applies after a time cue (`at`, `is`, `it's`, `was`, `by`, `until`, `till`, `around`, `about`) or when the phrase is a whole clause, and not when a number word such as `pm` follows the hour. Ranges such as "from ten to twelve people" still format.
- **Meridiem and zones** (`Setting::Letters`): `a m` and `p m` after a number word join into `am` and `pm`. After them, three letters spelling a time zone join into its abbreviation ("seven a m e s t" → `7 am EST`).
- **Noon** (`Setting::Noon`): "twelve noon" and "twelve midnight" become `noon` and `midnight`.
- **Whitespace:** runs of whitespace collapse to one space.

It allocates: at most 3 times per line with a reused `String`, at most 7 with a new one, on the dictation gold set. It is not covered by the 1.00 µs budget.

### Normalizer::format

```rust
pub fn format(&self, tokens: &[&[u8]], out: &mut [u8]) -> Result<Formatted, FormatError>
```

Rewrites up to 8 pre-split tokens into `out` with no allocation. This is the call the 1.00 µs p99 budget covers, and it is available without `alloc`.

- Tokens are words with no punctuation attached. A token such as `five,` is unknown and passes through, which can split a number.
- Tokens longer than 15 bytes skip the lexicon and pass through.
- Rewritten spans and untouched tokens are joined by single spaces.

Errors:

- `FormatError::TooManyTokens` if `tokens.len() > 8`.
- `FormatError::Overflow` if `out` is too small. Nothing is written past the end of `out`.

## Setting

```rust
pub enum Setting { SingleDigits, Percent, Currency, Dates, Letters, Noon, Oclock, Temperature }
```

A rewrite beyond writing numbers as digits. All are on by default. Derives `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq` and `Hash`.

| Setting | Name | On | Off |
|---|---|---|---|
| `SingleDigits` | `single-digits` | like 8 years | like eight years |
| `Percent` | `percent` | 50% | 50 percent |
| `Currency` | `currency` | $5, $4.50 | 5 dollars, 4 dollars and 50 cents |
| `Dates` | `dates` | July 4th, 1990 | july 4th 1990 |
| `Letters` | `letters` | 7 am EST | 7 a m e s t |
| `Noon` | `noon` | noon | 12 noon |
| `Oclock` | `oclock` | 5 o'clock | 5 oclock |
| `Temperature` | `temperature` | 72 degrees Fahrenheit | 72 degrees fahrenheit |

- `Setting::ALL` lists every setting.
- `name()` gives the kebab-case name, and `from_name(&str)` turns it back into a `Setting`.
- With `SingleDigits` off, ordinals ("she finished third") and numbers of two or more words ("twenty five people" → `25 people`) are unchanged. A label or identifier word before a number ("step five", "room five") still gives digits.
- A word whose setting is off (`percent`, `dollars`, `p m`, `oclock`, `fahrenheit`) is kept as spoken, and the number before it is written as a plain number.

## Formatted

What `Normalizer::format` wrote.

| Field | Type | Meaning |
|---|---|---|
| `len` | `usize` | Bytes written to the output buffer |
| `spans` | `usize` | Number of spans rewritten |

## FormatError

| Variant | `Display` | Cause |
|---|---|---|
| `Overflow` | `output buffer is too small` | `out` cannot hold the result |
| `TooManyTokens` | `more than 8 tokens in one call` | More than `MAX_TOKENS` tokens |

Implements `Display` and `core::error::Error`, so `?` works with `Box<dyn Error>` and `anyhow`.

## format_line

```rust
pub fn format_line(engine: &Normalizer, line: &str, out: &mut String)   // feature "alloc"
```

The free-function form of `Normalizer::normalize_into`, with the same behavior. It appends to `out`.

## Tables and english

```rust
pub fn english() -> &'static Tables
```

The built-in English tables as static data. `Tables` holds the lexicon, shape table, context table, cells and candidate programs. Pass it to `Normalizer::new`. The tables are built by `build.rs` at compile time.

## MAX_TOKENS

```rust
pub const MAX_TOKENS: usize = 8;
```

The most tokens one `Normalizer::format` call accepts.

## Features

| Feature | Adds |
|---|---|
| none | `Normalizer`, `Normalizer::format`, `Setting`, `Formatted`, `FormatError`, `Tables`, `english`, `MAX_TOKENS`. `#![no_std]`, no heap. |
| `alloc` | `Normalizer::normalize`, `Normalizer::normalize_into`, `format_line` |
| `std` (default) | `alloc`, the table builder, and the `clm-itn`, `clm-bench`, `clm-eval`, `clm-learn` binaries |

```toml
clm-itn = { git = "https://github.com/CoderDayton/clm-itn", default-features = false }                       # format only
clm-itn = { git = "https://github.com/CoderDayton/clm-itn", default-features = false, features = ["alloc"] } # plus normalize
```

## Commands

### clm-itn

```text
clm-itn [--no-<setting>]... [--help | --version]
```

Reads spoken-form text from stdin, one sentence per line, and writes the written form to stdout. Invalid UTF-8 bytes become U+FFFD. `--no-<name>` switches a setting off, for example `--no-percent`. `--help` lists every setting.

### clm-bench

```text
clm-bench [--n <count>] [--seed <u64>] [--tiers] | --lines <tsv> [--passes <count>]
```

Times `Normalizer::format` over a generated corpus (default 10⁶ utterances, seed 1) and checks p99 against 1.00 µs. `--tiers` adds a per-tier split. `--lines` times `format_line` over the first column of a TSV file and reports only. Exits non-zero when the gate fails. See [PERFORMANCE.md](PERFORMANCE.md).

### clm-eval

```text
clm-eval --lines <tsv> [--tag <t>] [--all-failures] [--no-<setting>]...
clm-eval --pairs <pairgen split file> [--isolated]
clm-eval --spell <id TAB raw TAB final file>
```

- `--lines` scores `spoken TAB written [TAB tag]` lines: exact, digits right, and over-format rate, per tag and for firm lines (tags not ending in `!`). `--tag` filters by tag, and `--all-failures` prints every miss.
- `--pairs` scores a `tools/pairgen/` split file per semiotic class, either in sentence context or with `--isolated` spans.
- `--spell` reads `id`, `raw`, `final` columns, turns the digits in `final` back into words, and prints `spoken`, `written`, `whisper` columns. This builds a test set from text that already has digits.

### clm-learn

```text
clm-learn [--plain=strong|all|off] <pairs dir with train.txt val.txt test.txt> <learned.txt out>
```

Learns context rules, Tier 3 orders and shape counts from a pair corpus. See [TRAINING.md](TRAINING.md).
