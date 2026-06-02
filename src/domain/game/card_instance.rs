//! A specific card instance within a game.

use super::Conditions;
use crate::domain::types::ids::{CardDefId, CardId};
use serde::{Deserialize, Serialize};

/// A physical card in a game: a unique [`CardId`] plus the printed card it
/// represents ([`CardDefId`]) and its current [`Conditions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInstance {
    id: CardId,
    definition: CardDefId,
    conditions: Conditions,
}

impl CardInstance {
    /// Create a card instance with the given conditions.
    #[must_use]
    pub const fn new(id: CardId, definition: CardDefId, conditions: Conditions) -> Self {
        Self {
            id,
            definition,
            conditions,
        }
    }

    /// The instance id (unique within the game).
    #[must_use]
    pub const fn id(self) -> CardId {
        self.id
    }

    /// The printed card this instance represents.
    #[must_use]
    pub const fn definition(self) -> CardDefId {
        self.definition
    }

    /// The current conditions on this instance.
    #[must_use]
    pub const fn conditions(self) -> Conditions {
        self.conditions
    }

    /// Mutable access to this instance's conditions.
    pub const fn conditions_mut(&mut self) -> &mut Conditions {
        &mut self.conditions
    }
}
