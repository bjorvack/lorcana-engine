//! Error types for the lorcana-engine

use thiserror::Error;

/// Error type for lorcana-engine operations
///
/// Specific error variants will be added as needed during implementation.
/// Follow YAGNI principles - only add error types when you actually encounter them.
#[derive(Error, Debug)]
pub enum LorcanaError {
    /// Generic error for unclassified cases
    #[error("An error occurred: {0}")]
    Generic(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
