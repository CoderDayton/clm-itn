# CLM-ITN Specification (v1.1.0)

### Contextual Lexical Matrix for Inverse Text Normalization

CLM-ITN is a high-performance, zero-dependency, allocation-free Inverse Text Normalization framework designed for systems-level text formatting within a strict **1.00 µs p99 latency ceiling**. 

The architecture avoids non-deterministic generative models and complex pointer-chasing graphs. Instead, it relies on a hardware-sympathetic three-tier execution pipeline paired with a lock-free, asynchronous adaptive feedback loop.

### Pipeline Layers

*   **Tier 1 (Lexicon Lexing):** Maps each spoken token to a canonical numeric symbol and a contextual token class with one cuckoo-table lookup. A variable-length string comparison becomes one fixed 16-byte SIMD compare against at most 8 keys.
*   **Tier 2 (Trigram Context Interception):** For each numeric span, reads the class IDs of the token before and the token after the span. The (left class, span, right class) trigram (N = 3) selects the span's semantic domain (e.g., Time vs. Identifier) from a frozen 256×256 table. Low-confidence entries fall back to a fixed default.
*   **Tier 3 (Adaptive Phrase N-Gram Mapping):** Each span is a phrase n-gram of 1–8 tokens, keyed by its shape (the sequence of its token roles). The (domain, shape) pair selects an isolated cell, which holds that phrase's rendering candidates in rank order plus correction streak counters. User corrections reorder the ranks; rank 0 is the rendering used.

Tier 2 and Tier 3 are n-gram tables over token classes and roles, not statistical language models: every entry is a fixed choice, so the same input and cell state always give the same output.

---

## 1. Architectural Pipeline & Latency Budgets

The hot path processes streaming tokens in a linear sequence using preallocated arenas. Total budget: **1.00 µs at p99 for up to 8 tokens**.

**Budget Unit:** Every budget in this document is per call. One call processes one window of up to 8 tokens. Tier budgets are totals over all tokens and spans in the window, not per-token costs. Tier 1 meets its budget by issuing the bucket loads for every token in the window (up to 16) before comparing any of them, so their cache misses overlap instead of adding up.

```text
[Spoken Tokens] ──> Tier 1: Cuckoo Lexicon (0.25 µs)              ──> Canonical IDs & Classes
                ──> Tier 2: Trigram Context (0.20 µs)           ──> Shape & Domain per Span
                ──> Tier 3: Phrase N-Gram Matrix (0.15 µs)       ──> Candidate ID per Span
                ──> Output Assembly & Slack (0.35 µs)            ──> Formatted Target String
```

### Memory Isolation Invariant
*   Every table starts on a 64-byte boundary (`#[repr(align(64))]`), so each bucket, record, or cell group is one cache-line load.
*   The Tier 1 lexicon, role table, shape table, Tier 2 LUT, and candidate table are read-only after init. Nothing writes them, so they cannot false-share.
*   Tier 3 is the only table the worker writes. Cells are packed 8 per 64-byte line. A worker store invalidates that line in the reader's cache, so the next read of any of its 8 cells misses once. Stores happen once per accepted correction, so this cost is accepted rather than paying 8× memory for one cell per line.
*   The SPSC ring's head index and tail index each sit alone on a 128-byte-aligned block. Both threads touch them on every event, and Intel's adjacent-line prefetcher fetches lines in 128-byte pairs.

---

## 2. Detailed Component Specifications

**Table Build:** An offline table builder produces every frozen table and the initial Tier 3 cells, which ship as static data. The builder checks every invariant in this section and fails on any violation. This spec fixes table formats and invariants, not how table contents (such as Tier 2 confidence bands) are derived.

