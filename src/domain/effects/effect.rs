//! Effects produced by abilities.

use super::target::{CharacterFilter, Target};
use super::trigger::TriggerCondition;
use crate::domain::game::{Property, Stat};
use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};

/// A numeric amount used by effects (damage, lore, draws, `{S}` change). Either a
/// fixed value or one computed at resolution — "for each …" / "equal to …".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Amount {
    /// A constant amount.
    Fixed(i32),
    /// The number of in-play characters matching `filter` (from the controller's
    /// perspective) — "for each [character] you have in play", "equal to the
    /// number of characters you have in play".
    PerMatchingCharacter(CharacterFilter),
    /// The current value of a characteristic on a character: the effect's source
    /// (`Target::SelfCard` — "equal to this character's {S}") or the effect's
    /// resolved target (any other `target` — "+{S} equal to their {W}"). §7.8.
    StatOf {
        /// Which characteristic to read.
        stat: Stat,
        /// Whose: `SelfCard` = the source; anything else = the resolved target.
        target: Target,
    },
    /// The number of cards in the controller's hand ("for each card in your hand").
    CardsInHand,
    /// The number of damage counters on the effect's source ("for each 1 damage on
    /// her").
    DamageOnSource,
    /// The amount carried by the triggering event — "deal **that much** damage",
    /// "draw **that many** cards". Only meaningful inside a triggered ability whose
    /// trigger supplies a number (e.g. damage dealt); the firing site substitutes
    /// the concrete value when the trigger is enqueued. Evaluates to 0 otherwise.
    TriggerAmount,
}

impl Amount {
    /// A constant amount (the common case).
    #[must_use]
    pub const fn fixed(n: i32) -> Self {
        Self::Fixed(n)
    }

    /// Substitute the triggering event's value for [`Amount::TriggerAmount`] ("that
    /// much"), turning it into a concrete [`Amount::Fixed`]. Other amounts are
    /// unchanged.
    #[must_use]
    pub fn with_trigger_amount(self, value: i32) -> Self {
        match self {
            Self::TriggerAmount => Self::Fixed(value),
            other => other,
        }
    }
}

/// Count-based conditions for effect gating ("if you have more than 3 cards in
/// your hand", "if you have more lore than each opponent", etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CountCondition {
    /// Controller has at least N cards in hand ("if you have 3 or more cards").
    HandSizeAtLeast(u32),
    /// Controller has more than N cards in hand ("if you have more than 3 cards").
    HandSizeMoreThan(u32),
    /// Controller has at least N lore ("if you have 3 or more lore").
    LoreAtLeast(u32),
    /// Controller has more than N lore ("if you have more than 3 lore").
    LoreMoreThan(u32),
    /// Controller has more lore than opponent ("if you have more lore than each opponent").
    LoreMoreThanOpponent,
}

/// What a [`Effect::Move`] selects to move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveSource {
    /// A target card (self / chosen / all-matching). Used for bounce, into-inkwell,
    /// and return-to-deck.
    Card(Target),
    /// The top `count` cards of each player in `who`'s deck (milling / digging).
    DeckTop {
        /// Whose deck.
        who: PlayerScope,
        /// How many off the top.
        count: Amount,
    },
    /// A single card chosen from `who`'s `zone` (discard / hand) matching `filter`
    /// — "return a character card from your discard to your hand", "put a card
    /// from your hand into your inkwell". Resolved to one pick and moved to the
    /// [`Effect::Move`] destination.
    ChosenFrom {
        /// Which zone to choose from.
        zone: SourceZone,
        /// Whose `zone` to choose from.
        who: PlayerScope,
        /// Which cards qualify (by printed predicates / category).
        filter: CharacterFilter,
    },
}

/// A non-play zone a [`MoveSource::ChosenFrom`] picks a card out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceZone {
    /// The discard pile.
    Discard,
    /// The hand.
    Hand,
}

/// Where a [`Effect::Move`] sends cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    /// The owner's hand ("return to hand" / bounce).
    Hand,
    /// The owner's inkwell, facedown and exerted.
    Inkwell,
    /// The owner's discard (e.g. milling the top of a deck).
    Discard,
    /// The owner's deck, at the given position.
    Deck(DeckPosition),
}

/// Which players an effect applies to.
///
/// The `Chosen*` variants require the controller to pick a player — a real
/// decision with 2+ candidates (3–4 player games); they auto-resolve when there's
/// only one candidate (e.g. a "chosen opponent" in a 2-player game). `Player` is
/// the resolved single target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerScope {
    /// The controller only ("you").
    You,
    /// Each opponent of the controller.
    EachOpponent,
    /// Every player (controller and opponents).
    EachPlayer,
    /// One opponent the controller chooses ("chosen opponent").
    ChosenOpponent,
    /// Any one player the controller chooses, including themselves ("chosen player").
    ChosenPlayer,
    /// A specific, already-resolved player (the outcome of a `Chosen*` choice).
    Player(PlayerId),
}

