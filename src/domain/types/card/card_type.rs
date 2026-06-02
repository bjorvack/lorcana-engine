//! Represents the type of a card

use serde::{Deserialize, Serialize};

/// Represents the type of a card
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardType {
    /// Character card
    Character,
    /// Action card
    Action,
    /// Item card
    Item,
    /// Location card
    Location,
    /// Song card
    Song,
}