### Tier 1: Bucketed Cuckoo Lexicon
*   **Latency Budget:** 0.25 µs for the window (two bucket loads per token, issued together for all tokens; the payload read after a match hits L1/L2).
*   **Data Structure:** Two-choice bucketed Cuckoo Hash Table consisting of 4096 contiguous slots.
*   **Layout:** 
    *   1024 buckets of 64 bytes (64 KB). Each bucket holds 4 exact 16-byte keys: the token's first 15 bytes after case folding, zero-padded, with the token length in byte 15. The key is the whole token, so a key match is exact and needs no verification.
    *   A parallel payload array of 4096 `u32` (16 KB) holds each slot's `u16` canonical token ID and `u8` class. It is read only after a match.
    *   Lexicon entries are at most 15 bytes; the table build rejects longer ones. Input tokens longer than 15 bytes skip lookup and pass through.
*   **Case Folding:** ASCII `A–Z` fold to `a–z`. All other bytes compare as-is.
*   **Hashing:** Split the key into `lo` and `hi` (`u64`). With `K1 = 0x9E3779B97F4A7C15` and `K2 = 0xC2B2AE3D27D4EB4F`:

```rust
let a = (lo ^ seed).wrapping_mul(K1).rotate_left(31);
let b = (hi ^ seed.rotate_left(32)).wrapping_mul(K2);
let mut h = (a ^ b).wrapping_mul(K1);
h ^= h >> 32;
let b1 = (h as usize) & (BUCKETS - 1);
let mut b2 = ((h >> 32) as usize) & (BUCKETS - 1);
if b2 == b1 { b2 ^= 1; }
```

*   **Seed:** The table build picks a 64-bit `seed` for which every entry inserts, and ships it with the table. The table never rehashes at runtime.
*   **Reserved Classes:** Class `0` is `BOUNDARY`, the class of any neighbor position outside the window. Class `1` is `UNKNOWN`, the class of any passthrough token. Lexicon entries use classes `2–255`. Both reserved classes have numeric role `0`, so they never join a span.
*   **Lookup Method:**
    1. Fold and pack the token into its 16-byte key, then compute `b1` and `b2`.
    2. Load both buckets. Across the window, issue every token's bucket loads before any compare.
    3. Compare the key with each of the bucket's 4 keys using 128-bit SIMD byte equality: `_mm_cmpeq_epi8` + `_mm_movemask_epi8` (SSE2, baseline on x86-64) or `vceqq_u8` (NEON, baseline on AArch64). A slot matches when all 16 bytes are equal. No runtime feature detection is needed.
    4. On a match, read the slot's payload and return the `u16` Canonical ID and the `u8` Class code. If both lookups miss, forward the token unchanged as a passthrough string with class `UNKNOWN` (1).

