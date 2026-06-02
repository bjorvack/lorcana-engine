//! Unique identifier for a zone

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a zone
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ZoneId(Uuid);

impl ZoneId {
    /// Create a new `ZoneId` with a random UUID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a `ZoneId` from a UUID
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

impl Default for ZoneId {
    fn default() -> Self {
        Self::new()
    }
}
