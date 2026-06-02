//! Represents the type of a card

use serde::{Deserialize, Serialize};

/// Represents the type of a card
///
/// Note: a Song is **not** a distinct card type. Per the comprehensive rules
/// (§6.3.3), a song is an `Action` carrying the "Song" classification, so it is
/// represented as `CardType::Action` plus a classification rather than its own
/// variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CardType {
    /// Character card
    Character,
    /// Action card (includes songs, which carry the "Song" classification)
    Action,
    /// Item card
    Item,
    /// Location card
    Location,
}
