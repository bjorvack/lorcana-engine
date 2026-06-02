//! Card definition types.
//!
//! A `CardDefinition` is the static, printed data for a card (reference data),
//! as opposed to a [`CardInstance`] which is a specific copy in a game. Only the
//! fields needed by the current slice are modeled; this grows as later slices
//! need name, cost, type, stats, abilities, etc.
//!
//! [`CardInstance`]: crate::domain::game::CardInstance

use crate::domain::types::ids::CardDefId;
use serde::{Deserialize, Serialize};

/// The printed data for a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDefinition {
    id: CardDefId,
    /// Whether the card has the inkwell symbol and so may be put into the
    /// inkwell (§4.3.3.1, §6.2.8).
    inkwell: bool,
}

impl CardDefinition {
    /// Create a card definition.
    #[must_use]
    pub const fn new(id: CardDefId, inkwell: bool) -> Self {
        Self { id, inkwell }
    }

    /// The printed-card id.
    #[must_use]
    pub const fn id(self) -> CardDefId {
        self.id
    }

    /// Whether this card has the inkwell symbol.
    #[must_use]
    pub const fn has_inkwell_symbol(self) -> bool {
        self.inkwell
    }
}
