//! The overall status of a game.

use crate::domain::types::ids::PlayerId;
use serde::{Deserialize, Serialize};

/// Where a game is in its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    /// Built but not yet started (decks shuffled; hands not dealt).
    NotStarted,
    /// Pre-game alter-hand: waiting for this player's mulligan decision, in turn
    /// order from the starting player (§3.1.6).
    AwaitingMulligan(PlayerId),
    /// Normal play is under way.
    Playing,
    /// The game is over. `winners` is empty for a draw, holds one player for the
    /// usual case, and may hold several for a simultaneous multiplayer win.
    Finished { winners: Vec<PlayerId> },
}
