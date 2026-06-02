//! A specific card instance within a game.

use super::{CharacterStats, Conditions};
use crate::domain::types::card::Classification;
use crate::domain::types::ids::{CardDefId, CardId};
use serde::{Deserialize, Serialize};

/// A physical card in a game: a unique [`CardId`] plus the printed card it
/// represents ([`CardDefId`]) and its current [`Conditions`].
///
/// `stats` holds the live [`CharacterStats`] while the card is an in-play
/// character; it is `None` for cards that aren't in-play characters (in a deck,
/// hand, inkwell, or for non-character types). `classifications` is denormalized
/// from the definition when the card enters play so selector matching (§7.8) can
/// stay state-only.
///
/// Not `Copy`: it owns a `Vec` of classifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInstance {
    id: CardId,
    definition: CardDefId,
    conditions: Conditions,
    stats: Option<CharacterStats>,
    classifications: Vec<Classification>,
}

impl CardInstance {
    /// Create a card instance with the given conditions, no in-play stats, and
    /// no classifications.
    #[must_use]
    pub const fn new(id: CardId, definition: CardDefId, conditions: Conditions) -> Self {
        Self {
            id,
            definition,
            conditions,
            stats: None,
            classifications: Vec::new(),
        }
    }

    /// The instance id (unique within the game).
    #[must_use]
    pub const fn id(&self) -> CardId {
        self.id
    }

    /// The printed card this instance represents.
    #[must_use]
    pub const fn definition(&self) -> CardDefId {
        self.definition
    }

    /// The current conditions on this instance.
    #[must_use]
    pub const fn conditions(&self) -> Conditions {
        self.conditions
    }

    /// Mutable access to this instance's conditions.
    pub const fn conditions_mut(&mut self) -> &mut Conditions {
        &mut self.conditions
    }

    /// The live character stats, if this is an in-play character.
    #[must_use]
    pub const fn stats(&self) -> Option<CharacterStats> {
        self.stats
    }

    /// `true` if this instance is an in-play character (has character stats).
    #[must_use]
    pub const fn is_character(&self) -> bool {
        self.stats.is_some()
    }

    /// Set (or clear) this instance's live character stats.
    pub const fn set_stats(&mut self, stats: Option<CharacterStats>) {
        self.stats = stats;
    }

    /// This instance's classifications (§6.2.6).
    #[must_use]
    pub fn classifications(&self) -> &[Classification] {
        &self.classifications
    }

    /// Whether this instance has the given classification.
    #[must_use]
    pub fn has_classification(&self, classification: &Classification) -> bool {
        self.classifications.contains(classification)
    }

    /// Set this instance's classifications (denormalized from the definition when
    /// the card enters play).
    pub fn set_classifications(&mut self, classifications: Vec<Classification>) {
        self.classifications = classifications;
    }
}
