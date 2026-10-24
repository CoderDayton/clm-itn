#![doc = include_str!("../README.md")]

pub mod builder;
pub mod engine;
pub mod hash;
pub mod learn;
pub mod tables;
pub mod types;

pub use engine::Engine;
pub use engine::window::format_line;
pub use learn::corrector::Corrector;
pub use tables::Tables;
pub use tables::english::english;
