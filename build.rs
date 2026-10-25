#![allow(dead_code)]

#[path = "src/builder/mod.rs"]
mod builder;
#[path = "src/engine"]
mod engine {
    pub mod span;
}
#[path = "src/hash.rs"]
mod hash;
#[path = "src/types.rs"]
mod types;

#[path = "src/tables"]
mod tables {
    mod bundle;
    pub mod candidate;
    pub mod cell;
    pub mod context;
    pub mod lexicon;
    pub mod shape;

    pub use bundle::{Aligned, MAX_CANONICAL, Tables};
}

use std::path::PathBuf;

const SOURCES: [&str; 16] = [
    "build.rs",
    "src/builder/mod.rs",
    "src/builder/english.rs",
    "src/builder/codegen.rs",
    "src/builder/learn.rs",
    "src/builder/learned.rs",
    "src/builder/learned.txt",
    "src/engine/span.rs",
    "src/hash.rs",
    "src/types.rs",
    "src/tables/bundle.rs",
    "src/tables/candidate.rs",
    "src/tables/cell.rs",
    "src/tables/context.rs",
    "src/tables/lexicon.rs",
    "src/tables/shape.rs",
];

fn main() {
    for src in SOURCES {
        println!("cargo:rerun-if-changed={src}");
    }
    // Shared table sources gate their std-only items on `not(clm_no_std)`, so
    // this script's own copy of them keeps those items.
    if std::env::var_os("CARGO_FEATURE_STD").is_none() {
        println!("cargo:rustc-cfg=clm_no_std");
    }
    let tables = builder::build_english()
        .unwrap_or_else(|e| panic!("English tables fail validation: {e:?}"));
    let code = builder::codegen::to_rust(&tables, "ENGLISH");
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    let path = PathBuf::from(out_dir).join("english_tables.rs");
    std::fs::write(&path, code).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}
