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
- **Long numbers:** a number spoken in more than 8 words is split after `thousand`, `million`, `billion` or `trillion`. The pieces are formatted alone and joined only when each renders as digits ("one hundred forty seven thousand four hundred fifty one" → `147,451`). Otherwise the whole run stays as words. A run of more than 8 digit words is split anywhere and its digits are concatenated ("one eight hundred two five five four six eight eight" → `18002554688`).
- **Thousands commas:** counts and amounts of 1,000 or more get commas (`1,500 people`, `$2,005`, `9,876,543`). Not grouped: 2000 to 2099 (read as years), "N hundred" or "N hundred and M" ending a clause or before `hours` ("in nineteen hundred" → `1900`, "in nineteen hundred and five" → `1905`, "seventeen hundred hours" → `1700 hours`), years, digit strings, times and decimals. Before a noun it is a count ("fifteen hundred and fifty people" → `1,550 people`).
- **Digit strings:** "double" and "triple" repeat the next digit word ("double oh seven" → `007`). In a run of 7 or more digit words, "hundred" stands for `00` (phone numbers). "dot" between two numbers becomes `.` ("one ninety two dot one sixty eight dot one dot one" → `192.168.1.1`).
- **Spoken forms:** "a hundred" or "a thousand" followed by more number words or a currency or unit word reads as "one" ("a hundred and twenty" → `120`, "a hundred dollars" → `$100`, "a thousand dollars" → `$1,000`); "a hundred people", "a hundred percent" and "a million dollars" stay words. "a dollar fifty" → `$1.50`. "eleven" to "nineteen" before `hundreds` gives a century ("the eighteen hundreds" → `the 1800s`). A hyphenated pair of number words reads as two words ("twenty-five" → `25`, "six forty-five a.m." → `6:45 a.m.`), and prints as written if it stays words. A digit token from `1` to `12` before a minute word reads as its word ("meet at 5 thirty pm" → `meet at 5:30 pm`). "it's" before a clock reading works like "at": "it's five thirty" and "it's 5 thirty" → `it's 5:30`; "it's twenty five" → `it's 25`. "the fourth of july" gives `the 4th of July`. A number before a street word (`street`, `avenue`, `road`, `boulevard`, `lane`, `drive`, `court`, `place`, `parkway`, `highway`) is read on its own ("at fourteen fifty five market street" → `at 1455 market street`).
- **Punctuation:** leading `"'([{` and trailing `.,;:!?"')]}` are stripped from a lexicon word, treated as a window edge, and put back ("twenty five," → `25,`). A trailing period is not doubled after `p.m.`.
- **Lone words:** "first" and "second" stay words when alone between punctuation or line edges ("First, we go"). Two or more digit words in a row, each closed by punctuation, are a count said aloud and stay words ("One, two, three, go."); "twenty, thirty, forty" still formats.
- **Verbs:** "may" and "march" after a pronoun, `who`, `to` or a modal (`will`, `can`, `could`, `might`, `would`, `should`, `must`) are verbs and stay lowercase words ("we march five miles" → `we march 5 miles`). After such a "may", "first" and "one" stay words ("it may first be noticed", "you may one day understand").
- **Run guard:** a run of number words stays as written when a filler (`uh`, `um`, `er`, `ah`, `uhm`, `hmm`) sits inside it ("twenty uh five"), when the next word is a counting word plus `s` or `'s` ("twenty fives"), or when a digit token touches it ("twenty 5"). When the number after the filler starts with the same word as the one before it, the speaker restarted: only the part before the filler stays ("twenty uh twenty five" → `twenty uh 25`).
- **Word sense:** a lone "one" to "nine" that is not a count stays a word with every setting: "one" after `no`, `which`, `this`, `that` or `the`; "one" before `of`, `more`, `another`, `at`, `way`, `thing` or `sec` when no number follows; "one day" opening a clause; "in one piece"; "one in a million"; "one by one"; "one another"; a number before `of us`, `of them` or `of you`; a number before a fraction word (`half`, `third`, `quarter`, and plurals such as `thirds`, `fifths`, `eighths`, `sixteenths`); "two and three quarters"; and "cloud nine", "six of one", "two heads are better than one", "eleventh hour", "one off", "twenty twenty vision". "one of five", "in one day" and "one in the morning" still give digits.
- **Relative times:** a minute word (`five`, `ten`, `twenty`, `twenty five`, `quarter`, `half`), then `to`, `till`, `past` or `after`, then an hour word (`one` to `twelve`) stays as spoken with every setting ("it is ten to four"). This applies after a time cue (`at`, `is`, `it's`, `was`, `by`, `until`, `till`, `around`, `about`) or when the phrase is a whole clause, and not when a number word such as `pm` follows the hour. Ranges such as "from ten to twelve people" still format.
- **Letters** (`Setting::Letters`): `a m` and `p m` after a number word join into `am` and `pm`. After them, three letters spelling a time zone join into its abbreviation ("seven a m e s t" → `7 am EST`). A lone letter next to a number joins it ("four k" → `4k`, "gate b twelve" → `b12`, "i sixty four" → `i64`); `a` never joins, `i` joins only before a number, and letters in a spelled run ("a b c") stay apart.
- **Percent** (`Setting::Percent`): "per cent" after a number reads as `%`.
- **Noon** (`Setting::Noon`): "twelve noon" and "twelve midnight" become `noon` and `midnight`.
- **Whitespace:** runs of whitespace collapse to one space.

