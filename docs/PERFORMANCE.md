# Performance

Measured latency and allocation counts for `Normalizer::normalize`, how they were taken, what a call costs, and what cannot be improved.

- [Verdict](#verdict)
- [How the numbers were taken](#how-the-numbers-were-taken)
- [Results](#results)
- [Cost model](#cost-model)
- [Limits](#limits)

## Verdict

**`Normalizer::normalize_into` meets its 1.00 µs p99 budget on utterances of up to 8 tokens: p99 905.4–908.8 ns over 10⁶ calls in each of 3 runs, about 1.1× headroom, with about one allocation per call. Whole dictation lines run longer: p99 1131–1162 ns across 3 runs on a shared machine, so the budget holds for short utterances, not for every line. A 168-word paragraph takes p50 12.9–13.5 µs in one call, about 77–80 ns per word, and p50 11.0–11.5 µs when normalized sentence by sentence.**

## How the numbers were taken

- **Machine:** Intel Core i9-13900K, one P-core pinned with `taskset -c 2`, Linux 7.0, rustc 1.97. The machine is shared (load average 3.6–3.8 during these runs), so other work ran at the same time.
- **Build:** `cargo bench` uses the `bench` profile, which inherits `release`: `opt-level = 3`, fat LTO, 16 codegen units.
- **Timer:** `lfence; rdtsc` before and `rdtscp; lfence` after each call, 0.334 ns per tick, invariant TSC. The timer's own cost (median of 10⁴ empty pairs, 20 ticks = 6.7 ns) is subtracted. The resolution gate (one tick ≤ 10 ns) passes.
- **Gate corpus:** 10⁶ generated utterances of up to 8 tokens (seed 1), mixing cardinals, ordinals, decimals, times and identifiers with non-number words, each joined into one line. An untimed pass over the whole corpus runs first, then each `normalize_into` call into one reused `String` is timed separately. The run passes with p99 ≤ 1000 ns and at most 1.05 allocations per call.
- **Allocations:** the bench installs a counting global allocator and counts every allocation in the timed loop.
- **Dictation lines:** the 359 spoken lines of `tests/data/dictation.tsv`, 1000 passes, one reused `String`. Report only.
- **Paragraph:** `tests/data/paragraph.jsonl`, one 168-word line (63 s at 160 words per minute) with times, a date, money, percents, an ordinal, a room number and step labels. Timed as one call, and with `--sentences` as its 9 sentences normalized back to back. 10⁴ passes, one reused `String`. The file carries the expected output, and a unit test checks that it matches exactly. Report only.

```bash
taskset -c 2 cargo bench --bench clm-bench                  # gate
taskset -c 2 cargo bench --bench clm-bench -- --tiers       # plus the table engine's tiers
taskset -c 2 cargo bench --bench clm-bench -- --corpus tests/data/dictation.tsv
taskset -c 2 cargo bench --bench clm-bench -- --corpus my-corpora/   # every .jsonl/.tsv/.txt file
taskset -c 2 cargo bench --bench clm-bench -- --corpus tests/data/paragraph.jsonl --passes 10000 --sentences
```

The same runs are `cargo script bench`, `cargo script bench-tiers`, `cargo script bench-corpus` and `cargo script bench-paragraph` (`--env CORPUS=<file|folder>`, default `tests/data`). Corpus file formats are in [API.md](API.md#clm-bench).

## Results

### Gate: `normalize_into`, up to 8 tokens

| Run | Core | p50      | p99      | p99.9     | max      | Allocations per call |
| --- | ---- | -------- | -------- | --------- | -------- | -------------------- |
| 1   | 2    | 491.5 ns | 908.8 ns | 1670.0 ns | 21820 ns | 1.01                 |
| 2   | 2    | 494.1 ns | 905.4 ns | 1586.5 ns | 10034 ns | 1.01                 |
| 3   | 2    | 493.5 ns | 908.8 ns | 1641.3 ns | 13350 ns | 1.01                 |

The one allocation is the line's word list.

### Table engine tiers

`--tiers` times the table engine that `normalize` calls once per window, stage by stage (one run). Stage budgets are warnings.

| Stage                          | p50     | p99      | Budget |
| ------------------------------ | ------- | -------- | ------ |
| Tier 1: lexicon lookup         | 72.1 ns | 131.5 ns | 250 ns |
| Tier 2: spans, shapes, context | 32.1 ns | 84.8 ns  | 200 ns |
| Tier 3: cell read              | 1.3 ns  | 9.3 ns   | 150 ns |
| Output assembly                | 33.4 ns | 88.8 ns  | 350 ns |

`normalize` looks each word up once while it reads the line and hands the result to the table engine, so the Tier 1 cost is not paid twice.

### Dictation lines

| Run | Core | p50      | p99       | p99.9     | max      | Lines over 1 µs at p99 |
| --- | ---- | -------- | --------- | --------- | -------- | ---------------------- |
| 1   | 2    | 539.5 ns | 1141.2 ns | 1714.1 ns | 227234 ns | 18 of 359             |
| 2   | 2    | 532.2 ns | 1130.5 ns | 1720.1 ns | 20171 ns  | 20 of 359             |
| 3   | 2    | 544.9 ns | 1161.9 ns | 1746.1 ns | 26297 ns  | 26 of 359             |

The slowest lines are long lines with several number spans or a money phrase.

| Buffer                 | Allocations over 359 lines | Most in one line |
| ---------------------- | -------------------------- | ---------------- |
| Reused `String`        | 422                        | 5                |
| New `String` each call | 881                        | 8                |

### Paragraph

| Run | Core | Input            | p50     | p99     | p99.9   | max     |
| --- | ---- | ---------------- | ------- | ------- | ------- | ------- |
| 1   | 2    | One call         | 12.9 µs | 15.1 µs | 19.7 µs | 38.8 µs |
| 1   | 2    | 9 sentences      | 11.5 µs | 13.3 µs | 16.6 µs | 27.2 µs |
| 2   | 2    | One call         | 13.5 µs | 15.9 µs | 22.0 µs | 34.9 µs |
| 2   | 2    | 9 sentences      | 11.3 µs | 13.0 µs | 15.4 µs | 21.7 µs |
| 3   | 2    | One call         | 13.2 µs | 15.9 µs | 25.1 µs | 33.5 µs |
| 3   | 2    | 9 sentences      | 11.0 µs | 13.1 µs | 17.9 µs | 28.3 µs |

Load average was 3.6–3.8. The call allocates 3 times with a reused `String` and 9 times with a new one.

## Cost model

`normalize` reads the line into a word list, looking each word up in the lexicon once. It then applies the line rules: joins, word sense, run guards and relative times. Next it plans windows of at most 8 words and runs the table engine on each window with the lookups it already has. The table engine does a fixed amount of work per window: no loops over table contents, no sorting, no allocation.

| Step              | Work                                                                                              | Table                                                | Size             |
| ----------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ---------------- |
| Lexicon           | Two bucket loads per word, then one 16-byte SIMD compare per slot                                 | Cuckoo lexicon, 1024 buckets of 4 keys, plus payload | 64 KB + 16 KB    |
| Span segmentation | One role read per word                                                                            | `role_of`                                            | 256 B            |
| Shape lookup      | One bucket load per span                                                                          | Shape table, 512 buckets                             | 32 KB            |
| Context           | One byte read per span                                                                            | Context table, 256×256                               | 64 KB            |
| Cell              | One 64-bit read per span                                                                          | Cells, 32768 × 8 B                                   | 256 KB           |
| Output            | Run the chosen candidate program (at most 15 ops) per span                                        | Candidates, 256 × 64 B per domain                    | 16 KB per domain |

Every table is `static`, read-only, and starts on a 64-byte boundary, so each bucket, record or cell is one cache-line load. No table is written at run time, so there is no false sharing between threads.

## Limits

- **The gate uses a generated corpus:** no production logs ship with the repository, so the gate replays generated utterances of up to 8 tokens. `--corpus` scores and times your own logs, but only reports.
- **Long lines can exceed 1 µs:** cost grows with line length and with the number of number spans. The budget covers short utterances.
- **Headroom:** p99 is 91% of the budget.
- **Max is not bounded:** the slowest single call ranged from 10.0 to 21.8 µs across gate runs. This is a time-shared OS, and only p99 and allocations are gated.
- **AArch64 has never run:** the NEON lookup and the `cntvct_el0` timer compile and pass clippy, but no ARM hardware has run them. On cores before Armv8.6 (Apple M1: 24 MHz, about 42 ns per tick), the counter fails the resolution gate, and the bench needs the PMU cycle counter with the core frequency locked.
- **Cache-cold calls are not measured:** the numbers come after a full warm-up pass.
