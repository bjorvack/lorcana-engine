//! Player inputs and the reasons an input may be rejected.

use crate::domain::game::TriggerId;
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
    /// Play a card from the active player's hand, paying its ink cost (§4.3.4).
    PlayCard {
        /// The hand card to play.
        card: CardId,
    },
    /// Send one of the active player's characters on a quest (§4.3.5).
    Quest {
        /// The character to quest with.
        character: CardId,
    },
    /// Challenge an exerted opposing character with one of the active player's
    /// characters (§4.3.6).
    Challenge {
        /// The active player's challenging character.
        challenger: CardId,
        /// The opposing character being challenged.
        target: CardId,
    },
    /// Use an activated ability of one of the active player's in-play cards
    /// (§7.5). `ability` indexes the source's activated abilities.
    UseAbility {
        /// The in-play card whose ability is being used.
        card: CardId,
        /// The index of the activated ability on that card.
        ability: usize,
    },
    /// End the active player's turn (§4.4).
    EndTurn,
    /// Answer the engine's currently pending decision (bag resolution, §8.7).
    Decide(Decision),
}

/// A player's answer to a [`PendingDecision`](crate::domain::game::PendingDecision).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// For `OrderTriggers`: the next of your bag triggers to resolve (§8.7.4).
    ResolveNext(TriggerId),
    /// For `MayResolve`: whether to apply an optional ("you may") trigger.
    May(bool),
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
    /// There is not enough ready ink to pay the card's cost (§4.3.4.6).
    #[error("not enough ready ink to play card {0:?}")]
    InsufficientInk(CardId),
    /// This card type cannot be played yet (only characters are supported so
    /// far; items, locations, and actions arrive in later slices).
    #[error("card {0:?} is of a type that cannot be played yet")]
    CardTypeNotPlayableYet(CardId),
    /// The named character is not in the active player's play area.
    #[error("character {0:?} is not in play")]
    CharacterNotInPlay(CardId),
    /// The named card in play is not a character and so cannot quest (§6.1.3).
    #[error("card {0:?} is not a character")]
    NotACharacter(CardId),
    /// The character is still drying and cannot quest this turn (§4.3.5.5).
    #[error("character {0:?} is still drying and cannot quest")]
    CharacterStillDrying(CardId),
    /// The character is exerted and so cannot be declared as questing or
    /// challenging (§4.3.5, §4.3.6.6).
    #[error("character {0:?} is exerted")]
    CharacterExerted(CardId),
    /// The challenge target is not an opposing card in play.
    #[error("challenge target {0:?} is not an opposing card in play")]
    TargetNotInPlay(CardId),
    /// The challenge target is not a character.
    #[error("challenge target {0:?} is not a character")]
    TargetNotACharacter(CardId),
    /// The challenge target is ready; only exerted characters can be challenged
    /// (§4.3.6.7).
    #[error("challenge target {0:?} is not exerted")]
    TargetNotExerted(CardId),
    /// A turn action was submitted while the engine is awaiting a decision; the
    /// pending decision must be answered first (§8.7).
    #[error("the engine is awaiting a decision; answer it before acting")]
    AwaitingDecision,
    /// A `Decide` input was submitted when no decision is pending.
    #[error("there is no pending decision to answer")]
    NoPendingDecision,
    /// The `Decide` answer doesn't match the pending decision.
    #[error("that answer does not match the pending decision")]
    InvalidDecision,
    /// The card has no activated ability at the given index.
    #[error("card {0:?} has no activated ability at the given index")]
    NoSuchAbility(CardId),
}
