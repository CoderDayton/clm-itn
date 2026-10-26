#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
#[doc(hidden)]
pub mod builder;
#[doc(hidden)]
pub mod engine;
#[doc(hidden)]
pub mod hash;
#[doc(hidden)]
pub mod tables;
#[doc(hidden)]
pub mod types;

#[cfg(feature = "alloc")]
pub use engine::window::format_line;
pub use engine::{Formatted, Normalizer};
pub use tables::Tables;
pub use tables::english::english;
pub use types::{FormatError, MAX_TOKENS, Setting};

#[cfg(doctest)]
#[doc = include_str!("../docs/examples.md")]
struct ExamplesDoctest;
