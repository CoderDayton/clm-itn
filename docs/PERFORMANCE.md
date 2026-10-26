# Performance

Measured latency and allocation counts for `Normalizer::normalize`, how they were taken, what a call costs, and what cannot be improved.

- [Verdict](#verdict)
- [How the numbers were taken](#how-the-numbers-were-taken)
- [Results](#results)
- [Cost model](#cost-model)
- [Limits](#limits)

## Verdict

**`Normalizer::normalize_into` meets its 1.00 µs p99 budget on utterances of up to 8 tokens: p99 826.7–886.8 ns over 10⁶ calls in each of 6 runs, about 1.1–1.2× headroom, with about one allocation per call. Whole dictation lines run longer: p99 935–1205 ns across 5 runs on a shared machine, so the budget holds for short utterances, not for every line.**

## How the numbers were taken

- **Machine:** Intel Core i9-13900K, one P-core pinned with `taskset -c 2` or `-c 4`, Linux 7.0, rustc 1.97. The machine is shared (load average about 3 during these runs), so other work ran at the same time.
- **Build:** `cargo bench` uses the `bench` profile, which inherits `release`: `opt-level = 3`, fat LTO, 16 codegen units.
- **Timer:** `lfence; rdtsc` before and `rdtscp; lfence` after each call, 0.334 ns per tick, invariant TSC. The timer's own cost (median of 10⁴ empty pairs, 20 ticks = 6.7 ns) is subtracted. The resolution gate (one tick ≤ 10 ns) passes.
- **Gate corpus:** 10⁶ generated utterances of up to 8 tokens (seed 1), mixing cardinals, ordinals, decimals, times and identifiers with non-number words, each joined into one line. An untimed pass over the whole corpus runs first, then each `normalize_into` call into one reused `String` is timed separately. The run passes with p99 ≤ 1000 ns and at most 1.05 allocations per call.
- **Allocations:** the bench installs a counting global allocator and counts every allocation in the timed loop.
- **Dictation lines:** the 359 spoken lines of `tests/data/dictation.tsv`, 1000 passes, one reused `String`. Report only.

```bash
taskset -c 2 cargo bench --bench clm-bench                  # gate
taskset -c 2 cargo bench --bench clm-bench -- --tiers       # plus the table engine's tiers
taskset -c 2 cargo bench --bench clm-bench -- --corpus tests/data/dictation.tsv
taskset -c 2 cargo bench --bench clm-bench -- --corpus my-corpora/   # every .jsonl/.tsv/.txt file
```

The same runs are `cargo script bench`, `cargo script bench-tiers` and `cargo script bench-corpus` (`--env CORPUS=<file|folder>`, default `tests/data`). Corpus file formats are in [API.md](API.md#clm-bench).

On hybrid CPUs, pin to a performance core that no other busy process shares.

## Results

### Gate: `normalize_into`, up to 8 tokens

| Run | Core | p50      | p99      | p99.9     | max      | Allocations per call |
| --- | ---- | -------- | -------- | --------- | -------- | -------------------- |
| 1   | 4    | 456.7 ns | 832.0 ns | 1123.1 ns | 15749 ns | 1.01                 |
| 2   | 4    | 461.4 ns | 845.4 ns | 1463.7 ns | 6608 ns  | 1.01                 |
| 3   | 4    | 457.4 ns | 832.7 ns | 1105.1 ns | 16920 ns | 1.01                 |
| 4   | 2    | 455.4 ns | 828.7 ns | 1157.9 ns | 23520 ns | 1.01                 |
| 5   | 2    | 472.1 ns | 886.8 ns | 1488.4 ns | 32794 ns | 1.01                 |
| 6   | 2    | 455.4 ns | 826.7 ns | 1095.8 ns | 10455 ns | 1.01                 |

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
| 1   | 4    | 485.4 ns | 935.5 ns  | n/a       | n/a      | 7 of 359               |
| 2   | 4    | 489.4 ns | 960.2 ns  | 1501.1 ns | 20652 ns | 8 of 359               |
| 3   | 4    | 498.8 ns | 1082.4 ns | 1767.5 ns | 12912 ns | 129 of 359             |
| 4   | 2    | 462.7 ns | 1204.6 ns | 2657.6 ns | 13936 ns | 227 of 359             |
| 5   | 2    | 494.8 ns | 967.5 ns  | 1502.4 ns | 13259 ns | 15 of 359              |

Runs 3 and 4 shared the pinned core with other work, so check the core is idle before comparing numbers. The slowest lines are long lines with several number spans or a money phrase.

| Buffer                 | Allocations over 359 lines | Most in one line |
| ---------------------- | -------------------------- | ---------------- |
| Reused `String`        | 421                        | 5                |
| New `String` each call | 876                        | 8                |

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
- **Headroom is thin:** p99 sits at 83–89% of the budget, so a busy core can push a run over it.
- **Max is not bounded:** the slowest single call ranged from 6.6 to 32.8 µs across runs. This is a time-shared OS, and only p99 and allocations are gated.
- **AArch64 has never run:** the NEON lookup and the `cntvct_el0` timer compile and pass clippy, but no ARM hardware has run them. On cores before Armv8.6 (Apple M1: 24 MHz, about 42 ns per tick), the counter fails the resolution gate, and the bench needs the PMU cycle counter with the core frequency locked.
- **Cache-cold calls are not measured:** the numbers come after a full warm-up pass. Calls made after other work has evicted the tables from cache will cost more.
