//! `clm-bench` rebuilt with libtest so `cargo test` runs the harness's unit
//! tests. `cargo test` skips bench targets, and a harness-less bench never runs
//! `#[test]` functions, so the tests live here instead of in the bench modules.

include!("main.rs");

mod unit {
    mod accuracy;
    mod alloc;
    mod clock;
    mod corpus;
    mod gold;
    mod lines;
    mod pairs;
    mod pmu;
    mod replay;
    mod score;
    mod spell;
    mod tiers;
}
