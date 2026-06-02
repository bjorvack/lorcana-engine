//! Game events: the engine's output log.
//!
//! Events describe what happened as inputs are applied. They are produced by the
//! engine (see `domain::engine`) and consumed by a host (UI, CLI, tests). Events
//! are outputs only — they are never used to drive the game.

use crate::domain::types::ids::{CardId, PlayerId};
use crate::domain::types::turn::Step;
use serde::{Deserialize, Serialize};

/// Something that happened in the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameEvent {
    /// Opening hands were dealt to all players (§3.1.5).
    HandsDealt,
    /// A player finished their mulligan, returning `returned` cards (§3.1.6).
    MulliganResolved {
        /// The player who altered their hand.
        player: PlayerId,
        /// How many cards they put on the bottom of their deck.
        returned: u32,
    },
    /// A player's turn began.
    TurnStarted {
        /// The new active player.
        player: PlayerId,
        /// The turn number (1-based).
        turn: u32,
    },
    /// A step within the current phase was entered.
    StepEntered {
        /// The step now in progress.
        step: Step,
    },
    /// A player drew a card from their deck (§4.2.3).
    CardDrawn {
        /// The player who drew.
        player: PlayerId,
        /// The card drawn into their hand.
        card: CardId,
    },
    /// A player attempted to draw from an empty deck (§1.9.1.2 loss pending).
    DeckEmptyOnDraw {
        /// The player who could not draw.
        player: PlayerId,
    },
    /// A player put a card into their inkwell (§4.3.3).
    CardPutInInkwell {
        /// The player who inked.
        player: PlayerId,
        /// The card moved to the inkwell.
        card: CardId,
    },
    /// A player played a card into their play area (§4.3.4).
    CardPlayed {
        /// The player who played the card.
        player: PlayerId,
        /// The card now in play.
        card: CardId,
    },
    /// A character was sent on a quest (§4.3.5).
    Quested {
        /// The questing player.
        player: PlayerId,
        /// The questing character.
        character: CardId,
    },
    /// A player gained lore.
    LoreGained {
        /// The player who gained lore.
        player: PlayerId,
        /// How much lore was gained.
        amount: u32,
    },
    /// A player lost lore (clamped at 0).
    LoreLost {
        /// The player who lost lore.
        player: PlayerId,
        /// How much lore was lost.
        amount: u32,
    },
    /// A character challenged an opposing character (§4.3.6).
    Challenged {
        /// The challenging player.
        player: PlayerId,
        /// The challenging character.
        challenger: CardId,
        /// The character being challenged.
        target: CardId,
    },
    /// A card was banished from play to its owner's discard (§1.9.1.3, §8.6.2).
    Banished {
        /// The owner of the banished card.
        player: PlayerId,
        /// The banished card.
        card: CardId,
    },
    /// A player used an activated ability (§7.5).
    AbilityActivated {
        /// The player who used the ability.
        player: PlayerId,
        /// The source card whose ability was used.
        card: CardId,
    },
    /// A player ended their turn (§4.4).
    TurnEnded {
        /// The player whose turn ended.
        player: PlayerId,
    },
    /// A player lost the game (§1.9.1.2, §3.2.1.2).
    PlayerLost {
        /// The player who lost.
        player: PlayerId,
    },
    /// The game ended. `winners` is empty for a draw, one entry in the usual
    /// case, or several for a simultaneous multiplayer win.
    GameEnded {
        /// The winning player(s).
        winners: Vec<PlayerId>,
    },
}