/// How the discarded cards are selected (§8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DiscardBy {
    /// The discarding player chooses which of their own cards (hand unseen by the
    /// chooser, "chooses and discards").
    #[default]
    Owner,
    /// Cards are chosen uniformly at random ("discards a card at random").
    Random,
}

/// How many cards a discard effect removes from a hand (§8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscardAmount {
    /// Exactly N cards (the player chooses which, unless the hand is smaller).
    Count(u32),
    /// The player's whole hand ("discard your hand").
    WholeHand,
}

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
    /// Each player in `who` draws `amount` cards ("draw 2 cards" = `You`; "each
    /// player draws 3"; "chosen player draws 5").
    Draw {
        /// Which players draw.
        who: PlayerScope,
        /// How many each draws.
        amount: Amount,
    },
    /// Each player in `who` changes their lore by `amount` (positive = gain,
    /// negative = lose, clamped at 0). "Gain 2 lore" = `You` +2; "each opponent
    /// loses 1 lore" = `EachOpponent` -1; "chosen opponent loses 1" = `ChosenOpponent`.
    Lore {
        /// Which players' lore changes.
        who: PlayerScope,
        /// The signed lore change applied to each.
        amount: Amount,
    },
    /// Move card(s) from one zone to another (§7, §8): the single zone-move
    /// primitive. Covers "return to hand" (bounce), "into your inkwell", "return
    /// to deck", and milling — `what` selects the cards, `to` is the destination.
    Move {
        /// Which cards move (a target card, or the top of a player's deck).
        what: MoveSource,
        /// Where they go.
        to: Destination,
    },
    /// Give the target character `amount` of `stat` (`{S}`/`{W}`/`{L}`) until end
    /// of turn ("gets +N {S}/{L} this turn"; Support adds the source's current
    /// `{S}`). §7.6.1.
    GiveStatThisTurn {
        /// Who is buffed/debuffed.
        target: Target,
        /// Which stat changes.
        stat: Stat,
        /// The signed change.
        amount: Amount,
    },
    /// Deal `amount` damage to the target character (§4.3.6.16, §9). Lethal damage
    /// banishes it at the next game-state check.
    DealDamage {
        /// Who takes the damage.
        target: Target,
        /// How much damage (evaluated and clamped at 0).
        amount: Amount,
    },
    /// Remove up to `amount` damage from the target character (§9.4; "remove up to
    /// N damage from chosen character").
    RemoveDamage {
        /// Whose damage is removed.
        target: Target,
        /// How much damage to remove (evaluated and clamped at 0).
        amount: Amount,
    },
    /// Move up to `amount` damage counters from `from` to `to` (§9.3; "move up to
    /// 2 damage from chosen character to this character"). One side is usually
    /// `SelfCard`; the other (chosen) side is resolved via targeting. The amount
    /// moved is capped by the damage actually on `from`.
    MoveDamage {
        /// The character losing damage.
        from: Target,
        /// The character receiving it.
        to: Target,
        /// The maximum to move ("up to N").
        amount: Amount,
    },
    /// Banish the target directly (not via damage) — "banish chosen character".
    Banish(Target),
    /// Exert the target ("exert chosen opposing character").
    Exert(Target),
    /// Ready the target ("ready this character" / "ready chosen character").
    Ready(Target),
    /// Freeze the target: it can't ready at the start of its next turn (the flag
    /// is consumed at that ready step). Does **not** exert — compose with
    /// [`Effect::Exert`] for "exert chosen character; it can't ready…".
    Freeze(Target),
    /// Grant the target a **triggered ability** until end of turn ("gains
    /// 'Whenever this character challenges, …' this turn", §7.6). The granted
    /// ability fires from the target alongside its printed triggers.
    GrantAbilityThisTurn {
        /// Who gains the ability.
        target: Target,
        /// When the granted ability fires.
        condition: TriggerCondition,
        /// What it does.
        effect: Box<Self>,
        /// Whether the granted trigger is a "you may" (optional).
        optional: bool,
    },
    /// Grant the target a continuous [`Property`] (keyword / restriction /
    /// permission) until end of turn — "gains Evasive", "can't quest", "can
    /// challenge ready characters this turn" (§10, §1.2.2).
    GrantThisTurn {
        /// Who is affected.
        target: Target,
        /// The granted property.
        property: Property,
    },
    /// Grant a property (keyword / restriction / permission) to the target
    /// **permanently** — it lasts while the target is in play ("gains Evasive",
    /// §7.6). Contrast [`Effect::GrantThisTurn`] (until end of turn).
    Grant {
        /// Who is affected.
        target: Target,
        /// The granted property.
        property: Property,
    },
    /// Grant a property to the target until its controller's **next ready step**
    /// — the timing for "can't ready / can't quest at the start of their next
    /// turn" effects (like freeze's `CantReady`, surviving the granter leaving).
    GrantNextTurn {
        /// Who is affected.
        target: Target,
        /// The granted property.
        property: Property,
    },
    /// Grant the target an **activated** ability until end of turn ("gains '{E} —
    /// Draw a card' this turn", §7.5). Usable like a printed activated ability.
    GrantActivatedThisTurn {
        /// Who gains the ability.
        target: Target,
        /// Ink cost to activate.
        ink: u32,
        /// Whether activating exerts the card.
        exert_self: bool,
        /// What the activated ability does.
        effect: Box<Self>,
    },
    /// Put the top `count` cards of the controller's deck under this character,
    /// facedown (Boost keyword, §10.4). Cards under a character are not in play.
    Boost {
        /// How many cards to put under (typically 1, but could be dynamic).
        count: Amount,
    },
    /// Choose `target`, then apply `then` to it if it matches `filter`, else
    /// `otherwise` ("Chosen character gets +2 {S}; if a Villain character is
    /// chosen, they get +3 instead"). `then`/`otherwise` apply to the **chosen
    /// target** (their own inner target is ignored).
    IfTargetMatches {
        /// Who is chosen.
        target: Target,
        /// The condition tested against the chosen target.
        filter: CharacterFilter,
        /// Applied to the target when it matches.
        then: Box<Self>,
        /// Applied to the target when it doesn't.
        otherwise: Box<Self>,
    },
    /// Players in `who` discard cards from their hand ("choose and discard 2
    /// cards"; "each opponent chooses and discards a card"; "discard your hand").
    /// Each discarding player chooses which of their own cards (§8.4).
    Discard {
        /// Which players discard.
        who: PlayerScope,
        /// How many each discards.
        amount: DiscardAmount,
        /// How the discarded cards are selected — the player's own choice
        /// (default) or at random (hand unseen, §8.4).
        by: DiscardBy,
    },
    /// The named players reveal their hand (an information event — hand contents
    /// are already known to the engine, §8.x; Dolores Madrigal / Copper / Nothing
    /// to Hide).
    RevealHand {
        /// Whose hand is revealed.
        whose: PlayerScope,
    },
    /// A chosen opponent reveals their hand and the **controller** picks a card
    /// matching `filter` for them to discard ("chosen opponent reveals their hand
    /// and discards an action card of your choice", Lenny / Timon / Goldie, §8.4).
    /// Hand contents are already known to the engine, so the reveal is implicit.
    OpponentDiscardsChosen {
        /// Whose hand (the revealing opponent), usually `ChosenOpponent`.
        whose: PlayerScope,
        /// Which of their hand cards the controller may pick to discard.
        filter: CharacterFilter,
    },
    /// The controller plays a card matching `filter` from their hand **for free**
    /// (no ink), choosing which eligible card (§6). Wrap in [`Effect::May`] for
    /// "you may play …".
    PlayFreeFromHand {
        /// Which hand cards are eligible.
        filter: CharacterFilter,
    },
    /// Look at the top `count` cards of `whose` deck; the controller may take up to
    /// `take_count` matching `filter` into their hand; the rest go to `rest` (§8.2).
    /// "Look at the top 4 cards … put a character into your hand, rest on the
    /// bottom"; `whose` is usually `You` but can be a chosen player's deck.
    /// `take_count` defaults to 1 for backward compatibility.
    LookAtTopAndTake {
        /// Whose deck is looked at (resolved to a single deck owner).
        whose: PlayerScope,
        /// How many cards to look at.
        count: u32,
        /// How many matching cards the controller may take (defaults to 1).
        take_count: u32,
        /// Which of the looked-at cards the controller may take into hand.
        filter: CharacterFilter,
        /// Where the cards that aren't taken go (in the looked-at player's deck).
        /// If `rest_per_card` is Some, this field is ignored.
        rest: DeckPosition,
        /// Whether the controller may reorder the looked-at cards before taking.
        reorder: bool,
        /// Optional per-card destinations (for split top/bottom effects like
        /// Dr. Facilier). If Some, specifies the destination for each looked-at card
        /// in order; cards taken to hand are not included. If None, `rest` is used
        /// for all non-taken cards.
        rest_per_card: Option<Vec<DeckPosition>>,
    },
    /// Search `whose` deck for up to `take_count` cards matching `filter`, take them
    /// into hand, then shuffle the deck. Unlike look-at-top, this searches the entire
    /// deck (§8.2).
    SearchDeckAndTake {
        /// Whose deck to search (resolved to a single deck owner).
        whose: PlayerScope,
        /// How many matching cards the controller may take.
        take_count: u32,
        /// Which cards in the deck are eligible to be taken.
        filter: CharacterFilter,
    },
    /// "Name a card, then reveal the top card of your deck": the controller names
    /// a card; if the revealed top card has that name it goes to `match_to` and the
    /// controller gains `lore_on_match`, otherwise it goes to `otherwise_to` (§8.2;
    /// Merlin / Bruno / The Sorcerer's Hat).
    NameThenReveal {
        /// Lore gained when the revealed card matches the named card.
        lore_on_match: Amount,
        /// Where the revealed card goes on a match (hand / inkwell / …).
        match_to: Destination,
        /// Where it goes otherwise (usually the bottom of the deck).
        otherwise_to: Destination,
    },
    /// "Name a card, then return all character cards with that name from your
    /// discard to your hand" (§8.2; Blast from Your Past).
    NameThenRecur,
    /// Resolve a sequence of effects in order ("draw a card **and** gain 1 lore";
    /// "[A], then [B]", §7.1.2). A later effect resumes after an earlier one's
    /// choice is answered.
    All(Vec<Self>),
    /// Resolve `target` **once**, then apply each of `effects` to the resolved
    /// character in order ("Exert chosen character. They can't ready at the start
    /// of their next turn." = exert then freeze on one chosen target). The
    /// sub-effects act on the resolved target (their own inner target is ignored),
    /// so the player picks a single character (§7.1).
    OnTarget {
        /// The single character the sequence acts on (e.g. a `ChosenCharacter`).
        target: Target,
        /// The effects applied to that character, in order.
        effects: Vec<Self>,
    },
    /// Optionally resolve `inner` ("you may …"): the controller is asked yes/no,
    /// and `inner` resolves only on yes (§7.1.3). Composes optionality onto any
    /// effect, so individual effects don't carry an `optional` flag.
    May(Box<Self>),
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
        /// The board condition: the controller must have matching characters.
        filter: CharacterFilter,
        /// How many matching characters are required ("if you have N or more …").
        /// `1` is plain "if you have a …".
        at_least: u32,
        /// The effect to resolve when the condition holds.
        then: Box<Self>,
    },
    /// Resolve `then` only if a count-based condition holds (hand size, lore,
    /// comparisons, etc.).
    IfCount {
        /// The count condition to check.
        condition: CountCondition,
        /// The effect to resolve when the condition holds.
        then: Box<Self>,
    },
    /// "Choose one: [A] • [B] • [C]" — the controller picks one of the offered
    /// effects to resolve (Anna / Baloo / Baymax's Healthcare Chip, §7.1.9).
    ChooseOne {
        /// The offered effects (2–4 options in practice; 2 is most common).
        options: Vec<Self>,
        /// Whether the choice is optional ("you may choose one" vs mandatory).
        optional: bool,
    },
}

