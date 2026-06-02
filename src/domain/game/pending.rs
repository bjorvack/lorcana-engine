//! A decision the engine is waiting on before it can continue resolving.

use super::bag::TriggerId;
use crate::domain::effects::Effect;
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
            | Self::ChooseUpToN { player, .. } => *player,
        }
    }
}
