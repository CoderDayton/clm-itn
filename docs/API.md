# API

Every public export of the `clm_itn` crate, its Cargo features, and its command-line tools.

- [Rules that apply everywhere](#rules-that-apply-everywhere)
- [Normalizer](#normalizer)
- [Setting](#setting)
- [Features](#features)
- [Commands](#commands)

## Rules that apply everywhere

- Input is spoken-form English, lowercase or cased. Words not in the lexicon, and digits already in the input, pass through with their original bytes and case.
- Output is deterministic: the same input and settings always give the same output.
- `normalize` cannot fail and does not panic on random lines (`tests/fuzz.rs`).
- `Setting` is `#[non_exhaustive]`, so a `match` on it needs a `_` arm.
- The crate exports `Normalizer` and `Setting`, nothing else. The `internals` feature exposes the engine for this repo's tests, bench and `clm-learn`; it is unstable and not part of the API.

## Normalizer

`Normalizer<'t>` turns spoken-form text into written form. It is a small value that borrows read-only tables. It is `Copy`, `Send` and `Sync` and takes no locks, so build it once and share it across threads. Every `Setting` starts on. `Debug` lists the settings that are off.

### Normalizer::english

```rust
pub fn english() -> Normalizer<'static>
```

A normalizer over the built-in English tables, which are compiled into the binary, with every setting on. Allocates nothing. `Normalizer::default()` is the same normalizer.

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
pub fn normalize(&self, text: &str) -> String
```

Rewrites a line of any length and returns a new `String`. It is `normalize_into` into a fresh buffer.

### Normalizer::normalize_into

```rust
pub fn normalize_into(&self, text: &str, out: &mut String)
```

Rewrites a line of any length and appends the result to `out`. Clear `out` between lines to reuse it. Behavior:

- **Windows:** the line is split into windows of at most 8 tokens. A cut never falls inside a run of number words. When a run would cross the limit, the next window starts on the word before the run, so the run keeps its left context ("room", "at").
- **Long numbers:** a number spoken in more than 8 words, or one a window cannot read whole, is read one group at a time. It is cut after each `thousand`, `million`, `billion`, `trillion`, `quadrillion` or `quintillion` that a group follows, and the groups are joined into one number ("two hundred twenty five thousand three hundred and forty second" → `225,342nd`, "twenty five thousand three hundred and forty two dollars and sixteen cents" → `$25,342 and 16 cents`). In such a run, the "and" of "hundred and five" or "thousand and five" is part of the number; "one hundred and two hundred" stays words. A scale above `billion` with no group after it prints as a word ("five trillion dollars" → `$5 trillion`). When a group does not render as digits, the whole run stays as words. A run of more than 8 digit words is split anywhere and its digits are concatenated ("one eight hundred two five five four six eight eight" → `18002554688`).
- **Thousands commas:** counts and amounts of 1,000 or more get commas (`1,500 people`, `$2,005`, `9,876,543`). Not grouped: 2000 to 2099 (read as years), "N hundred" or "N hundred and M" ending a clause or before `hours` ("in nineteen hundred" → `1900`, "in nineteen hundred and five" → `1905`, "seventeen hundred hours" → `1700 hours`), years, digit strings, times and decimals. Before a noun it is a count ("fifteen hundred and fifty people" → `1,550 people`).
- **Digit strings:** "double" and "triple" repeat the next digit word ("double oh seven" → `007`). In a run of 7 or more digit words, "hundred" stands for `00` (phone numbers). "dot" between two numbers becomes `.` ("one ninety two dot one sixty eight dot one dot one" → `192.168.1.1`).
- **Spoken forms:** "a hundred" or "a thousand" followed by more number words or a currency or unit word reads as "one" ("a hundred and twenty" → `120`, "a hundred dollars" → `$100`, "a thousand dollars" → `$1,000`); "a hundred people", "a hundred percent" and "a million dollars" stay words. "a dollar fifty" → `$1.50`. "eleven" to "nineteen" before `hundreds` gives a century ("the eighteen hundreds" → `the 1800s`). A hyphenated pair of number words reads as two words ("twenty-five" → `25`, "six forty-five a.m." → `6:45 a.m.`), and prints as written if it stays words. A digit token from `1` to `12` before a minute word reads as its word ("meet at 5 thirty pm" → `meet at 5:30 pm`). "it's", "it is" and "it was" before a clock reading work like "at": "it's five thirty", "it's 5 thirty", "it is five thirty" and "it was five thirty" → `it's 5:30`, `it is 5:30`, `it was 5:30`; "it's twenty five" → `it's 25`. "is" or "was" without "it" is no time cue ("the score is five thirty" stays words). A time with minutes over 59 or hours over 23 reads as a plain number when the words have one ("it was nineteen ninety nine" → `it was 1999`, "at twelve sixty" → `at 1260`). When they do not, it stays a time after a firm cue (`at`, or `am`, `pm` or `o'clock` in the span: "at nine sixty nine pm" → `at 9:69 pm`) and stays as spoken after `it's`, `it is` or `it was` ("it was one ninety nine"). "the fourth of july" gives `the 4th of July`. A number before a street word (`street`, `avenue`, `road`, `boulevard`, `lane`, `drive`, `court`, `place`, `parkway`, `highway`) is read on its own ("at fourteen fifty five market street" → `at 1455 market street`).
- **Punctuation:** leading `"'([{` and trailing `.,;:!?"')]}` are stripped from a lexicon word, treated as a window edge, and put back ("twenty five," → `25,`). A trailing period is not doubled after `p.m.`.
- **Lone words:** "first" and "second" stay words when alone between punctuation or line edges ("First, we go"). Two or more digit words in a row, each closed by punctuation, are a count said aloud and stay words ("One, two, three, go."); "twenty, thirty, forty" still formats.
- **Verbs:** "may" and "march" after a pronoun, `who`, `to` or a modal (`will`, `can`, `could`, `might`, `would`, `should`, `must`) are verbs and stay lowercase words ("we march five miles" → `we march 5 miles`). After such a "may", "first" and "one" stay words ("it may first be noticed", "you may one day understand").
- **Run guard:** a run of number words stays as written when a filler (`uh`, `um`, `er`, `ah`, `uhm`, `hmm`) sits inside it ("twenty uh five"), when the next word is a counting word plus `s` or `'s` ("twenty fives"), or when a digit token touches it ("twenty 5"). When the number after the filler starts with the same word as the one before it, the speaker restarted: only the part before the filler stays ("twenty uh twenty five" → `twenty uh 25`).
- **Word sense:** a lone "one" to "nine" that is not a count stays a word with every setting: "one" after a determiner or common adjective (`the`, `which`, `each`, `other`, `old`, `new`, `next`, `best`, `bottom` and others: "the old one"); "one" closing a clause or before `is`, `was`, `that`, `that's`, `which` or `who`, unless a cue word such as `step`, `number`, `option` or `at` comes before it ("Here's one.", "pick one that works", but "on step one." → `on step 1.`); "one" before `of`, `more`, `another`, `at`, `way`, `thing`, `sec`, `every` or `piece` when no number follows; "one day" opening a clause; "in one piece"; "one in a million"; "one by one"; "one another"; a number before `of us`, `of them` or `of you`; a number before a fraction word (`half`, `third`, `quarter`, and plurals such as `thirds`, `fifths`, `eighths`, `sixteenths`); "two and three quarters"; and "cloud nine", "six of one", "two heads are better than one", "eleventh hour", "one off", "twenty twenty vision". "one of five", "in one day" and "one in the morning" still give digits.
- **Relative times:** a minute word (`five`, `ten`, `twenty`, `twenty five`, `quarter`, `half`), then `to`, `till`, `past` or `after`, then an hour word (`one` to `twelve`) stays as spoken with every setting ("it is ten to four"). This applies after a time cue (`at`, `is`, `it's`, `was`, `by`, `until`, `till`, `around`, `about`) or when the phrase is a whole clause, and not when a number word such as `pm` follows the hour. Ranges such as "from ten to twelve people" still format.
- **Letters** (`Setting::Letters`): `a m` and `p m` after a number word join into `am` and `pm`. After them, three letters spelling a time zone join into its abbreviation ("seven a m e s t" → `7 am EST`). A lone letter next to a number joins it ("four k" → `4k`, "gate b twelve" → `b12`, "i sixty four" → `i64`); `a` never joins, `i` joins only before a number, and letters in a spelled run ("a b c") stay apart.
- **Percent** (`Setting::Percent`): "per cent" after a number reads as `%`.
- **Noon** (`Setting::Noon`): "twelve noon" and "twelve midnight" become `noon` and `midnight`.
- **Whitespace:** every space, tab and line break prints as written, including leading and trailing whitespace ("  twenty five  " → `  25  `). Gaps inside a span that becomes one number go with the words it replaces ("it  costs  twenty five  dollars" → `it  costs  $25`). A tab or line break (`\t`, `\n`, `\v`, `\f`, `\r`, U+0085, U+2028, U+2029) ends the context: no number, window or rule crosses it ("twenty\nfive" → `20\nfive`).

It allocates about once per call on utterances of up to 8 tokens, and at most 5 times per line with a reused `String` (8 with a new one) on the dictation gold set. The bench gates its p99 at 1.00 µs on utterances of up to 8 tokens; a 168-word paragraph takes p50 12.9–13.5 µs ([PERFORMANCE.md](PERFORMANCE.md)).

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

- `Setting::ALL` is a `&'static [Setting]` of every setting, so a new setting does not change its type.
- `name()` gives the kebab-case name, and `from_name(&str)` turns it back into a `Setting`.
- With `SingleDigits` off, ordinals ("she finished third") and numbers of two or more words ("twenty five people" → `25 people`) are unchanged. A label or identifier word before a number ("step five", "room five") still gives digits.
- A word whose setting is off (`percent`, `dollars`, `p m`, `oclock`, `fahrenheit`) is kept as spoken, and the number before it is written as a plain number.

## Features

| Feature | Effect |
|---|---|
| `std` (default) | Links `std`; builds the `clm-itn` binary and the `clm-bench` benchmark |
| none | `#![no_std]` with `alloc`; the same API |
| `internals` | Unstable. Exposes the engine, the table builder and the `clm-learn` binary for this repo's own tools |

```toml
clm-itn = { git = "https://github.com/CoderDayton/clm-itn", default-features = false } # no_std
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
cargo bench --bench clm-bench -- --corpus <file|folder> [--passes <count>] [--tag <t>] [--all-failures] [--sentences] [--no-<setting>]...
cargo bench --bench clm-bench -- --pairs <pairgen split file> [--isolated]
cargo bench --bench clm-bench -- --spell <id TAB raw TAB final file>
```

The default mode times `Normalizer::normalize_into` over a generated corpus (default 10⁶ utterances of up to 8 tokens, seed 1) and checks p99 against 1.00 µs and allocations against 1.05 per call. `--tiers` adds a per-tier split of the table engine. Exits non-zero when the gate fails. See [PERFORMANCE.md](PERFORMANCE.md).

`--corpus` reads a corpus file, or each `.jsonl`, `.tsv` and `.txt` file in a folder (sorted by name, one report per file). For each file it prints accuracy, when lines carry a written form, then `normalize_into` timing (report only):

- Accuracy: right and wrong counts, then exact, digits right, and over-format rate per tag, for all lines and for firm lines (tags not ending in `!`). `--tag` keeps one tag (and its `!` form), `--all-failures` prints every miss, and `--no-<setting>` switches a setting off.
- `--sentences` also cuts each line after every `.`, `?` or `!` that a space or the line's end follows, and times its sentences normalized back to back as one sample per line.
- `.jsonl`: one JSON object per line. The spoken text is the first string field named `spoken`, `input`, `raw`, `text` or `sentence`; the written form is `written` or `gold`; the tag is `tag` or `category`. A line without spoken text is an error that names the line.
- `.tsv` and `.txt`: `spoken [TAB written [TAB tag]]` per line. Lines starting with `#` are skipped. A line with a tab but an empty side counts as malformed.

`--pairs` scores a `tools/pairgen/` split file per semiotic class, either in sentence context or with `--isolated` spans.
- `--spell` reads `id`, `raw`, `final` columns, turns the digits in `final` back into words, and prints `spoken`, `written`, `whisper` columns. This builds a test set from text that already has digits.

### clm-learn

```text
cargo run --release --features internals --bin clm-learn -- [--plain=strong|all|off] <pairs dir with train.txt val.txt test.txt> <learned.txt out>
```

Learns context rules, Tier 3 orders and shape counts from a pair corpus. It needs the `internals` feature. See [TRAINING.md](TRAINING.md).