### Span Segmentation & Shape ID
*   **Latency Budget:** Counted inside Tier 2's 0.20 µs. All tables below are frozen and fit in L1/L2.
*   **Numeric Role:** A frozen 256-byte table `role_of[class]` maps each class to a 4-bit role. Role `0` means not numeric. Roles `1–15` name the token's part in a number (e.g., `1: Digit` for zero–nine and "oh", `2: Teen`, `3: Tens`, `4: Scale` for hundred/thousand, `5: Point`, `6: Connector` for "and"). The table build fixes the full role list.
*   **Span:** A maximal run of tokens with nonzero role, broken before any token whose class is in the frozen 256-bit set `starts` (month names), so "sixty seven january seventeenth" is two spans (`67 January 17`). Each span is one unit of work for Tier 2 and Tier 3. Tokens outside spans pass through unchanged.
*   **Shape Key:** `key = Σ role[i] << (4 * i)` over the span's tokens, as a `u32`. Roles are nonzero, so the key encodes both span length and role sequence with no collisions.
*   **Shape ID:** A frozen two-choice bucketed table, using Tier 1's hash with `lo = key`, `hi = 0`, and its own seed, maps the key to an 11-bit Span Shape ID (`u16`, at most 2047 shapes). Each 64-byte bucket holds 8 exact `u32` keys and 8 `u16` IDs; exact keys need no verification. Size: 512 buckets (32 KB).
*   **Affix Split:** Two frozen 16-bit masks, `suffix_roles` and `prefix_roles`, mark roles that act as affixes (e.g. currency and unit words as suffixes, "minus" as a prefix). If a span ends in a suffix role, up to its last 2 suffix-role tokens leave the shape key, keeping at least 1 core token; the class of the first stripped token becomes the span's right context (e.g. "five square kilometers": core "five", right context "square"). Then, if the remaining core has length ≥ 2 and starts with a prefix role, that first token leaves the key and its class becomes the left context. The shape is looked up on the core key only, and `shape_len[shape]` must equal the core length, or the span takes `UNKNOWN_SHAPE`. The span itself (its tokens, receipt, and candidate input) still covers the affix tokens. This lets money, percent, negative, and measure spans reuse the plain number shapes.
*   **Prefix Literal:** If a prefix token was stripped and the chosen candidate's `span_len` is exactly one less than the span's length, the engine writes the literal `value_of[prefix token]` (e.g. "minus" → `-`) and then renders the candidate over the remaining tokens ("minus sixty six kilograms" → `-66 kg`).
*   **Month Prefix:** Month names are a literal class with the prefix role; their `value_of` is the capitalized name with a trailing space ("january" → `January `). The structural row left = month routes to domain 13 (date with month) at confidence 15, with `affix_len` 1. Date-shaped cores (an ordinal day in the first 2 tokens, or a year) rank: r0 `January 8, 1815` (the comma only before a year), r1 the spoken form `january 8 1815`, r2 `January 8 1815`, r3 the cardinal reading. Other cores fall back to the prefix literal plus the cardinal reading ("july two thousand twelve" → `July 2012`). "may" has its own class with the lowercase literal `may ` and domain 14, whose date candidates write `May `; fallbacks keep the lowercase verb ("you may go", "may five hundred" → `may 500`).
*   **Length Guard:** Otherwise, if the chosen candidate's `span_len` differs from the span's length, the span passes through unchanged. This covers spans the build invariant cannot: two affixes at once, an abstaining affix span (whose `cardinal_default` has no affix), and a context row that routes an affix span to a domain without affixes. The check replaces the `NONE` test (`NONE` has `span_len` 0) and reads only the candidate record that rendering reads anyway.
*   **Unknown Shapes:** Shape ID `0` is `UNKNOWN_SHAPE`. A span whose key is not in the table passes through unchanged and skips Tiers 2–3.
*   **Example:** "five thirty" → roles `[1, 3]` → key `0x31` → the shape ID of the Digit+Tens shape.

### Tier 2: Contextual Class Matrix
*   **Latency Budget:** 0.20 µs for the window, covering span segmentation, shape lookup, and one LUT read per span (up to 8). All three tables are L1/L2-resident.
*   **Data Structure:** A dense, frozen 256x256 `u8` Look-Up Table (LUT) representing all possible token-class configurations.
*   **Layout:** Flat, contiguous memory block (64 KB) completely resident in L2 cache.
*   **Lookup Method:**
    1. Read the `u8` Class codes of the token just before the span and the token just after it. A position outside the window reads as `BOUNDARY` (0).
    2. Index the table directly: `let entry = lut[((left_class as usize) << 8) | right_class as usize];` Widen before shifting: `u8 << 8` overflows and panics in debug builds. With `lut: [u8; 65536]` the index is always in bounds, so the bounds check compiles away.
    3. Extract bits branchlessly:
        *   `Bits 0-3`: Active `NumberDomain` ID: `0` Cardinal, `1` Time, `2` Identifier, `3` Ordinal, `4` Date, `5` Plain (number words that stay words), `6` Money, `7` Cents, `8` Percent, `9` Negative, `10` Measure, `11` Measure with power word ("square", "cubic"), `12` Measure with prefix word ("kilo", "mega"), `13` Date with month, `14` Date with "may"; `15` reserved.
        *   `Bits 4-7`: Quantized Confidence Band (Values `0-15`).
    4. **Affix Rows:** Context rows whose class is an affix are fixed by structure, at confidence 15: right = currency → Money, right = cent → Cents, right = percent → Percent, otherwise left = minus → Negative. They take precedence over learned rows, because an affix class appears as context only through the affix split.
    5. **Abstention Gate:** If the confidence band is below a fixed margin threshold ($\tau = 3$), the span uses `cardinal_default[shape_id]`, a frozen 2048-byte table of Candidate IDs (one per shape), instead of its Tier 3 cell. The engine computes both choices and selects with a mask, so the gate has no branch. Learning never changes this path: the span's receipt carries `cell = 0xFFFF`, and a correction against it returns `Abstained` and pushes nothing.
    6. **Words-First Rule:** `words_first` is a frozen 4096-bit set (512 bytes) indexed by canonical ID. When a span is one token long, its token's bit is set, its domain is Cardinal or Ordinal or it abstained (or Time, when the token's role is in the frozen `words_first_roles` mask, which holds the ordinal role, so "at first sight" stays words while "at five" → `at 5`), and at least one neighbor is a word rather than a window boundary, the span routes to the Plain domain without abstaining. The Plain cell ranks `RAW` first, so the word stays as written ("this one only"), and the receipt names a real cell, so corrections still apply. The English set is "one", "first", "second" and "two" through "nine"; a lexicon unit, currency, scale, or other number word next to the token makes the span longer than one token, so those keep formatting ("five meters" → `5 m`, "five dollars" → `$5`). A noun outside the lexicon is a plain word, so "five days" stays as written. The engine applies it with a mask, so the rule has no branch.

