//! Unique identifier for a player

use serde::{Deserialize, Serialize};

/// Identifies a player by their seat index (0-based).
///
/// Player ids are deterministic: the player in seat 0 always has
/// `PlayerId::from_index(0)`. This keeps game state reproducible for replays,
/// unlike a randomly generated identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(u8);

impl PlayerId {
    /// Create a `PlayerId` from a 0-based seat index.
    #[must_use]
    pub const fn from_index(index: u8) -> Self {
        Self(index)
    }

    /// Get the 0-based seat index.
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }
}
