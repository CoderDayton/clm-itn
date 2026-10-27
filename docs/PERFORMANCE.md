# Performance

Measured latency, memory and allocation counts for `Normalizer::normalize`, how they were taken, what a call costs, and what cannot be improved.

- [Verdict](#verdict)
- [How the numbers were taken](#how-the-numbers-were-taken)
- [Results](#results)
- [Cost model](#cost-model)
- [Limits](#limits)

## Verdict

**`Normalizer::normalize_into` meets its 1.00 µs p99 budget on utterances of up to 8 tokens: p99 841.3–847.4 ns over 10⁶ calls in each of 3 runs, about 1.2× headroom, with about one allocation per call. Whole dictation lines run longer: p99 1005–1032 ns across 3 runs on a shared machine, so the budget holds for short utterances, not for every line. A 168-word paragraph takes p50 12.5 µs in one call, about 75 ns per word, and p50 10.5–10.6 µs when normalized sentence by sentence.**

## How the numbers were taken

- **Machine:** Intel Core i9-13900K, one P-core pinned with `taskset -c 2`, Linux 7.0, rustc 1.97. The machine is shared (load average 2.6–2.7 during these runs), so other work ran at the same time.
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
| 1   | 2    | 464.7 ns | 841.3 ns | 1188.6 ns | 13725 ns | 1.01                 |
| 2   | 2    | 468.1 ns | 847.4 ns | 1372.9 ns | 20465 ns | 1.01                 |
| 3   | 2    | 465.4 ns | 842.0 ns | 1177.9 ns | 13458 ns | 1.01                 |

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
| 1   | 2    | 506.8 ns | 1027.6 ns | 1542.5 ns | 30491 ns  | 14 of 359             |
| 2   | 2    | 500.1 ns | 1004.9 ns | 1325.5 ns | 117200 ns | 9 of 359              |
| 3   | 2    | 506.1 ns | 1032.3 ns | 1547.8 ns | 33012 ns  | 13 of 359             |

The slowest lines are long lines with several number spans or a money phrase.

| Buffer                 | Allocations over 359 lines | Most in one line |
| ---------------------- | -------------------------- | ---------------- |
| Reused `String`        | 422                        | 5                |
| New `String` each call | 881                        | 8                |

### Paragraph

| Run | Core | Input            | p50     | p99     | p99.9   | max      |
| --- | ---- | ---------------- | ------- | ------- | ------- | -------- |
| 1   | 2    | One call         | 12.5 µs | 14.6 µs | 20.3 µs | 31.9 µs  |
| 1   | 2    | 9 sentences      | 10.5 µs | 12.4 µs | 18.5 µs | 34.1 µs  |
| 2   | 2    | One call         | 12.5 µs | 14.6 µs | 20.5 µs | 29.0 µs  |
| 2   | 2    | 9 sentences      | 10.6 µs | 12.4 µs | 14.0 µs | 17.0 µs  |
| 3   | 2    | One call         | 12.5 µs | 14.7 µs | 20.4 µs | 33.5 µs  |
| 3   | 2    | 9 sentences      | 10.5 µs | 12.5 µs | 16.2 µs | 221.6 µs |

Load average was 2.6–2.7. The call allocates 3 times with a reused `String` and 9 times with a new one.

### Long lines

84,588 words of Whisper transcripts (3,645 lines, not in the repository), joined into one line with spaces, or kept one transcript per line with `\n`. The last row repeats the text 12 times. One `normalize_into` call per row, median of 5 to 200 calls, core 2, 2 runs, load average 3.0–3.5.

| Words     | One line, ns/word | One per line, ns/word |
| --------- | ----------------- | --------------------- |
| 304       | 49.4–50.2         | 46.3                  |
| 2,642     | 75.5–76.4         | 65.5–65.9             |
| 23,399    | 77.9–80.8         | 71.3–72.0             |
| 84,588    | 78.8–79.0         | 71.3–72.1             |
| 1,015,056 | 79.0–79.2         | 71.9–72.3             |

The 168-word paragraph repeated 6,000 times (1,008,000 words) as one line runs at 78.5–85.6 ns per word over 6 runs (load average 3.5–9). One call over the paragraph once runs at 74.3–77.4 ns per word.

Putting several lines in one call saves nothing: the transcripts cost 78.3–78.5 ns per word at one call per line and 76.9–79.0 ns per word at 10, 100 or 1,000 lines per call joined with `\n`.

### Memory

Peak memory of one `normalize_into` call, with `out` reserved before the call so the output is not counted. RSS is `VmHWM` after the call minus `VmRSS` before it, with the high-water mark reset (`/proc/self/clear_refs`) just before the call, one call per process, Linux and glibc. Heap is the peak of live bytes above the start of the call, from a counting global allocator. Sparse is the Whisper transcripts joined into one line; dense is the 168-word paragraph repeated as one line; per line is the same paragraph one per line. Each figure is one call.

| Words      | Sparse, RSS | Dense, RSS | Per line, RSS | Sparse, heap | Dense, heap | Per line, heap |
| ---------- | ----------- | ---------- | ------------- | ------------ | ----------- | -------------- |
| 10,000     | 0.63 MiB    | 0.63 MiB   | 0.61 MiB      | 76,736 B     | 76,736 B    | 66,176 B       |
| 100,000    | 0.70 MiB    | 0.73 MiB   | 0.65 MiB      | 76,736 B     | 76,736 B    | 66,176 B       |
| 1,000,000  | 0.68 MiB    | 0.62 MiB   | 0.65 MiB      | 76,736 B     | 76,736 B    | 66,176 B       |
| 10,000,000 | 0.71 MiB    | 0.78 MiB   | 0.69 MiB      | 76,736 B     | 76,736 B    | 66,176 B       |

Memory does not grow with the line. A line of only number words, such as "one, two, three, four, five," repeated to 800,000 words, has no place to cut and takes 137 MiB of RSS, about 180 bytes per word (2.3 MiB at 8,000 words, 15.6 MiB at 80,000). Without a reserved `out`, growing the output adds 1 to 2.4 times the output size at the peak; the output is about the size of the input, 5.3 MB per 10⁶ words.

## Cost model

`normalize` reads the line into a word list, looking each word up in the lexicon once. It then applies the line rules: joins, word sense, run guards and relative times. Next it plans windows of at most 8 words and runs the table engine with the lookups it already has on each window that holds a number word; other windows print as written. The table engine does a fixed amount of work per window: no loops over table contents, no sorting, no allocation.

A line of 256 words or more is formatted in batches, so its word list stays in cache and its memory does not grow with the line. A batch reads at least 256 words plus 16 past its cut, and starts with the 16 words before its first written word. It is cut before a word that starts a window and is not a number word, digit, filler, single letter, `dot`, `am`, `pm`, `m`, `may` or `march`. A cut inside a segment leaves at least 2 words on each side and has no `hundred` among the 5 words before it. The only chains of line rules longer than 16 words are runs of number words, fillers and digits, and the cut word is none of them, so the output is the same as one pass over the line. With 2 words of margin instead of 16, 44 of 28,436 batched runs on the random-word corpus differ from one pass; with 4 or more, none do.

| Step              | Work                                                                                              | Table                                                | Size             |
| ----------------- | ------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ---------------- |
| Lexicon           | Two bucket loads per word, then one 16-byte SIMD compare per slot                                 | Cuckoo; 295 words use 128 of 1024 buckets            | 8 KB + 2 KB read |
| Word sense        | One 32-bit read per word a line rule checks                                                       | `sense`, by canonical ID, plus 8 idiom rows          | 16 KB            |
| Span segmentation | One role read per word                                                                            | `role_of`                                            | 256 B            |
| Shape lookup      | One bucket load per span                                                                          | Shape table, 512 buckets                             | 32 KB            |
| Context           | One byte read per span                                                                            | Context table, 256×256                               | 64 KB            |
| Cell              | One 64-bit read per span                                                                          | Cells, 32768 × 8 B                                   | 256 KB           |
| Output            | Run the chosen candidate program (at most 15 ops) per span                                        | Candidates, 256 × 64 B per domain                    | 16 KB per domain |

The lexicon build starts at the fewest buckets, a power of two, that keep it at most 90% full, doubles until every word places, and lookups read only those. Every table is `static`, read-only, and starts on a 64-byte boundary, so each bucket, record or cell is one cache-line load. No table is written at run time, so there is no false sharing between threads.

## Limits

- **The gate uses a generated corpus:** no production logs ship with the repository, so the gate replays generated utterances of up to 8 tokens. `--corpus` scores and times your own logs, but only reports.
- **Long lines can exceed 1 µs:** cost grows with line length and with the number of number spans. The budget covers short utterances.
- **A line of only number words has no cut:** a spoken list or one number with no other word between its words is formatted in one pass, and its memory grows by about 180 bytes per word.
- **One call uses one core:** `normalize` does not spread a line over threads. Lines are formatted independently, so text split at line breaks can run on several threads with the same output.
- **Headroom:** p99 is 85% of the budget.
- **Max is not bounded:** the slowest single call ranged from 13.5 to 20.5 µs across gate runs. This is a time-shared OS, and only p99 and allocations are gated.
- **AArch64 has never run:** the NEON lookup and the `cntvct_el0` timer compile and pass clippy, but no ARM hardware has run them. On cores before Armv8.6 (Apple M1: 24 MHz, about 42 ns per tick), the counter fails the resolution gate, and the bench needs the PMU cycle counter with the core frequency locked.
- **Cache-cold calls are not measured:** the numbers come after a full warm-up pass.