### Tier 3: Adaptive Permutation Matrix
*   **Latency Budget:** 0.15 µs (Dominated by a single cache-line fetch).
*   **Data Structure:** Flat direct-indexed array of 32768 cells (256 KB), indexed by `cell = (domain_id << 11) | shape_id` (4-bit domain, 11-bit shape).
*   **Layout:**
    *   Each entry is a single atomic 64-bit word (`AtomicU64`) representing a frozen structural candidate cell.
    *   The 64 bits are tightly bit-packed as follows:
        *   `Bits 0-7`:   `u8` Candidate ID at Rank 0 (The active hot path choice).
        *   `Bits 8-15`:  `u8` Candidate ID at Rank 1.
        *   `Bits 16-23`: `u8` Candidate ID at Rank 2.
        *   `Bits 24-31`: `u8` Candidate ID at Rank 3.
        *   `Bits 32-35`: 4-bit saturation streak counter for Candidate at Rank 0.
        *   `Bits 36-39`: 4-bit saturation streak counter for Candidate at Rank 1.
        *   `Bits 40-43`: 4-bit saturation streak counter for Candidate at Rank 2.
        *   `Bits 44-47`: 4-bit saturation streak counter for Candidate at Rank 3.
        *   `Bits 48-63`: **Unused Slack Space (Reserved / Forced Zero)**.
    *   Candidate ID `0` is `NONE`. It fills unused ranks when a shape has fewer than 4 valid candidates. It is never at Rank 0 and never promoted.
*   **Lookup Method:**
    1. Execute a single `AtomicU64::load(Relaxed)` to fetch the packed cell word.
    2. **Branchless Output Selection:** The active formatting choice is always Rank 0. The hot path extracts this byte directly via a bitwise mask: `let candidate_id = (packed_word & 0xFF) as u8;` This completely skips runtime sorting or comparison loops.

### Candidate Table & Output Assembly
*   **Latency Budget:** The 0.35 µs Output Assembly slot.
*   **Data Structure:** One frozen array of 256 candidate records per domain, indexed by (domain, Candidate ID). A cell's Candidate IDs index its own domain's array; `cardinal_default` IDs index the Cardinal array. The literal pool is shared by all domains. A candidate is a formatting program, not a string: one Time candidate renders "five thirty" as `5:30` and "six forty" as `6:40`.
*   **Record Layout:**

```rust
#[repr(C, align(64))]
struct Candidate {
    span_len: u8,   // tokens this program consumes
    op_count: u8,   // ops used, 0-15
    _pad: [u8; 2],
    ops: [u32; 15], // opcode u8 + three u8 operands
}
```

*   **Token Values:** A frozen table `value_of[canonical_id]` gives each lexicon token its `u32` numeric value.
*   **Execution:** The program reads span tokens left to right into four `u64` field registers and writes bytes to the caller's output buffer. A failed program writes the span's original text (as `RAW` over the whole span) instead of its output; a buffer overflow still returns `Overflow`.