impl Effect {
    /// Substitute the concrete `card` for every [`Target::TriggerCard`] ("the
    /// challenging / challenged character") in this effect, recursing into nested
    /// effects. Called when a challenge-trigger ability that references the other
    /// combatant is enqueued.
    #[must_use]
    pub fn with_trigger_card(self, card: CardId) -> Self {
        let bind = |t: Target| {
            if matches!(t, Target::TriggerCard) {
                Target::Card(card)
            } else {
                t
            }
        };
        match self {
            Self::GiveStatThisTurn {
                target,
                stat,
                amount,
            } => Self::GiveStatThisTurn {
                target: bind(target),
                stat,
                amount,
            },
            Self::DealDamage { target, amount } => Self::DealDamage {
                target: bind(target),
                amount,
            },
            Self::RemoveDamage { target, amount } => Self::RemoveDamage {
                target: bind(target),
                amount,
            },
            Self::Banish(t) => Self::Banish(bind(t)),
            Self::Exert(t) => Self::Exert(bind(t)),
            Self::Ready(t) => Self::Ready(bind(t)),
            Self::Freeze(t) => Self::Freeze(bind(t)),
            Self::Move {
                what: MoveSource::Card(t),
                to,
            } => Self::Move {
                what: MoveSource::Card(bind(t)),
                to,
            },
            Self::GrantThisTurn { target, property } => Self::GrantThisTurn {
                target: bind(target),
                property,
            },
            Self::Grant { target, property } => Self::Grant {
                target: bind(target),
                property,
            },
            Self::GrantNextTurn { target, property } => Self::GrantNextTurn {
                target: bind(target),
                property,
            },
            Self::OnTarget { target, effects } => Self::OnTarget {
                target: bind(target),
                effects: effects
                    .into_iter()
                    .map(|e| e.with_trigger_card(card))
                    .collect(),
            },
            Self::All(seq) => {
                Self::All(seq.into_iter().map(|e| e.with_trigger_card(card)).collect())
            }
            Self::May(inner) => Self::May(Box::new(inner.with_trigger_card(card))),
            other => other,
        }
    }

