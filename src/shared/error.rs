//! Error types for the lorcana-engine

use thiserror::Error;

/// Error type for lorcana-engine operations
#[derive(Error, Debug)]
pub enum LorcanaError {
    /// Generic error for unclassified cases
    #[error("An error occurred: {0}")]
    Generic(String),
    // TODO: Add more specific error types as needed
}

// TODO: Expand error types in Phase 1.2
