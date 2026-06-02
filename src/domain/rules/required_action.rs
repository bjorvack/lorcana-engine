//! Required actions produced by the game-state check.

use crate::domain::types::ids::PlayerId;
use serde::{Deserialize, Serialize};

/// An action the game *must* perform as a result of a game-state check (§1.9.1).
///
/// Only the win/loss actions exist so far; banishment (§1.9.1.3) is added with
/// challenges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RequiredAction {
    /// The given player wins the game (§1.9.1.1, §3.2.1.3).
    PlayerWins(PlayerId),
    /// The given player loses the game (§1.9.1.2).
    PlayerLoses(PlayerId),
}
