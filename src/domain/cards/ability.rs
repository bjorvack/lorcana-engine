//! Card abilities.

use crate::domain::effects::{Effect, TriggerCondition};
use crate::domain::game::Stat;
use crate::domain::types::card::Classification;
use serde::{Deserialize, Serialize};

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
    /// `true` if the effect is a "you may" (optional) effect.
    pub optional: bool,
    /// What the ability does when it resolves.
    pub effect: Effect,
}

impl TriggeredAbility {
    /// Create a (mandatory) triggered ability.
    #[must_use]
    pub const fn new(condition: TriggerCondition, effect: Effect) -> Self {
        Self {
            condition,
            optional: false,
            effect,
        }
    }

    /// Create an optional ("you may") triggered ability.
    #[must_use]
    pub const fn optional(condition: TriggerCondition, effect: Effect) -> Self {
        Self {
            condition,
            optional: true,
            effect,
        }
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
}

impl StaticAbility {
    /// Create a self static modifier ("this character gets +delta {stat}").
    #[must_use]
    pub const fn self_modifier(stat: Stat, delta: i32) -> Self {
        Self {
            target: StaticTarget::SelfCard,
            stat,
            delta,
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
        }
    }
}
