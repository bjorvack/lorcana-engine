//! Represents the rarity of a card

use serde::{Deserialize, Serialize};

/// Represents the rarity of a card
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rarity {
    /// Common rarity
    Common,
    /// Uncommon rarity
    Uncommon,
    /// Rare rarity
    Rare,
    /// Super Rare rarity
    SuperRare,
    /// Legendary rarity
    Legendary,
    /// Epic rarity
    Epic,
    /// Enchanted rarity
    Enchanted,
    /// Iconic rarity
    Iconic,
}
