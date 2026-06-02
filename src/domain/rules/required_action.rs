//! Required actions produced by the game-state check.

use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};

/// An action the game *must* perform as a result of a game-state check (§1.9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RequiredAction {
    /// The given player wins the game (§1.9.1.1, §3.2.1.3).
    PlayerWins(PlayerId),
    /// The given player loses the game (§1.9.1.2).
    PlayerLoses(PlayerId),
    /// The given card is banished — its damage has reached its Willpower
    /// (§1.9.1.3). It moves to its owner's discard (§8.6.2).
    Banish {
        /// The card's owner.
        player: PlayerId,
        /// The card to banish.
        card: CardId,
    },
}
