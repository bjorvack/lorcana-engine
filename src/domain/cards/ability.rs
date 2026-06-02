//! Card abilities.

use crate::domain::effects::{Effect, TriggerCondition};
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