It allocates: at most 6 times per line with a reused `String`, at most 9 with a new one, on the dictation gold set. It is not covered by the 1.00 µs budget.

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
| `Letters` | `letters` | 7 am EST, 4k, b12 | 7 a m e s t, 4 k, b 12 |
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
| `std` (default) | `alloc`, the table builder, the `clm-itn` and `clm-learn` binaries, and the `clm-bench` benchmark |

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
cargo bench --bench clm-bench -- [--n <count>] [--seed <u64>] [--tiers]
cargo bench --bench clm-bench -- --corpus <file|folder> [--passes <count>] [--tag <t>] [--all-failures] [--no-<setting>]...
cargo bench --bench clm-bench -- --pairs <pairgen split file> [--isolated]
cargo bench --bench clm-bench -- --spell <id TAB raw TAB final file>
```

The default mode times `Normalizer::format` over a generated corpus (default 10⁶ utterances, seed 1) and checks p99 against 1.00 µs. `--tiers` adds a per-tier split. Exits non-zero when the gate fails. See [PERFORMANCE.md](PERFORMANCE.md).

`--corpus` reads a corpus file, or each `.jsonl`, `.tsv` and `.txt` file in a folder (sorted by name, one report per file). For each file it prints accuracy, when lines carry a written form, then `format_line` timing (report only):

- Accuracy: right and wrong counts, then exact, digits right, and over-format rate per tag, for all lines and for firm lines (tags not ending in `!`). `--tag` keeps one tag (and its `!` form), `--all-failures` prints every miss, and `--no-<setting>` switches a setting off.
- `.jsonl`: one JSON object per line. The spoken text is the first string field named `spoken`, `input`, `raw`, `text` or `sentence`; the written form is `written` or `gold`; the tag is `tag` or `category`. A line without spoken text is an error that names the line.
- `.tsv` and `.txt`: `spoken [TAB written [TAB tag]]` per line. Lines starting with `#` are skipped. A line with a tab but an empty side counts as malformed.

`--pairs` scores a `tools/pairgen/` split file per semiotic class, either in sentence context or with `--isolated` spans.
- `--spell` reads `id`, `raw`, `final` columns, turns the digits in `final` back into words, and prints `spoken`, `written`, `whisper` columns. This builds a test set from text that already has digits.

### clm-learn

```text
clm-learn [--plain=strong|all|off] <pairs dir with train.txt val.txt test.txt> <learned.txt out>
```

Learns context rules, Tier 3 orders and shape counts from a pair corpus. See [TRAINING.md](TRAINING.md).
