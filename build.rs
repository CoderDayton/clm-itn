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

#[path = "src/learn"]
mod learn {
    pub mod cell;
}

#[path = "src/tables"]
mod tables {
    mod bundle;
    pub mod candidate;
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
    "src/learn/cell.rs",
    "src/tables/bundle.rs",
    "src/tables/candidate.rs",
    "src/tables/context.rs",
    "src/tables/lexicon.rs",
    "src/tables/shape.rs",
];

fn main() {
    for src in SOURCES {
        println!("cargo:rerun-if-changed={src}");
    }
    let tables = builder::build_english()
        .unwrap_or_else(|e| panic!("English tables fail validation: {e:?}"));
    let code = builder::codegen::to_rust(&tables, "ENGLISH");
    let out_dir = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR for build scripts");
    let path = PathBuf::from(out_dir).join("english_tables.rs");
    std::fs::write(&path, code).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}
