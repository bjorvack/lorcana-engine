//! A decision the engine is waiting on before it can continue resolving.

use super::bag::TriggerId;
use crate::domain::effects::{Amount, DeckPosition, Destination, DiscardAmount, Effect};
use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};

/// A point in bag resolution that requires a player's input before the engine
/// can proceed. While a decision is pending, only a matching `Decide` input is
/// accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingDecision {
    /// The player has two or more triggered abilities in the bag and must choose
    /// which to resolve next (§8.7.4–§8.7.5).
    OrderTriggers {
        /// The player who must choose.
        player: PlayerId,
        /// The candidate triggers (their own bag entries).
        options: Vec<TriggerId>,
    },
    /// An optional ("you may") triggered ability is resolving; the player chooses
    /// whether to apply it (§7.1.3).
    MayResolve {
        /// The player who must choose.
        player: PlayerId,
        /// The optional trigger awaiting a yes/no.
        trigger: TriggerId,
    },
    /// A Bodyguard character just entered play; its controller chooses whether it
    /// enters exerted instead of ready (§10.3.2).
    EnterPlayExerted {
        /// The player who must choose.
        player: PlayerId,
        /// The Bodyguard character that just entered play.
        card: CardId,
    },
    /// A targeted effect is resolving and its controller must choose a target
    /// from `options`; the stashed `effect` is applied to the pick, then any
    /// `rest` effects resolve in order (§7.1.2, §7.1).
    ChooseTarget {
        /// The player who must choose.
        player: PlayerId,
        /// The effect's source card.
        source: CardId,
        /// The eligible targets.
        options: Vec<CardId>,
        /// The effect to apply to the chosen target.
        effect: Effect,
        /// The remaining effects of the same ability/action, resolved in order
        /// after this one (the "[A] then [B]" continuation).
        rest: Vec<Effect>,
    },
    /// A discard effect is resolving and `player` must choose exactly `count`
    /// cards from their own hand to discard. Afterwards the `remaining_players`
    /// each discard per `amount` in turn; then `rest` resolves (§8.4, §7.1).
    ChooseCardsToDiscard {
        /// The player who must choose (the discarding player).
        player: PlayerId,
        /// The effect's source card (for resuming the continuation's controller).
        source: CardId,
        /// How many cards must be chosen.
        count: u32,
        /// How much each remaining player discards.
        amount: DiscardAmount,
        /// Players that still discard after this one (multi-player scope).
        remaining_players: Vec<PlayerId>,
        /// The remaining effects of the ability/action, resolved in order after.
        rest: Vec<Effect>,
    },
    /// A "play a card for free" effect is resolving; `player` chooses one of
    /// `options` (from hand) to play; then `rest` resolves (§6). Optionality is
    /// expressed by wrapping in `Effect::May` (see `MayResolveEffect`).
    ChoosePlayFree {
        /// The player who must choose.
        player: PlayerId,
        /// The effect's source card (continuation controller).
        source: CardId,
        /// The eligible cards to play for free.
        options: Vec<CardId>,
        /// The remaining effects, resolved in order after.
        rest: Vec<Effect>,
    },
    /// A "look at the top N" effect is resolving; `player` chooses up to one of
    /// `options` (the eligible looked-at cards) to take into hand, then the rest of
    /// `looked_at` go to `rest_position` and `rest` resolves (§8.2).
    ChooseFromRevealed {
        /// The player who chooses and receives the taken card (the looker).
        player: PlayerId,
        /// The effect's source card (continuation controller).
        source: CardId,
        /// Whose deck was looked at (where the rest go); usually == `player`.
        deck_owner: PlayerId,
        /// All the looked-at cards (top of deck), in deck order.
        looked_at: Vec<CardId>,
        /// The subset of `looked_at` that may be taken into hand.
        options: Vec<CardId>,
        /// Where the cards that aren't taken go.
        rest_position: DeckPosition,
        /// The remaining effects, resolved in order after.
        rest: Vec<Effect>,
    },
    /// A player-scoped effect with a `Chosen*` scope is resolving; `player` chooses
    /// one player from `options`, the effect re-targets onto them, then `rest`
    /// resolves (§7.1; multiplayer "chosen opponent / player").
    ChoosePlayer {
        /// The player who must choose.
        player: PlayerId,
        /// The effect's source card.
        source: CardId,
        /// The candidate players.
        options: Vec<PlayerId>,
        /// The player-scoped effect to re-target onto the chosen player.
        effect: Effect,
        /// The remaining effects, resolved in order after.
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
    /// A two-target move-damage is resolving; `player` picks one endpoint from
    /// `options`, then `effect` is re-run with it resolved (§9.3).
    ChooseMoveTarget {
        /// The player choosing the endpoint.
        player: PlayerId,
        /// The effect's source card.
        source: CardId,
        /// The candidate cards.
        options: Vec<CardId>,
        /// The move-damage effect to re-run with this endpoint resolved.
        effect: Effect,
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
    /// "Up to N" — the controller chooses 0..`max` distinct targets from
    /// `options`; `effect` applies to each, then `rest` resolves (§7.1.8).
    ChooseUpToN {
        /// The player who must choose.
        player: PlayerId,
        /// The effect's source card.
        source: CardId,
        /// The eligible targets.
        options: Vec<CardId>,
        /// The maximum number of distinct targets that may be chosen.
        max: u32,
        /// The effect applied to each chosen target.
        effect: Effect,
        /// The remaining effects of the ability/action, resolved in order after.
        rest: Vec<Effect>,
    },
}

impl PendingDecision {
    /// The player who must answer this decision.
    #[must_use]
    pub const fn player(&self) -> PlayerId {
        match self {
            Self::OrderTriggers { player, .. }
            | Self::MayResolve { player, .. }
            | Self::EnterPlayExerted { player, .. }
            | Self::ChooseTarget { player, .. }
            | Self::ChooseCardsToDiscard { player, .. }
            | Self::ChoosePlayFree { player, .. }
            | Self::ChooseFromRevealed { player, .. }
            | Self::ChoosePlayer { player, .. }
            | Self::NameCard { player, .. }
            | Self::NameThenRecur { player, .. }
            | Self::ChooseMoveTarget { player, .. }
            | Self::MayResolveEffect { player, .. }
            | Self::ChooseUpToN { player, .. } => *player,
        }
    }
}