| Op | Operands | Effect |
|---|---|---|
| `FOLD` | field, n | Read n tokens into the field by cardinal grammar: add units, multiply by scales. Adding a value Q to the running group P requires P mod 10^digits(Q) = 0 (place-value rule; "fifty fifty" and "one fifth" fail); "hundred" multiplies the group, scales ≥ 1000 close it. A failure fails the program. |
| `DIGITS` | field, n | Read n tokens and append each value as decimal digits ("one two three" → `123`). |
| `EMIT` | field, width | Write the field in decimal, zero-padded to width. |
| `LIT` | literal ID | Write an entry from a frozen literal pool (≤ 255 entries, each ≤ 15 bytes, e.g. `:`, `.`, ` PM`). |
| `ORD` | field | Write the ordinal suffix for the field's value (`st`, `nd`, `rd`, `th`). |
| `RAW` | n | Read n tokens and write their original text unchanged. |
| `SKIP` | n | Read n tokens and write nothing (e.g., the spoken word "point" in a decimal). |
| `CHECK` | field, lo, hi | Fail the program unless lo ≤ the field ≤ hi (u8 bounds). Reads no tokens, writes nothing. Year and pair programs use `CHECK 0 10 29` (field 1 after a day) on the first part, so "fifty fifteen" is not a year. |
| `LITV` | k | Write the literal pool entry whose ID is `value_of` of span token k (e.g. "meters" → ` m`, "dollars" → `$`). Reads no tokens; consumption stays with `RAW` or `SKIP`. The table build checks `k < span_len` and that every token LITV can read has a value below the literal count. |

*   **Plural Suffix:** `plural` is a frozen 4096-bit set (512 bytes) indexed by canonical ID. Decade words ("twenties" … "nineties") are in the tens class with values 20 … 90; after a candidate renders, the engine writes `s` when the span's last token has its bit set, with no branch ("the nineteen eighties" → `the 1980s`, "upper sixties" → `upper 60s`).
*   **Build-Time Invariant:** For every cell `(domain, shape)`, each non-`NONE` candidate has `span_len` equal to `shape_len[shape] + affix_len[domain]`, where the frozen table `affix_len` gives the affix tokens a domain's candidates consume (0–2), and the `n` operands of its `FOLD`, `DIGITS`, `RAW`, and `SKIP` ops sum to that length. The table build rejects any cell that breaks this. This is what makes Candidate Freezing (Section 3) hold.
*   **Output Buffer:** The caller provides the output buffer. If it fills, the call returns `Overflow` and writes nothing past its end.
*   **Assembly:** Output pieces (rendered spans and passthrough tokens, in input order) are joined by single spaces. Passthrough tokens keep their original bytes and case.
*   **Window Limit:** A call with more than 8 tokens returns `TooManyTokens`. Callers split longer input into windows of at most 8 tokens and must not cut inside a run of numeric-role tokens; a cut there splits one number into two spans that render as two wrong values. A numeric run longer than 8 tokens cannot be formatted in one call. `engine::window::format_line` splits it into pieces of at most 8 tokens, each ending right after a group scale word (`thousand`, `million`, `billion`, `trillion`), taking the latest one that fits. Each piece is formatted alone and must render as digits, optionally followed by one scale word (`999 million`). The pieces are stitched left to right: the next piece replaces the trailing `k` zeros of the result so far, left-padded to `k` digits, where `k` is the exponent of the previous piece's last word. Stitching requires strictly decreasing exponents, a next piece of at most `k` digits, and `k` trailing zeros. If no scale word is in reach, or any check fails, the whole run passes through as raw text. `engine::window::windows_scaled` implements the split; `engine::window::windows` is the split with no scale words.
*   **Punctuation:** `format` expects tokens without attached punctuation; a token such as `five,` is unknown and passes through, which can split a number. `engine::window::format_line` strips leading `"'([{` and trailing `.,;:!?"')]}` from a token when the remainder is a lexicon word, treats that punctuation as a window boundary, and reattaches it to the output ("twenty five," → `25,`; "one, two" → `1, 2`). A trailing period is not doubled when the output already ends in one (`p.m.`).
*   **Lone Words:** A punctuation-bounded segment of one token whose canonical ID is in `words_first` stays as written, because punctuation and the line edge count as word neighbors ("First, we go" and "one, two, three" are unchanged; "twenty, thirty" → `20, 30`).
*   **Run Guard:** `format_line` leaves a run of value-bearing number words (nonzero role outside `suffix_roles` and `prefix_roles`) as written when a filler (`uh`, `um`, `er`, `ah`, `uhm`, `hmm`) sits between two of its words, when the next token is a counting word (digit, teen, tens, hundred, or scale role) plus `s` or `'s` ("seconds" is not one, so "thirty seconds" → `30 seconds`), or when a digit token touches it. These inputs have no single correct value, and formatting part of them shows a wrong one ("uh twenty uh five", "twenty fives", "eighteen hundreds", "twenty 5" are unchanged; "uh twenty five" → `uh 25`).
*   **Meridiem Join:** `format_line` joins the tokens `a m` into `am` when they directly follow a number word, because the article "a" is not a lexicon word ("eleven forty a m" → `11:40 am`; "five a day" is unchanged). After a number word, `am`, `pm`, or `p m`, three single letters spelling a time zone (EST, EDT, CST, CDT, MST, MDT, PST, PDT, GMT, UTC, BST) join into the capitalized abbreviation ("seven a m e s t" → `7 am EST`); the same letters elsewhere are unchanged.

