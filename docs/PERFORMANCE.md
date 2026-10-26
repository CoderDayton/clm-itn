# Performance

Measured latency and allocation counts for `Normalizer::format` and `format_line`, how they were taken, what each call costs, and what cannot be improved.

- [Verdict](#verdict)
- [How the numbers were taken](#how-the-numbers-were-taken)
- [Results](#results)
- [Cost model](#cost-model)
- [Limits](#limits)

## Verdict

**`Normalizer::format` meets its 1.00 µs p99 budget with about 3.8× headroom: p99 263.8–264.4 ns over 10⁶ calls in each of 3 runs, 0 allocations, 0 errors. Every tier is under its own budget. `format_line`, which handles whole lines, has p99 about 1000 ns on the dictation gold set. It allocates and is not gated, and 9–11 of its 359 lines go over 1 µs at p99.**

## How the numbers were taken

- **Machine:** Intel Core i9-13900K, one P-core pinned with `taskset -c 2`, Linux 7.0, rustc 1.97. The machine is shared, so other work ran at the same time.
- **Build:** `cargo bench` uses the `bench` profile, which inherits `release`: `opt-level = 3`, fat LTO, 16 codegen units.
- **Timer:** `lfence; rdtsc` before and `rdtscp; lfence` after each call, 0.334 ns per tick, invariant TSC. The timer's own cost (median of 10⁴ empty pairs, 20 ticks = 6.7 ns) is subtracted. The resolution gate (one tick ≤ 10 ns) passes.
- **Corpus:** 10⁶ generated utterances of up to 8 tokens (seed 1), mixing cardinals, ordinals, decimals, times and identifiers with non-number words. An untimed pass over the whole corpus runs first, and each call is timed separately. The run passes only with p99 ≤ 1000 ns, 0 allocations and 0 errors.
- **Allocations:** the bench installs a counting global allocator. Its count must not change across the timed loop.
- **`format_line`:** the 359 spoken lines of `tests/data/dictation.tsv`, 1000 passes, one reused `String`.

```bash
taskset -c 2 cargo bench --bench clm-bench                  # end to end, gated
taskset -c 2 cargo bench --bench clm-bench -- --tiers       # per-tier split
taskset -c 2 cargo bench --bench clm-bench -- --corpus tests/data/dictation.tsv
taskset -c 2 cargo bench --bench clm-bench -- --corpus my-corpora/   # every .jsonl/.tsv/.txt file
```

The same runs are `cargo script bench`, `cargo script bench-tiers` and `cargo script bench-corpus` (`--env CORPUS=<file|folder>`, default `tests/data`). Corpus file formats are in [API.md](API.md#clm-bench).

On hybrid CPUs, pin to a performance core that no other busy process shares.

## Results

### `Normalizer::format`, end to end

| Run | p50      | p99      | p99.9    | max      | Allocations |
| --- | -------- | -------- | -------- | -------- | ----------- |
| 1   | 130.9 ns | 264.4 ns | 325.2 ns | 13451 ns | 0           |
| 2   | 130.2 ns | 264.4 ns | 319.2 ns | 5039 ns  | 0           |
| 3   | 130.2 ns | 263.8 ns | 337.9 ns | 4352 ns  | 0           |

### Per tier

Each stage is timed separately (`--tiers`, one run). Stage budgets are warnings; the end-to-end p99 is the gate.

| Stage                          | p50     | p99      | Budget |
| ------------------------------ | ------- | -------- | ------ |
| Tier 1: lexicon lookup         | 70.1 ns | 126.2 ns | 250 ns |
| Tier 2: spans, shapes, context | 26.7 ns | 78.1 ns  | 200 ns |
| Tier 3: cell read              | 0.7 ns  | 8.7 ns   | 150 ns |
| Output assembly                | 32.7 ns | 84.8 ns  | 350 ns |

### `format_line`

| Run | p50      | p99       | p99.9     | max      | Lines over 1 µs at p99 |
| --- | -------- | --------- | --------- | -------- | ---------------------- |
| 1   | 629.0 ns | 1230.6 ns | 1928.4 ns | 16679 ns | 58 of 359              |
| 2   | 635.7 ns | 1300.7 ns | 2287.7 ns | 28803 ns | 95 of 359              |

Runs taken while another busy process shared the pinned core gave p99 1251–1400 ns, so check the core is idle before comparing numbers. The slowest lines are long lines with several number spans or a money phrase, such as "the bill came to eighty seven dollars and twenty cents" and "i walked one thousand two hundred thirty four steps".

| Buffer                 | Allocations over 359 lines | Most in one line |
| ---------------------- | -------------------------- | ---------------- |
| Reused `String`        | 1104                       | 6                |
| New `String` each call | 1560                       | 9                |

## Cost model

One `format` call does a fixed amount of work for up to 8 tokens: no loops over table contents, no sorting, no allocation.

| Step              | Work per call                                                                                     | Table                                                | Size             |
| ----------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ---------------- |
| Tier 1            | Two bucket loads per token, all issued before any compare, then one 16-byte SIMD compare per slot | Cuckoo lexicon, 1024 buckets of 4 keys, plus payload | 64 KB + 16 KB    |
| Span segmentation | One role read per token                                                                           | `role_of`                                            | 256 B            |
| Shape lookup      | One bucket load per span                                                                          | Shape table, 512 buckets                             | 32 KB            |
| Tier 2            | One byte read per span                                                                            | Context table, 256×256                               | 64 KB            |
| Tier 3            | One 64-bit read per span                                                                          | Cells, 32768 × 8 B                                   | 256 KB           |
| Output            | Run the chosen candidate program (at most 15 ops) per span                                        | Candidates, 256 × 64 B per domain                    | 16 KB per domain |

Every table is `static`, read-only, and starts on a 64-byte boundary, so each bucket, record or cell is one cache-line load. No table is written at run time, so there is no false sharing between threads.

`format_line` adds a pass that splits and inspects words into an allocated word list, then plans windows, applies the run, word-sense and relative-time guards, and handles punctuation before calling `format` per window. Its p50 is about 4× a `format` call (518 ns against 130 ns).

## Limits

- **The gate uses a generated corpus:** no production logs ship with the repository, so the gate replays generated windows of up to 8 tokens. Real dictation lines can be longer and go through `format_line`; `--corpus` scores and times your own logs that way, but only reports.
- **Max is not bounded:** the slowest single call ranged from 4.4 to 13.5 µs across runs. This is a time-shared OS, and only p99 is gated.
- **`format_line` is not gated:** it allocates, and its cost grows with line length and with the number of spans. For a hard bound, split lines yourself and call `format`.
- **AArch64 has never run:** the NEON lookup and the `cntvct_el0` timer compile and pass clippy, but no ARM hardware has run them. On cores before Armv8.6 (Apple M1: 24 MHz, about 42 ns per tick), the counter fails the resolution gate, and the bench needs the PMU cycle counter with the core frequency locked.
- **Cache-cold calls are not measured:** the numbers come after a full warm-up pass. Calls made after other work has evicted the tables from cache will cost more.
