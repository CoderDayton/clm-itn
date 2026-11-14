# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `Normalizer` with one entry point, `normalize`, plus `normalize_into` for buffer reuse; `no_std` with `alloc` when default features are off.
- Cardinals, ordinals, decimals, times, dates, money, percents, measures and identifiers, read from spoken English.
- `Setting`: every rewrite beyond digits (`%`, `$`, dates, letter joins, and more) is on by default and can be switched off. `SingleDigits` is the exception: a lone "eight" stays a word until you turn it on.
- Context cues for times: "it's", "it is" and "it was" before a clock reading; impossible times stay as spoken.
- 24-hour times ("oh seven thirty" → `07:30`, "oh eight hundred hours" → `0800 hours`) and ordinals after hundreds ("one hundred and first" → `101st`).
- Whitespace kept as written; numbers of any length.
- `clm-itn` command line: text arguments, `-f <FILE>`, or a stdin filter; `--json` output; `--no-<setting>` and `--single-digits` flags with combinable short forms (`-dp`); help when run bare in a terminal.
- `clm-learn` table builder and `clm-bench` for scoring and timing corpora.
- A `Language` definition the table builder reads; the engine takes every word it matches or writes from the tables. Built tables can be saved to and loaded from bytes (`internals`).
- Spanish, Portuguese and German: `Normalizer::spanish`, `Normalizer::portuguese` and `Normalizer::german` (`german` feature, on by default), each with its own tables and line rules.
- `clm-bench --accuracy <lang>[+digits]:<path>`: exact and value accuracy per language and their average; corpus files also read `spoken~written` lines.

### Performance
- 841–847 ns p99 per utterance of up to 8 tokens.
- Long lines are formatted in exact batches: 76,736 B of heap per call from 10⁴ to 10⁷ words.
- Release profile uses fat LTO with 16 codegen units.