---

## 3. Asynchronous Adaptive Feedback Loop

To keep the hot path wait-free, all learning mechanics are deferred to a background worker thread connected to the engine via a single-producer, single-consumer (`SPSC`) lock-free ring buffer.

```text
[Correction Call] ──(CorrectionEvent)──> SPSC Ring Buffer ──> [Worker Thread]
                                                                    │
[Hot Path Engine] <──(AtomicU64::store(Release) to one Tier 3 cell)─┘
```

### The Mutation Mechanism
1. **Receipts:** For each span it formats, the engine writes a `SpanReceipt { cell: u16, start: u8, len: u8 }` into a caller-provided array. The caller keeps the receipt and the input tokens until it has made any correction for that span.
2. **Resolving a Correction:** The correction call takes a receipt, the span's tokens, and the target text. It runs on the caller's thread, off the hot path. It renders each non-`NONE` candidate of the receipt's cell into a stack buffer and compares it with the target. A match gives the chosen Candidate ID. If nothing matches, the target is outside the frozen set: the call returns `OutOfSet`, increments an out-of-set counter, and pushes nothing. Learning only reorders existing candidates.
3. **Event:** A match pushes a fixed 4-byte `CorrectionEvent { cell: u16, candidate: u8, _pad: u8 }`. Events carry no strings, so pushing allocates nothing.
4. **Ring Full:** The ring has a fixed power-of-two capacity set at init. When it is full, the new event is dropped and an atomic drop counter is incremented. The producer never blocks and never overwrites unread slots.
5. The worker pops the event and reads the `NumberDomain` from `(cell >> 11) & 0x0F`.
6. The worker loads the event's cell word and finds the rank $r$ that holds candidate $c$. It modifies only this cell. The worker is the only writer, so a plain load-modify-store needs no compare-and-swap.
7. Streak counters belong to rank positions, not to candidates. The Rank 0 counter is always zero.
8. If $r = 0$, the current choice is confirmed: clear all counters.
9. If $r > 0$, increment the Rank $r$ counter and clear the other three.
10. **Hysteresis Promotion Gate:** When the Rank $r$ counter reaches the threshold $H$ (default $4$; valid range $1–15$, the 4-bit counter's limit), swap the candidate IDs at ranks $r$ and $r-1$ and clear all four counters.
11. Publish the new word with a single `AtomicU64::store(Release)`, skipping the store if the word is unchanged. The reader's `Relaxed` load is enough: the word is self-contained, and no other data is published with it.

### Bounding Wrong-Domain Blast Radius
If the Tier 2 matrix mistakenly misclassifies a sentence context, a user correction will land in the wrong domain's slot. The design handles this failure state via three structural bounds:
*   **Isolated Arenas:** Because cells are fundamentally isolated by their `NumberDomain` mapping key, a bad correction event modifies only the corrupted domain's slots. Outputs inside other domains (e.g., `Time`) remain physically untouched.
*   **Candidate Freezing:** The slot choices can *only* point to predefined, grammar-valid alternative representations of that identical span shape. The system cannot synthesize raw string literals or invent unseen tokens, meaning a bad write degrades to an alternative valid grammatical formatting choice rather than text corruption.
*   **Hysteresis Recovery:** Misrouted corrections cause at most one adjacent rank swap per $H$ consecutive misroutes, because every swap clears all counters. $H$ consecutive corrections for the displaced candidate swap it back.

---

## 4. Verification & Testing Primitives

Any implementation of this specification must pass three automated verification blocks before deployment.

### 1. Cycle-Counter Latency Profile
*   **Method:** Replay $10^6$ logged production utterances on one pinned core after a warm-up pass. Time each call separately.
*   **Metric:** The p99 of per-call time must be ≤ 1.00 µs, after subtracting the timer's own cost (the median of $10^4$ back-to-back empty timer reads). Report p50, p99, p99.9, and max.
*   **Allocation Check:** The bench binary installs a counting `#[global_allocator]`. Its count must not change across the replay.
*   **Timer:** The bench reads a hardware counter, with fences so the timed code cannot move across the reads:

| Target | Start read | End read | Ticks → ns |
|---|---|---|---|
| x86-64 | `lfence; rdtsc` | `rdtscp; lfence` | CPUID leaf `0x15` if it reports the TSC rate; else calibrate against `std::time::Instant` over ≥ 100 ms |
| AArch64 | `isb; mrs cntvct_el0` | `isb; mrs cntvct_el0` | `CNTFRQ_EL0` |
| Other | `std::time::Instant` | `std::time::Instant` | already ns |

*   **Resolution Gate:** A run can sign off this check only if one tick is ≤ 10 ns.
    *   x86-64 requires an invariant TSC (CPUID `0x80000007`, EDX bit 8). TSC rates are in the GHz range, so they pass.
    *   AArch64 cores from Armv8.6 on fix the counter at 1 GHz and pass. Earlier cores choose their own rate (Apple M1: 24 MHz, about 42 ns per tick) and fail. On those, the bench uses the PMU cycle counter `PMCCNTR_EL0`. Linux exposes it to user space when `kernel.perf_user_access = 1` and the event is opened with `perf_event_open` with `config1` bit 1 set. Cycles convert to ns only at a fixed clock, so lock the pinned core's frequency. On big.LITTLE parts, pin to one core type.
    *   A run that fails the gate still reports its numbers, marked coarse, and cannot sign off.
*   **Scope:** The timer, PMU path, and counting allocator live in the bench harness only. The library stays zero-dependency.

### 2. Domain-Disjoint Drift Test
*   **Method:** Split a validation dataset into slices by the domain Tier 2 routes each span to, not by its true domain; a misrouted span belongs to the slice of the domain it was routed to. Inject a burst of synthetic correction cycles exclusively targeting a single domain's cells.
*   **Invariant:** Execute a full string-level `diff` across the outputs of the non-target datasets. The number of changed outputs in non-target sets must equal exactly zero.

### 3. Hysteresis Sweep & Thrash Audit
*   **Method:** Stream correction sequences to a single cell while sweeping $H$ from 1 to 15:
    *   strict alternation (A, B, A, B, …);
    *   mixed ratios (3:1 and 1:3);
    *   runs of exactly $H$ and exactly $H-1$ corrections for one candidate.
*   **Invariants:**
    1. Every swap matches a scalar reference model of the update rule, event by event.
    2. $N$ corrections cause at most $\lfloor N / H \rfloor$ swaps.
    3. Strict alternation causes zero swaps for $H \ge 2$ and one swap per correction for $H = 1$, except a correction that names the Rank 0 candidate.
    4. $H$ consecutive corrections for a candidate at rank $r > 0$ cause exactly one swap; $H-1$ cause none.
