#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

// Public only under the unstable `internals` feature, for this repo's tests,
// bench and tools. `hash`, `tables` and `types` are shared with build.rs, so
// the engine alone leaves the builder's items in them unused.
#[cfg(feature = "internals")]
#[doc(hidden)]
pub mod builder;
#[cfg(feature = "internals")]
#[doc(hidden)]
pub mod engine;
#[cfg(not(feature = "internals"))]
mod engine;
#[cfg(feature = "internals")]
#[doc(hidden)]
pub mod hash;
#[cfg(not(feature = "internals"))]
#[allow(dead_code)]
mod hash;
#[cfg(feature = "internals")]
#[doc(hidden)]
pub mod tables;
#[cfg(not(feature = "internals"))]
#[allow(dead_code, unused_imports)]
mod tables;
#[cfg(feature = "internals")]
#[doc(hidden)]
pub mod types;
#[cfg(not(feature = "internals"))]
#[allow(dead_code)]
mod types;

pub use engine::Normalizer;
pub use types::Setting;

#[cfg(doctest)]
#[doc = include_str!("../docs/examples.md")]
struct ExamplesDoctest;
