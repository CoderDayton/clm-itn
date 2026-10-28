# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `Normalizer` with one entry point, `normalize`, plus `normalize_into` for buffer reuse; `no_std` with `alloc` when default features are off.
- Cardinals, ordinals, decimals, times, dates, money, percents, measures and identifiers, read from spoken English.
- `Setting`: every rewrite beyond digits (`%`, `$`, dates, single digits, letter joins, and more) is on by default and can be switched off.
- Context cues for times: "it's", "it is" and "it was" before a clock reading; impossible times stay as spoken.
- 24-hour times ("oh seven thirty" → `07:30`, "oh eight hundred hours" → `0800 hours`) and ordinals after hundreds ("one hundred and first" → `101st`).
- Whitespace kept as written; numbers of any length.
- `clm-itn` command-line filter and `clm-learn` table builder; `clm-bench` for scoring and timing corpora.

### Performance
- 841–847 ns p99 per utterance of up to 8 tokens.
- Long lines are formatted in exact batches: 76,736 B of heap per call from 10⁴ to 10⁷ words.
- Release profile uses fat LTO with 16 codegen units.
