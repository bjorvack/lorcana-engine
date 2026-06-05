//! Card abilities.

use crate::domain::effects::Amount;
use crate::domain::effects::{Effect, TriggerCondition};
use crate::domain::game::{Condition, Stat};
use crate::domain::types::card::Classification;
use serde::{Deserialize, Serialize};

/// When a triggered ability is allowed to fire relative to whose turn it is (§4.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnGate {
    /// Fires on any player's turn (the default).
    #[default]
    AnyTurn,
    /// "During your turn, …" — only while the controller is the active player.
    YourTurn,
    /// "During the opponent's turn, …" — only while the controller is *not* the
    /// active player.
    OpponentsTurn,
}

impl TurnGate {
    /// Whether a trigger with this gate may fire for `controller` while `active`
    /// is the active player.
    #[must_use]
    pub const fn allows(self, controller_is_active: bool) -> bool {
        match self {
            Self::AnyTurn => true,
            Self::YourTurn => controller_is_active,
            Self::OpponentsTurn => !controller_is_active,
        }
    }
}

/// A triggered ability (§7.4): when its `condition` is met its `effect` is added
/// to the bag to resolve.
///
/// `optional` captures abilities worded with "you may" (§7.1.3): the controller
/// chooses whether to resolve the effect.
///
/// TODO: activated, static, and replacement abilities (§7.5–§7.7) are separate
/// ability kinds added in later slices; only triggered abilities exist so far.
// Not `Copy` on purpose: `Effect` will gain non-`Copy` variants (targets,
// names) when the effect DSL lands (Slice 8), so deriving `Copy` now would be
// churn to undo later.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggeredAbility {
    /// What makes the ability fire.
    pub condition: TriggerCondition,
    /// When the trigger may fire relative to whose turn it is ("During your turn,
    /// …" / "During the opponent's turn, …"; §4.1). Defaults to any turn.
    pub turn_gate: TurnGate,
    /// What the ability does when it resolves. Optionality ("you may …") is
    /// expressed by wrapping this in [`Effect::May`] — there is no separate
    /// `optional` flag (the algebra composes it onto any effect).
    pub effect: Effect,
}

impl TriggeredAbility {
    /// Create a (mandatory) triggered ability.
    #[must_use]
    pub const fn new(condition: TriggerCondition, effect: Effect) -> Self {
        Self {
            condition,
            turn_gate: TurnGate::AnyTurn,
            effect,
        }
    }

    /// Create an optional ("you may") triggered ability — sugar for wrapping the
    /// effect in [`Effect::May`].
    #[must_use]
    pub fn optional(condition: TriggerCondition, effect: Effect) -> Self {
        Self::new(condition, Effect::May(Box::new(effect)))
    }

    /// Gate this trigger to the controller's own turn ("During your turn, …").
    #[must_use]
    pub const fn during_your_turn(mut self) -> Self {
        self.turn_gate = TurnGate::YourTurn;
        self
    }

    /// Gate this trigger to the opponent's turn ("During the opponent's turn, …").
    #[must_use]
    pub const fn during_opponents_turn(mut self) -> Self {
        self.turn_gate = TurnGate::OpponentsTurn;
        self
    }
}

/// The cost to use an activated ability (§7.5, written `[Cost] — [Effect]`).
///
/// Models the dominant shapes from the card pool: `{E}` (exert the source) and
/// `N {I}` (pay ink), alone or combined.
///
/// TODO(cost atoms — Slice 5a): add the remaining activation-cost atoms found in
/// the survey — banish-this (items, ~34 abilities) and discard-a-card — as
/// fields/variants when a card needs them. See `docs/planning/IMPLEMENTATION_PLAN.md`
/// ("Slice 5a").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityCost {
    /// Exert the source card (`{E}`). The source must be ready and not drying
    /// (§4.2.2.1).
    pub exert_self: bool,
    /// Ink to pay (`N {I}`), exerted from the inkwell like a card's cost.
    pub ink: u32,
}

impl AbilityCost {
    /// A cost of exerting the source and paying `ink`.
    #[must_use]
    pub const fn new(exert_self: bool, ink: u32) -> Self {
        Self { exert_self, ink }
    }

