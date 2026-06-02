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

/// The live characteristics of an in-play location (§6.5).
///
/// Like [`CharacterStats`] these are denormalized onto the [`CardInstance`] when
/// the location enters play so the game-state check (banishment at damage ≥
/// willpower) and the Set-step lore gain read them from state without a registry
/// lookup. Locations have no Strength and deal no damage (§6.5.5).
///
/// TODO(modifiable location stats — see the `Stat` TODO in
/// `src/domain/game/modifier.rs`): move cost / willpower / lore should become
/// `Stat` variants so continuous effects can adjust them; for now they're fixed.
///
/// [`CardInstance`]: crate::domain::game::CardInstance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationStats {
    /// Willpower `{W}` — banished when damage reaches it (§6.5.5).
    pub willpower: u32,
    /// Lore `{L}` gained at the controller's Set step (§6.5.6).
    pub lore: u32,
    /// Move cost — ink to move one of your characters here (§6.5.4, §4.3.7).
    pub move_cost: u32,
}

impl LocationStats {
    /// Create location stats.
    #[must_use]
    pub const fn new(willpower: u32, lore: u32, move_cost: u32) -> Self {
        Self {
            willpower,
            lore,
            move_cost,
        }
    }
}
