# Performance

Measured latency and allocation counts for `Engine::format` and `format_line`, how they were taken, what each call costs, and what cannot be improved.

- [Verdict](#verdict)
- [How the numbers were taken](#how-the-numbers-were-taken)
- [Results](#results)
- [Cost model](#cost-model)
- [Limits](#limits)

## Verdict

**`Engine::format` meets its 1.00 µs p99 budget with about 4× headroom: p99 257–259 ns over 10⁶ calls in each of 3 runs, 0 allocations, 0 errors. Every tier is under its own budget. `format_line`, which handles whole lines, has p99 947 ns on the dictation gold set. It allocates and is not gated, and 6 of its 359 lines go over 1 µs at p99.**

## How the numbers were taken

- **Machine:** Intel Core i9-13900K, one P-core pinned with `taskset -c 2`, Linux 7.0, rustc 1.97, release build. The machine is shared, so other work ran at the same time.
- **Timer:** `lfence; rdtsc` before and `rdtscp; lfence` after each call, 0.334 ns per tick, invariant TSC. The timer's own cost (median of 10⁴ empty pairs, 20 ticks = 6.7 ns) is subtracted. The resolution gate (one tick ≤ 10 ns) passes.
- **Corpus:** 10⁶ generated utterances of up to 8 tokens (seed 1), mixing cardinals, ordinals, decimals, times and identifiers with non-number words. An untimed pass over the whole corpus runs first, and each call is timed separately. The run passes only with p99 ≤ 1000 ns, 0 allocations and 0 errors.
- **Allocations:** the bench installs a counting global allocator. Its count must not change across the timed loop.
- **`format_line`:** the 359 spoken lines of `tests/data/dictation.tsv`, 1000 passes, one reused `String`.

```bash
cargo build --release --bin clm-bench
taskset -c 2 target/release/clm-bench            # end to end, gated
taskset -c 2 target/release/clm-bench --tiers    # per-tier split
taskset -c 2 target/release/clm-bench --lines tests/data/dictation.tsv
```

On hybrid CPUs, pin to a performance core.

## Results

### `Engine::format`, end to end

| Run | p50 | p99 | p99.9 | max | Allocations |
|---|---|---|---|---|---|
| 1 | 128.2 ns | 257.7 ns | 307.2 ns | 5043 ns | 0 |
| 2 | 127.5 ns | 257.7 ns | 313.2 ns | 6088 ns | 0 |
| 3 | 128.2 ns | 257.1 ns | 304.5 ns | 11346 ns | 0 |

### Per tier

Each stage is timed separately (`--tiers`, one run). Stage budgets are warnings; the end-to-end p99 is the gate.

| Stage | p50 | p99 | Budget |
|---|---|---|---|
| Tier 1: lexicon lookup | 69.4 ns | 122.2 ns | 250 ns |
| Tier 2: spans, shapes, context | 27.4 ns | 80.1 ns | 200 ns |
| Tier 3: cell read | 1.3 ns | 8.7 ns | 150 ns |
| Output assembly | 32.7 ns | 86.8 ns | 350 ns |

### `format_line`

| p50 | p99 | p99.9 | max | Lines over 1 µs at p99 |
|---|---|---|---|---|
| 514.2 ns | 946.8 ns | 1124.5 ns | 681890 ns | 6 of 359 |

The slowest lines by per-line p99 run 1040–1129 ns. They are long lines with several number spans, such as "i walked one thousand two hundred thirty four steps".

| Buffer | Allocations over 359 lines | Most in one line |
|---|---|---|
| Reused `String` | 1076 | 3 |
| New `String` each call | 1483 | 7 |

## Cost model

One `format` call does a fixed amount of work for up to 8 tokens: no loops over table contents, no sorting, no allocation.

| Step | Work per call | Table | Size |
|---|---|---|---|
| Tier 1 | Two bucket loads per token, all issued before any compare, then one 16-byte SIMD compare per slot | Cuckoo lexicon, 1024 buckets of 4 keys, plus payload | 64 KB + 16 KB |
| Span segmentation | One role read per token | `role_of` | 256 B |
| Shape lookup | One bucket load per span | Shape table, 512 buckets | 32 KB |
| Tier 2 | One byte read per span | Context table, 256×256 | 64 KB |
| Tier 3 | One 64-bit read per span | Cells, 32768 × 8 B | 256 KB |
| Output | Run the chosen candidate program (at most 15 ops) per span | Candidates, 256 × 64 B per domain | 16 KB per domain |

Every table is `static`, read-only, and starts on a 64-byte boundary, so each bucket, record or cell is one cache-line load. No table is written at run time, so there is no false sharing between threads.

`format_line` adds a pass that splits and inspects words into an allocated word list, then plans windows and handles punctuation before calling `format` per window. Its p50 is about 4× a `format` call (514 ns against 128 ns).

## Limits

- **Generated corpus:** the SPEC asks for a replay of 10⁶ logged production utterances. No production logs exist, so the bench replays a generated corpus of windows of up to 8 tokens. Real dictation lines can be longer and go through `format_line`.
- **Max is not bounded:** the slowest single call ranged from 4.6 to 11.3 µs across runs. This is a time-shared OS, and only p99 is gated.
- **`format_line` is not gated:** it allocates, and its cost grows with line length and with the number of spans. For a hard bound, split lines yourself and call `format`.
- **AArch64 has never run:** the NEON lookup and the `cntvct_el0` timer compile and pass clippy, but no ARM hardware has run them. On cores before Armv8.6 (Apple M1: 24 MHz, about 42 ns per tick), the counter fails the resolution gate, and the bench needs the PMU cycle counter with the core frequency locked.
- **Cache-cold calls are not measured:** the numbers come after a full warm-up pass. Calls made after other work has evicted the tables from cache will cost more.
