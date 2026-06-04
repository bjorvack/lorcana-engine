//! Application layer — an ergonomic facade over the domain reducer for embedding
//! the engine in a host (CLI, server, AI).

pub mod api;
pub mod host;

pub use api::{Game, SetupError};
