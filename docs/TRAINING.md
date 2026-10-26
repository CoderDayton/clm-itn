# Corpus Training

How the corpus pipeline learns table entries from spoken/written pairs, and why the shipped tables do not use them.

The shipped English tables are hand-built (`src/builder/english.rs`). A corpus pipeline can learn Tier 2 context rules, Tier 3 rank orders, and the shape ranking from counts, but it does not improve formatting accuracy, so `src/builder/learned.txt` ships header-only.

## Pipeline

1. `tools/pairgen/` runs NVIDIA NeMo-text-processing's English TN grammars (Apache-2.0) over public-domain text (Project Gutenberg books with the PG header and license removed; US federal works under 17 U.S.C. §105: Federal Register, National Weather Service forecast discussions). It writes aligned spoken/written span records, split 90/5/5 by sentence hash. The corpus stays outside the repository.
2. `cargo run --release --bin clm-learn -- <pairs dir> src/builder/learned.txt` counts the train split, picks cue-word context classes, writes context rules (Wilson 95% lower bound confidence, backoff pair → left → right → hand rule), Tier 3 orders, and shape counts, and reports validation and test scores.
3. `build.rs` applies `learned.txt` when it has entries. Hand rules are the floor: a learned rule overrides a hand rule only on the same side, or as a pair, with confidence at least the hand rule's.

The Google Text Normalization dataset is licensed CC BY-NC-SA 4.0 and is not used for training or shipped tables.

`text-processing-rs` (Apache-2.0, a hand-written Rust port of NeMo ITN) is not used: its TN output agrees with NeMo on 32.5% of sentences with digits, so it cannot generate pairs, and its ITN is tuned to NeMo's own test suite. Its sentence mode allocates about 700 times per token, with p99 near 80 µs per short phrase.

## Results

Run on 198,779 sentences (train 178,873, val 10,073, test 9,833) against the hand tables as they stood with 511 shapes. The hand tables have changed since; for current accuracy, run the dictation gold set (see `README.md`, Checks).

| Tables | Val exact | Test exact |
|---|---|---|
| Hand (511 shapes) | 72.6% | 71.5% |
| Learned, formatted labels only | 72.5% | 71.4% |
| Learned, with strong "stays words" rules, no learned shapes | 79.7% | 78.6% |

- Formatted labels add nothing over the hand tables. With learned shapes dropped, context and Tier 3 orders change no output.
- The whole gain of the strong variant comes from learning that a number stays as words. The corpus is mostly book prose, which spells out small numbers; that rule drops cardinal routing from 98.5% to 73.8%. Whether a lone number stays as words is a choice, set with `Setting::SingleDigits`.
- Learned shape ranking lowers validation by 8 sentences. The ranking floor is chosen by validation exact-sentence score, defaulting to the hand priors.
- The τ sweep picks 14–15, which abstains nearly every span; `TAU` stays 3.
