//! A decision the engine is waiting on before it can continue resolving.

use super::bag::TriggerId;
use crate::domain::effects::{Amount, DeckPosition, Destination, DiscardAmount, DiscardBy, Effect};
use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};

/// A reference a [`PendingDecision::Choose`] can pick — a card or a player. The
/// unified currency for "choose from a set of options" decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChoiceRef {
    /// An in-play / zoned card.
    Card(CardId),
    /// A player.
    Player(PlayerId),
}

/// What to do with the pick(s) once a [`PendingDecision::Choose`] resolves. The
/// continuation of the general choose primitive (grows as more bespoke choices
/// migrate onto it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChoiceThen {
    /// Substitute the (single) pick into `effect`, then resolve it + the rest
    /// ("choose a player/character; the effect re-targets onto it", §7.1).
    SubstituteAndResolve(Box<Effect>),
    /// Apply `effect` to each picked card, then resolve the rest ("chosen
    /// character … " / "up to N chosen characters …", §7.1 / §7.1.8).
    ApplyToEach(Box<Effect>),
    /// Apply each of `effects` (in order) to each picked card, then resolve the
    /// rest — "[A] then [B] to the same chosen character" ([`Effect::OnTarget`]).
    ApplyAllTo(Vec<Effect>),
    /// Play each picked card for free (§6).
    PlayFree,
    /// Take the (up-to-one) picked card from `deck_owner`'s deck into hand; the
    /// rest of `looked_at` go to `rest_position` (look-at-top, §8.2).
    TakeRevealed {
        /// Whose deck the looked-at cards came from.
        deck_owner: PlayerId,
        /// All looked-at cards (the non-taken ones return to the deck).
        looked_at: Vec<CardId>,
        /// Where the non-taken cards go.
        rest_position: DeckPosition,
    },
    /// Take a picked card from `deck_owner`'s deck into hand; the rest of
    /// `looked_at` go to per-card destinations (split top/bottom, §8.2).
    TakeRevealedPerCard {
        /// Whose deck the looked-at cards came from.
        deck_owner: PlayerId,
        /// All looked-at cards (the non-taken ones return to the deck).
        looked_at: Vec<CardId>,
        /// Per-card destinations for each looked-at card (in order).
        destinations: Vec<DeckPosition>,
    },
    /// Discard the picked cards, then continue the discard down `remaining_players`
    /// (each discards per `amount` in turn, §8.4).
    Discard {
        /// How much each remaining player discards.
        amount: DiscardAmount,
        /// How the remaining players' discards are selected.
        by: DiscardBy,
        /// The players who still need to discard, in order.
        remaining_players: Vec<PlayerId>,
    },
    /// Discard each picked card from `owner`'s hand (the chooser is someone else —
    /// "chosen opponent … discards a card of your choice", §8.4).
    DiscardFrom {
        /// Whose hand the picked cards are discarded from.
        owner: PlayerId,
    },
    /// Take the picked cards from `deck_owner`'s deck into hand, then shuffle the deck
    /// (search deck, §8.2).
    SearchDeckTake {
        /// Whose deck the cards were taken from.
        deck_owner: PlayerId,
    },
    /// Move the picked card(s) to `to` ("return a card from your discard to your
    /// hand"; "put a card from your hand into your inkwell"). The card is taken
    /// from wherever it is by `move_self_card`.
    MoveChosenTo {
        /// Whose card(s) (discard/hand) are moved.
        owner: PlayerId,
        /// Where the picked cards go.
        to: Destination,
    },
}

/// A point in bag resolution that requires a player's input before the engine
/// can proceed. While a decision is pending, only a matching `Decide` input is
/// accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingDecision {
    /// The general "choose `min..=max` of `options`, then run `then`" decision —
    /// the primitive the bespoke choose variants are migrating onto (§7.1).
    Choose {
        /// Who chooses.
        player: PlayerId,
        /// The effect's source card (continuation controller).
        source: CardId,
        /// The candidates.
        options: Vec<ChoiceRef>,
        /// Minimum number to pick.
        min: u32,
        /// Maximum number to pick.
        max: u32,
        /// What to do with the pick(s).
        then: ChoiceThen,
        /// The remaining effects, resolved in order after.
        rest: Vec<Effect>,
    },
    /// The player has two or more triggered abilities in the bag and must choose
    /// which to resolve next (§8.7.4–§8.7.5).
    OrderTriggers {
        /// The player who must choose.
        player: PlayerId,
        /// The candidate triggers (their own bag entries).
        options: Vec<TriggerId>,
    },
    /// A Bodyguard character just entered play; its controller chooses whether it
    /// enters exerted instead of ready (§10.3.2).
    EnterPlayExerted {
        /// The player who must choose.
        player: PlayerId,
        /// The Bodyguard character that just entered play.
        card: CardId,
        /// The source of the effect that played it (for resuming `rest`).
        cont_source: CardId,
        /// Remaining effects to resolve after the choice (non-empty only when a
        /// Bodyguard is played mid-effect, e.g. "play a character for free, then …").
        rest: Vec<Effect>,
    },
    /// A "name a card, then reveal the top of your deck" effect is resolving;
    /// `player` names a card and the revealed top is matched against it (§8.2).
    NameCard {
        /// The player naming the card (and revealing their deck).
        player: PlayerId,
        /// The effect's source card.
        source: CardId,
        /// Lore gained on a match.
        lore_on_match: Amount,
        /// Where the revealed card goes on a match.
        match_to: Destination,
        /// Where it goes otherwise.
        otherwise_to: Destination,
        /// The remaining effects, resolved in order after.
        rest: Vec<Effect>,
    },
    /// A "name a card, then return all character cards with that name from your
    /// discard to your hand" effect is resolving (Blast from Your Past, §8.2).
    NameThenRecur {
        /// The player naming the card (and recurring from their discard).
        player: PlayerId,
        /// The effect's source card.
        source: CardId,
        /// The remaining effects, resolved in order after.
        rest: Vec<Effect>,
    },

    /// A `May` effect is resolving; `player` chooses whether to resolve `effect`
    /// ("you may …", §7.1.3). `rest` resolves afterwards either way.
    MayResolveEffect {
        /// The player who must choose.
        player: PlayerId,
        /// The effect's source card (continuation controller).
        source: CardId,
        /// The effect resolved only if the player agrees.
        effect: Effect,
        /// The remaining effects, resolved in order after.
        rest: Vec<Effect>,
    },
    /// A "Choose one" modal effect is resolving; `player` picks one of the
    /// offered effects to resolve (§7.1.9). `rest` resolves afterwards.
    ChooseOne {
        /// The player who must choose.
        player: PlayerId,
        /// The effect's source card (continuation controller).
        source: CardId,
        /// The offered effects (2–4 options in practice).
        options: Vec<Effect>,
        /// The remaining effects, resolved in order after.
        rest: Vec<Effect>,
    },
}

impl PendingDecision {
    /// The player who must answer this decision.
    #[must_use]
    pub const fn player(&self) -> PlayerId {
        match self {
            Self::OrderTriggers { player, .. }
            | Self::EnterPlayExerted { player, .. }
            | Self::Choose { player, .. }
            | Self::NameCard { player, .. }
            | Self::NameThenRecur { player, .. }
            | Self::MayResolveEffect { player, .. }
            | Self::ChooseOne { player, .. } => *player,
        }
    }
}
