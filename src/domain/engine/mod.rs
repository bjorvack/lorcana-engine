//! The engine: setup and the input-driven reducer.

pub mod input;
pub mod reducer;

// Re-export for convenience
pub use input::{Decision, Input, Rejected};
pub use reducer::{apply, start};
