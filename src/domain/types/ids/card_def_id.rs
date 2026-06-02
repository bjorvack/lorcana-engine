//! Identifier for a printed card definition

use serde::{Deserialize, Serialize};

/// Identifies a printed card (a card *definition*), as opposed to a specific
/// physical instance of it in a game (see [`CardId`]).
///
/// Many [`CardId`] instances can share the same `CardDefId` (e.g. four copies
/// of the same card in a deck).
///
/// [`CardId`]: super::CardId
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CardDefId(u32);

impl CardDefId {
    /// Create a `CardDefId` from a raw value.
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
