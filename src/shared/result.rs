//! Result type alias for lorcana-engine

use crate::shared::error::LorcanaError;

/// Result type alias for lorcana-engine operations
pub type Result<T> = std::result::Result<T, LorcanaError>;