    /// Substitute the triggering event's value for every [`Amount::TriggerAmount`]
    /// ("that much" / "that many") in this effect, recursing into nested effects.
    /// Called once when a triggered ability that references the trigger's amount is
    /// enqueued, so the bagged effect carries a concrete value.
    #[must_use]
    #[allow(clippy::too_many_lines)] // one variant-dispatch match over a large enum
    pub fn with_trigger_amount(self, value: i32) -> Self {
        let recur = |e: Box<Self>| Box::new(e.with_trigger_amount(value));
        match self {
            // Amount-bearing leaves: substitute the amount.
            Self::Draw { who, amount } => Self::Draw {
                who,
                amount: amount.with_trigger_amount(value),
            },
            Self::Lore { who, amount } => Self::Lore {
                who,
                amount: amount.with_trigger_amount(value),
            },
            Self::GiveStatThisTurn {
                target,
                stat,
                amount,
            } => Self::GiveStatThisTurn {
                target,
                stat,
                amount: amount.with_trigger_amount(value),
            },
            Self::DealDamage { target, amount } => Self::DealDamage {
                target,
                amount: amount.with_trigger_amount(value),
            },
            Self::RemoveDamage { target, amount } => Self::RemoveDamage {
                target,
                amount: amount.with_trigger_amount(value),
            },
            Self::MoveDamage { from, to, amount } => Self::MoveDamage {
                from,
                to,
                amount: amount.with_trigger_amount(value),
            },
            Self::Boost { count } => Self::Boost {
                count: count.with_trigger_amount(value),
            },
            Self::Move {
                what: MoveSource::DeckTop { who, count },
                to,
            } => Self::Move {
                what: MoveSource::DeckTop {
                    who,
                    count: count.with_trigger_amount(value),
                },
                to,
            },
            Self::NameThenReveal {
                lore_on_match,
                match_to,
                otherwise_to,
            } => Self::NameThenReveal {
                lore_on_match: lore_on_match.with_trigger_amount(value),
                match_to,
                otherwise_to,
            },
            // Nested-effect variants: recurse.
            Self::All(seq) => Self::All(
                seq.into_iter()
                    .map(|e| e.with_trigger_amount(value))
                    .collect(),
            ),
            Self::OnTarget { target, effects } => Self::OnTarget {
                target,
                effects: effects
                    .into_iter()
                    .map(|e| e.with_trigger_amount(value))
                    .collect(),
            },
            Self::May(inner) => Self::May(recur(inner)),
            Self::ScheduleDelayed { when, effect } => Self::ScheduleDelayed {
                when,
                effect: recur(effect),
            },
            Self::IfControl {
                filter,
                at_least,
                then,
            } => Self::IfControl {
                filter,
                at_least,
                then: recur(then),
            },
            Self::IfCount { condition, then } => Self::IfCount {
                condition,
                then: recur(then),
            },
            Self::IfTargetMatches {
                target,
                filter,
                then,
                otherwise,
            } => Self::IfTargetMatches {
                target,
                filter,
                then: recur(then),
                otherwise: recur(otherwise),
            },
            Self::GrantAbilityThisTurn {
                target,
                condition,
                effect,
                optional,
            } => Self::GrantAbilityThisTurn {
                target,
                condition,
                effect: recur(effect),
                optional,
            },
            Self::GrantActivatedThisTurn {
                target,
                ink,
                exert_self,
                effect,
            } => Self::GrantActivatedThisTurn {
                target,
                ink,
                exert_self,
                effect: recur(effect),
            },
            Self::ChooseOne { options, optional } => Self::ChooseOne {
                options: options
                    .into_iter()
                    .map(|e| e.with_trigger_amount(value))
                    .collect(),
                optional,
            },
            // No amount and no nested effect: unchanged.
            other => other,
        }
    }
}
