//! Effects produced by abilities.

use super::target::Target;
use crate::domain::cards::Keyword;
use crate::domain::game::{Permission, Restriction};
use serde::{Deserialize, Serialize};

/// When a delayed (floating) triggered ability fires (§7.4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelayedWhen {
    /// At the end of the current turn ("at the end of this turn, …").
    EndOfTurn,
}

/// Where a card returned to a deck goes (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeckPosition {
    /// On top of the owner's deck.
    Top,
    /// On the bottom of the owner's deck.
    Bottom,
    /// Shuffled into the owner's deck.
    Shuffle,
}

/// A built-in effect an ability or action can produce.
///
/// The structured effect / target / condition DSL (Slice 8): untargeted effects
/// (draw, lore), and `Target`-based effects (`SelfCard` / `ChosenCharacter` /
/// `AllCharacters` / `UpToCharacters` / `ChosenItem` / `ChosenLocation`, filtered
/// by side / classification / name / cost / `{S}` / damaged / exerted) resolved
/// via the `ChooseTarget` / `ChooseUpToN` pending decisions, "[A] then [B]"
/// sequencing, and a board-condition guard (`IfControl`).
///
/// TODO(remaining — Slice 8b+): replacement effects (§7.7, e.g. "takes no damage
/// from the challenge"), conditional-on-the-chosen-target ("if a Villain is
/// chosen, … instead"), player targets, and effect-granted keywords. An
/// `Effect::Custom(name)` escape hatch (compiled-in handler) remains the plan for
/// the rare card the DSL can't express.
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
    /// Return the target card to its owner's deck (top / bottom / shuffled in),
    /// e.g. "shuffle chosen character into their player's deck" (§8.2).
    ReturnToDeck {
        /// Who is returned.
        target: Target,
        /// Where in the deck.
        position: DeckPosition,
    },
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
    /// Exert the target ("exert chosen opposing character").
    Exert(Target),
    /// Ready the target ("ready this character" / "ready chosen character").
    Ready(Target),
    /// Give the target a keyword until end of turn ("chosen character gains
    /// Challenger +2 this turn", "gains Evasive", §10).
    GrantKeywordThisTurn {
        /// Who gains the keyword.
        target: Target,
        /// The granted keyword.
        keyword: Keyword,
    },
    /// Place a prevention on the target until end of turn ("can't quest", "can't
    /// be challenged this turn", §1.2.2).
    RestrictThisTurn {
        /// Who is restricted.
        target: Target,
        /// The prevention.
        restriction: Restriction,
    },
    /// Grant the target a permission until end of turn ("can challenge ready
    /// characters this turn", Pick a Fight).
    PermitThisTurn {
        /// Who gains the permission.
        target: Target,
        /// The permission.
        permission: Permission,
    },
    /// Choose `target`, then apply `then` to it if it matches `filter`, else
    /// `otherwise` ("Chosen character gets +2 {S}; if a Villain character is
    /// chosen, they get +3 instead"). `then`/`otherwise` apply to the **chosen
    /// target** (their own inner target is ignored).
    IfTargetMatches {
        /// Who is chosen.
        target: Target,
        /// The condition tested against the chosen target.
        filter: super::target::CharacterFilter,
        /// Applied to the target when it matches.
        then: Box<Self>,
        /// Applied to the target when it doesn't.
        otherwise: Box<Self>,
    },
    /// Schedule a one-shot **delayed** effect to resolve later (§7.4.7), e.g.
    /// "at the end of this turn, banish this character".
    ScheduleDelayed {
        /// When the delayed effect fires.
        when: DelayedWhen,
        /// The effect resolved at that time.
        effect: Box<Self>,
    },
    /// Resolve `then` only if the controller has at least one in-play character
    /// matching `filter` ("if you have a character named X in play, …", §7.1).
    IfControl {
        /// The board condition: the controller must have a matching character.
        filter: super::target::CharacterFilter,
        /// The effect to resolve when the condition holds.
        then: Box<Self>,
    },
}