    /// The `{E}` cost (exert the source only).
    #[must_use]
    pub const fn exert() -> Self {
        Self::new(true, 0)
    }
}

/// An activated ability: a cost the active player may pay to resolve an effect
/// immediately (§7.5).
// Not `Copy` on purpose: `Effect` will gain non-`Copy` variants (effect DSL,
// Slice 8), so deriving `Copy` now would be churn to undo later.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivatedAbility {
    /// The cost to use the ability.
    pub cost: AbilityCost,
    /// The effect produced when the ability is used.
    pub effect: Effect,
}

impl ActivatedAbility {
    /// Create an activated ability.
    #[must_use]
    pub const fn new(cost: AbilityCost, effect: Effect) -> Self {
        Self { cost, effect }
    }
}

/// Which cards a static ability's modifier applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaticTarget {
    /// The source card itself ("this character gets +N {S}").
    SelfCard,
    /// The controller's characters with any of the given classifications (empty
    /// ⇒ all of the controller's characters), optionally including the source
    /// ("your [other] [classification] characters get +N {S}").
    OwnedCharacters {
        /// Required classifications (any-of); empty matches every character.
        classifications: Vec<Classification>,
        /// Whether the source itself is included.
        include_self: bool,
    },
}

/// A static ability that continuously modifies a characteristic while the card
/// is in play (§7.6).
///
/// TODO(duration — Slice 5f): add timed statics ("until end of turn"); a resolved
/// timed effect must snapshot its targets and not affect later-entering cards
/// (§7.6.3), unlike the continuous statics here. See
/// `docs/planning/IMPLEMENTATION_PLAN.md` ("Slice 5f").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticAbility {
    /// Which cards the modifier applies to.
    pub target: StaticTarget,
    /// The characteristic modified.
    pub stat: Stat,
    /// The signed amount (e.g. `+2` or `-1`).
    pub delta: i32,
    /// A condition gating the modifier ("while this character is exerted, …"); it
    /// applies only while the condition holds. `None` means always.
    pub condition: Option<Condition>,
    /// If set, the effective delta is `delta × count` — a dynamic "+N {stat} for
    /// each …" static (e.g. "+1 {L} for each other Villain you have in play").
    pub per: Option<Amount>,
}

/// A static ability that modifies a **game rule** while the card is in play (the
/// win/loss modification layer, §1.2.1).
///
/// TODO(modification layer — Slice 5g+): only the lore-to-win override exists so
/// far. The add / remove-suppress space ("you can't lose", "opponents can't
/// win", added alternate wins) is enumerated in the `win_loss.rs` test TODO and
/// lands as more cards need it. See `docs/planning/IMPLEMENTATION_PLAN.md`
/// ("Slice 5g").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameRuleStatic {
    /// "Opponents need `threshold` lore to win the game" (Donald Duck –
    /// Flustered Sorcerer).
    OpponentsLoreToWin(u32),
}

impl StaticAbility {
    /// Create a self static modifier ("this character gets +delta {stat}").
    #[must_use]
    pub const fn self_modifier(stat: Stat, delta: i32) -> Self {
        Self {
            target: StaticTarget::SelfCard,
            stat,
            delta,
            condition: None,
            per: None,
        }
    }

    /// Create a selector static modifier over the controller's characters.
    #[must_use]
    pub const fn owned_characters(
        classifications: Vec<Classification>,
        include_self: bool,
        stat: Stat,
        delta: i32,
    ) -> Self {
        Self {
            target: StaticTarget::OwnedCharacters {
                classifications,
                include_self,
            },
            stat,
            delta,
            condition: None,
            per: None,
        }
    }

    /// Gate this static ability on a [`Condition`] (builder).
    #[must_use]
    pub const fn with_condition(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Make this static's delta scale by a live [`Amount`] (builder), for "+N
    /// {stat} for each …".
    #[must_use]
    pub fn with_count(mut self, per: Amount) -> Self {
        self.per = Some(per);
        self
    }
}
