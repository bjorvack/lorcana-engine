//! Card definition types.
//!
//! A `CardDefinition` is the static, printed data for a card (reference data),
//! as opposed to a [`CardInstance`] which is a specific copy in a game. Only the
//! fields needed by the current slice are modeled; this grows as later slices
//! need name, abilities, classifications, etc.
//!
//! [`CardInstance`]: crate::domain::game::CardInstance

use super::card_kind::CardKind;
use crate::domain::types::card::CardType;
use crate::domain::types::ids::CardDefId;
use serde::{Deserialize, Serialize};

/// The printed data for a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDefinition {
    id: CardDefId,
    /// Ink cost to play the card (§6.2.7).
    cost: u32,
    /// Whether the card has the inkwell symbol and so may be put into the
    /// inkwell (§4.3.3.1, §6.2.8).
    inkwell: bool,
    /// Type-specific characteristics.
    kind: CardKind,
}

impl CardDefinition {
    /// Create a card definition.
    #[must_use]
    pub const fn new(id: CardDefId, cost: u32, inkwell: bool, kind: CardKind) -> Self {
        Self {
            id,
            cost,
            inkwell,
            kind,
        }
    }

    /// Convenience constructor for a character card.
    #[must_use]
    pub const fn character(
        id: CardDefId,
        cost: u32,
        inkwell: bool,
        strength: u32,
        willpower: u32,
        lore: u32,
    ) -> Self {
        Self::new(
            id,
            cost,
            inkwell,
            CardKind::Character {
                strength,
                willpower,
                lore,
            },
        )
    }

    /// The printed-card id.
    #[must_use]
    pub const fn id(self) -> CardDefId {
        self.id
    }

    /// The ink cost to play this card.
    #[must_use]
    pub const fn cost(self) -> u32 {
        self.cost
    }

    /// Whether this card has the inkwell symbol.
    #[must_use]
    pub const fn has_inkwell_symbol(self) -> bool {
        self.inkwell
    }

    /// The type-specific characteristics.
    #[must_use]
    pub const fn kind(self) -> CardKind {
        self.kind
    }

    /// The card-type tag.
    #[must_use]
    pub const fn card_type(self) -> CardType {
        self.kind.card_type()
    }
}
