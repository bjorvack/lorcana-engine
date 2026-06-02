//! Type-specific card data.

use crate::domain::types::card::CardType;
use serde::{Deserialize, Serialize};

/// The per-type characteristics of a card.
///
/// Modeling these as variants keeps invalid states unrepresentable (e.g. an
/// action can't carry willpower). Only the data needed so far is included;
/// variants gain fields as later slices need them (e.g. location move cost, the
/// song classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardKind {
    /// A character with its printed Strength, Willpower, and Lore (§6.1, §6.2).
    Character {
        /// Printed Strength `{S}`.
        strength: u32,
        /// Printed Willpower `{W}`.
        willpower: u32,
        /// Printed Lore `{L}` gained when questing.
        lore: u32,
    },
    /// An action (resolves then goes to discard; never in play).
    Action,
    /// An item (stays in play).
    Item,
    /// A location (stays in play, §6.5).
    Location {
        /// Move cost — ink to move a character here (§6.5.4).
        move_cost: u32,
        /// Printed Willpower `{W}` — banished when damage reaches it (§6.5.5).
        willpower: u32,
        /// Printed Lore `{L}` — gained at the Set step (§6.5.6).
        lore: u32,
    },
}

impl CardKind {
    /// The [`CardType`] tag for this kind.
    #[must_use]
    pub const fn card_type(self) -> CardType {
        match self {
            Self::Character { .. } => CardType::Character,
            Self::Action => CardType::Action,
            Self::Item => CardType::Item,
            Self::Location { .. } => CardType::Location,
        }
    }
}
