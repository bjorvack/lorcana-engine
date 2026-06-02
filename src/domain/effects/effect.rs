//! Effects produced by abilities.

use super::target::Target;
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
// Not `Copy`: targeted variants carry a `Target`, which can hold a
// `CharacterFilter` with classification strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    /// The controller draws this many cards.
    DrawCards(u32),
    /// The controller gains this much lore.
    GainLore(u32),
    /// Each opponent of the controller loses this much lore (clamped at 0).
    EachOpponentLosesLore(u32),
    /// Move the target card to its owner's hand (§7; e.g. Marshmallow "return
    /// this card to your hand"). For `Target::SelfCard` the source returns itself
    /// — including from the discard, where banishment leaves it.
    ReturnToHand(Target),
    /// Put the target card into its owner's inkwell facedown and exerted (Gramma
    /// Tala "into your inkwell facedown and exerted").
    IntoInkwell(Target),
    /// Give the target character `amount` Strength `{S}` until end of turn (e.g.
    /// Support adds the source's current `{S}`; "gets +N {S} this turn").
    GiveStrengthThisTurn {
        /// Who is buffed/debuffed.
        target: Target,
        /// The signed `{S}` change.
        amount: i32,
    },
    /// Deal `amount` damage to the target character (§4.3.6.16, §9). Lethal damage
    /// banishes it at the next game-state check.
    DealDamage {
        /// Who takes the damage.
        target: Target,
        /// How much damage.
        amount: u32,
    },
    /// Remove up to `amount` damage from the target character (§9.4; "remove up to
    /// N damage from chosen character").
    RemoveDamage {
        /// Whose damage is removed.
        target: Target,
        /// How much damage to remove (clamped at 0).
        amount: u32,
    },
    /// Banish the target directly (not via damage) — "banish chosen character".
    Banish(Target),
    /// Resolve `then` only if the controller has at least one in-play character
    /// matching `filter` ("if you have a character named X in play, …", §7.1).
    IfControl {
        /// The board condition: the controller must have a matching character.
        filter: super::target::CharacterFilter,
        /// The effect to resolve when the condition holds.
        then: Box<Self>,
    },
}
