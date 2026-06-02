//! Unique identifier for a player

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a player
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(Uuid);

impl PlayerId {
    /// Create a new `PlayerId` with a random UUID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a `PlayerId` from a UUID
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

impl Default for PlayerId {
    fn default() -> Self {
        Self::new()
    }
}
