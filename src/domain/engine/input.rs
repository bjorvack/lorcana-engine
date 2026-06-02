//! Player inputs and the reasons an input may be rejected.

use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An input submitted to the engine. Inputs (plus the seed) are the complete,
/// replayable record of a game; the engine emits events in response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    /// Alter-hand decision: put these hand cards on the bottom of the deck, then
    /// draw back up to a full hand and reshuffle (§3.1.6).
    Mulligan {
        /// The player making the decision (must be the one currently awaited).
        player: PlayerId,
        /// The cards to put on the bottom of the deck (may be empty).
        put_back: Vec<CardId>,
    },
    /// Put a card from the active player's hand into their inkwell (§4.3.3).
    PutCardInInkwell {
        /// The hand card to ink.
        card: CardId,
    },
    /// End the active player's turn (§4.4).
    EndTurn,
}

/// Why an [`Input`] was rejected. When an input is rejected the game state is
/// left unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Rejected {
    /// `start` was called on a game that is not in the `NotStarted` state.
    #[error("the game has already been started")]
    AlreadyStarted,
    /// A mulligan was submitted while the game is not awaiting one.
    #[error("the game is not awaiting a mulligan")]
    NotAwaitingMulligan,
    /// A mulligan was submitted by the wrong player.
    #[error("it is not that player's turn to mulligan")]
    WrongMulliganPlayer,
    /// A mulligan put-back card is not in the player's hand.
    #[error("mulligan put-back card {0:?} is not in the player's hand")]
    MulliganCardNotInHand(CardId),
    /// A turn action was submitted while the game is not in progress.
    #[error("the game is not in progress")]
    NotPlaying,
    /// A turn action was submitted outside the Main phase.
    #[error("that action can only be taken during the Main phase")]
    NotMainPhase,
    /// The named card is not in the active player's hand.
    #[error("card {0:?} is not in the active player's hand")]
    CardNotInHand(CardId),
    /// The named card has no known definition in the registry.
    #[error("card {0:?} has no definition in the registry")]
    UnknownCard(CardId),
    /// The named card does not have the inkwell symbol (§4.3.3.1).
    #[error("card {0:?} cannot be put into the inkwell")]
    NoInkwellSymbol(CardId),
    /// The once-per-turn inkwell action has already been used (§4.3.3).
    #[error("a card has already been put into the inkwell this turn")]
    AlreadyInkedThisTurn,
}
