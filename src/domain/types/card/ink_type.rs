//! Represents the ink color/type of a card

use serde::{Deserialize, Serialize};

/// Represents the ink color/type of a card
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InkType {
    /// Amber ink
    Amber,
    /// Amethyst ink
    Amethyst,
    /// Emerald ink
    Emerald,
    /// Ruby ink
    Ruby,
    /// Sapphire ink
    Sapphire,
    /// Steel ink
    Steel,
}
