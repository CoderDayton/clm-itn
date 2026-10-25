# CLM-ITN Team Board

Maintained by the boss session (`clam-itn-43`). Employees: read it before starting a task; send updates by message and the boss records them here.

## Members

| Session | Role | Message address |
|---|---|---|
| `clam-itn-43` | Boss: plans, reviews, owns shared types and docs | `clam-itn-43` |
| `clam-itn-e5` | Specialist: tables, tiers, builder | `clam-itn-e5` |
| `clam-itn-11` | Local execution: queue, worker, bench, eval, tests | `clam-itn-11` |
| `clam-itn-65` | Research strategist: synthesizes team results and outside research into ranked proposals; asks members for data; edits no repo files | `clam-itn-65` |

## Layout

Files are grouped by what they are for. Put new files in the folder that matches their purpose. A new folder, or a file that fits no folder, needs the boss's approval first.

```text
build.rs              generates the static English tables          e5
docs/                 SPEC.md, TEAM.md                             boss
src/
├── lib.rs            modules + root re-exports                    boss
├── types.rs          shared IDs, constants, records               boss
├── hash.rs           SPEC hash + seed generator                   boss
├── tables/           read-only data the engine reads              e5
│   ├── mod.rs            module list, re-exports Tables
│   ├── bundle.rs         Tables, Aligned
│   ├── lexicon.rs        Tier 1: word → class
│   ├── shape.rs          shape key + shape table
│   ├── context.rs        Tier 2: context → domain
│   ├── candidate.rs      output programs
│   └── english.rs        static ENGLISH (includes the generated file)
├── engine/           the formatting hot path
│   ├── mod.rs            Engine: format, resolve, correct         e5
│   ├── span.rs           span segmentation                        e5
│   └── window.rs         line splitting, format_line              boss
├── learn/            corrections and learning
│   ├── cell.rs           Tier 3: rank word + learning rule        e5
│   ├── ring.rs           correction queue (SPSC)                  11
│   ├── worker.rs         background worker, drain                 11
│   └── corrector.rs      queue + wake in one call                 boss
├── builder/          builds and checks tables (build time)        e5
│   ├── mod.rs, english.rs, codegen.rs
├── main.rs           CLI                                          boss
└── bin/
    ├── clm-bench/    latency replay, timers, PMU, alloc counter   11
    ├── clm-eval/     NeMo accuracy scorer                         11
    └── clm-learn/    corpus counts → learned tables               e5
tools/pairgen/        NeMo TN pair generator (Python, offline)     11
tests/
├── end_to_end.rs     spoken → written examples                    boss
├── drift.rs          SPEC §4.2 drift                              e5
├── static_tables.rs  allocation + static-table checks             e5
└── fuzz.rs           random-input invariants                      11
```

`build.rs` mirrors `tables/` with inline `#[path]` modules. A new file under `tables/` that the builder needs must also be added there and to its `SOURCES` list.

## Current work

| Who | Task | Touches | Build state |
|---|---|---|---|
| `clam-itn-e5` | Paused. MEASURE step 0 ("per" units, °C/°F) built, not yet gated | `tables/`, `engine/` (except `window.rs`), `builder/`, `src/bin/clam-learn/` | Green |
| `clam-itn-11` | Paused. Gatekeeper: before/after eval + pinned p99 | `tools/pairgen/`, `src/bin/clam-eval/`, `src/bin/clam-bench/`, `learn/ring.rs`, `learn/worker.rs` | Green |
| `clam-itn-65` | Researching fixes for the merged open-problem list | none (research only) | — |
| `clam-itn-43` | Reviews, spec, docs, `types.rs`, `engine/window.rs`, `learn/corrector.rs`, CLI | `docs/`, `README.md`, `src/types.rs`, `src/engine/window.rs`, `src/main.rs` | Green |

One editor per file. To change a file you don't own, message its owner.

## Shared interfaces

| Interface | Provider | Used by |
|---|---|---|
| `clam_itn::english() -> &'static Tables` (`tables::english`), `builder::build_english() -> Result<Box<Tables>, BuildError>` | `clam-itn-e5` | bench, eval, fuzz, CLI, tests |
| `clam_itn::Engine` (`engine::Engine`): `new(&'t Tables)`, `format`, `resolve`, `correct`, `tables`, `cells` | `clam-itn-e5` | bench, eval, fuzz, CLI, tests, corrector |
| `engine::window::{windows, windows_scaled}`, `clam_itn::format_line(&Engine, &str, &mut String)` | `clam-itn-43` | CLI, eval, learner |
| `clam_itn::Corrector` (`learn::corrector`): `start`, `correct`, `dropped`, `stop` | `clam-itn-43` | applications |
| `learn::ring::channel`, `learn::worker::{drain, Worker}` | `clam-itn-11` | engine tests, drift, end-to-end, corrector |

## Rules

1. **Announce red and green.** Before a change that breaks the shared build, message the other employee and the boss; message again when it is green. Keep red periods short.
2. **Announce API changes.** When a shared interface changes, message every user of it with the exact new signature.
3. **Talk directly** about interfaces and build state. Bring scope, priority, and anything only the user can decide to the boss.
4. **Report to the boss** when a task is done: what changed, how it was checked.
5. No commits or pushes. No new dependencies. Follow the repo and global `CLAUDE.md`.
6. **Decide with evidence.** For a design decision, research it (web search via `mcp__searxng`, docs via `mcp__context7`), pick a solution, and send it to the boss with the evidence. Keep working on it unless the boss objects. The boss decides; only commits, pushes, deletions, and dependency changes go to the user.
7. The GPU lock is shared across sessions; this project is CPU-only, so mark commands `# nogpu`.

## Status

- `cargo script lint-ci`: clean. `cargo script test`: all green (309 tests).
- Latency (SPEC §4.1): PASS, `Engine::format` p99 ~245 ns pinned on a P-core under shared load, 0 allocations. Per tier p99: Tier 1 ~125, Tier 2 ~63, Tier 3 ~8, output ~83 ns.
- Tables: 11-bit shapes (1017 of 2047 used), ≈730 KB static plus 256 KB live cells.
- aarch64: `cargo clippy --target aarch64-unknown-linux-gnu --all-targets -- -D warnings` clean; not run (no emulator or linker).
- NeMo (`clm-eval <dir>`): 68.1% exact, 87.5% digits, partial 2.7%. NeMo exact is reported, not gated, because the output follows AP style (capital months, "Month D, YYYY") and NeMo golds are lowercase without commas.
- Own pairs (`clm-eval --pairs`, val): in-scope 71.4% exact / 79.1% relaxed / 84.3% value; balanced (in-scope + kept-as-words) 87.8% exact; DATE 73.8% exact, 94.8% digits.
- Known open problems: merged list under research (wrong values: "one fifth" → `6th`, fillers inside numbers, "fifty fifty" → `5050`).
- Training data: Google TN is CC BY-NC-SA 4.0, so eval only and never shipped. Training pairs come from NeMo TN (Apache-2.0) over public-domain text.
- Spec conformance: all items met except production replay logs (none available).
