//! Unique identifier for a card instance

use serde::{Deserialize, Serialize};

/// Identifies a specific card *instance* within a game (a physical card),
/// distinct from the printed card it represents (see [`CardDefId`]).
///
/// Instance ids are allocated sequentially by the game so that the same seed
/// and inputs always produce the same ids, keeping replays reproducible.
///
/// [`CardDefId`]: super::CardDefId
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CardId(u32);

impl CardId {
    /// Create a `CardId` from a raw sequential value.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get the underlying raw value.
    #[must_use]
    pub const fn as_raw(self) -> u32 {
        self.0
    }
}
