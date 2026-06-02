//! The live characteristics of an in-play character.

use serde::{Deserialize, Serialize};

/// A character's current Strength, Willpower, and Lore while in play.
///
/// These are copied from the [`CardDefinition`] when the character enters play
/// and then live on the [`CardInstance`], so the game-state check and challenge
/// resolution read them directly from state (no registry lookup). Once stat
/// modifiers exist (Slice 5) they will adjust these per-instance values.
///
/// [`CardDefinition`]: crate::domain::cards::CardDefinition
/// [`CardInstance`]: crate::domain::game::CardInstance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterStats {
    /// Current Strength `{S}` — damage dealt in a challenge (§6.2.9).
    pub strength: u32,
    /// Current Willpower `{W}` — banished when damage reaches it (§6.2.10).
    pub willpower: u32,
    /// Current Lore `{L}` — gained when questing (§6.2.11).
    pub lore: u32,
}

impl CharacterStats {
    /// Create character stats.
    #[must_use]
    pub const fn new(strength: u32, willpower: u32, lore: u32) -> Self {
        Self {
            strength,
            willpower,
            lore,
        }
    }
}
