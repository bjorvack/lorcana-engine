//! Effects produced by abilities.

use serde::{Deserialize, Serialize};

/// A built-in effect an ability can produce.
///
/// Kept minimal for Slice 4 — just enough to make triggered abilities
/// observable and testable. These are the no-target effects common on
/// enters-play and quest triggers (e.g. "draw a card", "each opponent loses 1
/// lore").
///
/// TODO(effect DSL — Slice 8): grow this into the structured effect / target /
/// condition DSL described in the architecture. Targeted effects ("deal N damage
/// to chosen character", "return chosen character to hand"), conditional and
/// "up to N" effects, modifiers, and replacement effects need a `Target`
/// selector and player choices (which is why those are deferred until the
/// `PendingDecision` machinery is fully in place). An `Effect::Custom(name)`
/// escape hatch (compiled-in handler) remains the plan for the rare card the DSL
/// can't express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    /// The controller draws this many cards.
    DrawCards(u32),
    /// The controller gains this much lore.
    GainLore(u32),
    /// Each opponent of the controller loses this much lore (clamped at 0).
    EachOpponentLosesLore(u32),
}
