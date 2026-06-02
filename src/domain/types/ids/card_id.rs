//! Unique identifier for a card instance

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a card instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CardId(Uuid);

impl CardId {
    /// Create a new `CardId` with a random UUID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a `CardId` from a UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    #[must_use]
    pub const fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for CardId {
    fn default() -> Self {
        Self::new()
    }
}
