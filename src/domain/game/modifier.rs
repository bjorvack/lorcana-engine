//! Continuous stat modifiers (§7.6, §7.8).
//!
//! A character's current value of a characteristic is its printed base (stored
//! on the [`CardInstance`](super::CardInstance)) **plus** the sum of all active
//! modifiers that apply to it, computed on demand. The sum is taken as a signed
//! value and clamped to 0 only at the point of use (a negative `{S}` deals no
//! damage, a negative `{L}` grants none), while the true value is retained for
//! combining further modifiers (§7.8.1.2/§7.8.2/§7.8.3).

use crate::domain::cards::Keyword;
use crate::domain::effects::{Amount, CharacterFilter, Effect, TriggerCondition};
use crate::domain::types::card::Classification;
use crate::domain::types::ids::{CardId, PlayerId};
use crate::domain::types::turn::Step;
use serde::{Deserialize, Serialize};

/// An **activated** ability granted to a card by an effect.
///
/// "Gains '{E} — Draw a card' this turn" (§7.5): usable like a printed activated
/// ability for as long as `duration` holds. Stored as primitives (cost + effect)
/// to stay decoupled from the card-definition types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantedActivated {
    /// The card that has the granted ability.
    pub source: CardId,
    /// Ink cost to activate.
    pub ink: u32,
    /// Whether activating exerts the card.
    pub exert_self: bool,
    /// What it does.
    pub effect: Effect,
    /// How long the grant lasts.
    pub duration: ModifierDuration,
}

/// A triggered ability granted to a card by an effect.
///
/// "Gains 'Whenever this character challenges, …' this turn" (§7.6): fires
/// alongside the card's printed triggered abilities for as long as `duration`
/// holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantedTrigger {
    /// The card that has the granted ability (it fires from this card).
    pub source: CardId,
    /// When it fires.
    pub condition: TriggerCondition,
    /// What it does. ("You may …" optionality is part of the effect via
    /// [`Effect::May`].)
    pub effect: Effect,
    /// How long the grant lasts.
    pub duration: ModifierDuration,
}

/// A modifiable characteristic.
///
/// TODO(modifiable location stats — Slice 8b+): locations are in play (Slice 7b)
/// but their characteristics aren't yet modifiable — add `Stat` variants for
/// **move cost** (§4.3.7), location willpower, and start-of-turn lore when a card
/// needs to modify them. See `docs/planning/IMPLEMENTATION_PLAN.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    /// Strength `{S}`.
    Strength,
    /// Willpower `{W}`.
    Willpower,
    /// Lore `{L}`.
    Lore,
}

/// A condition that gates a continuous modifier — it applies only while the
/// condition holds, evaluated on demand (§7.6 "while …" static abilities).
///
/// Grows as cards need it (stat thresholds, "while at a location", "while you
/// have a … in play", …). The first cut is registry-free (state-only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Condition {
    /// While the modifier's source card is exerted ("while this character is
    /// exerted, …").
    SourceExerted,
}

/// How long a modifier lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierDuration {
    /// Active for as long as the source card is in play (§7.6.4).
    WhileSourceInPlay,
    /// A permanent grant: never expires on a turn/step boundary. Removed only when
    /// the affected card leaves play (model it with `source` = the affected card,
    /// so the leave-play modifier sweep clears it). E.g. "gains Evasive".
    Permanent,
    /// Active until the end of the current turn; expires at cleanup.
    UntilEndOfTurn,
    /// Active until `player` next reaches `step` (consumed when that player
    /// completes that step). Survives the end of the turn it was created in.
    /// Generalizes "until the start of their next turn" timings — e.g. one-shot
    /// freeze is `UntilStep { step: Ready, player: <the frozen card's owner> }`.
    UntilStep {
        /// The step at which this expires.
        step: Step,
        /// Whose turn's `step` consumes it.
        player: PlayerId,
    },
}

/// Which cards a modifier applies to. Matching against a card is done by
/// [`GameState`](super::GameState), which knows each in-play card's owner and
/// classifications (denormalized onto the instance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierTarget {
    /// Applies to exactly one card (e.g. a self modifier).
    Card(CardId),
    /// Applies to all of `owner`'s in-play characters that have any of
    /// `classifications` (empty ⇒ all of the owner's characters), optionally
    /// excluding one card (for "your **other** characters"). Models selector
    /// statics like "your Villain characters get +1 {S}" (§7.8 Example A).
    OwnedCharacters {
        /// The player whose characters are affected.
        owner: PlayerId,
        /// Required classifications (any-of); empty matches every character.
        classifications: Vec<Classification>,
        /// A card to exclude (the source, for "your other characters").
        except: Option<CardId>,
    },
}

/// A continuous modifier to a **game rule** contributed by a static ability in
/// play (the win/loss modification layer, §1.2.1). Removed when its source
/// leaves play.
///
/// TODO(modification layer — Slice 5g+): only the lore-to-win override exists so
/// far (Donald Duck). The fuller add / remove-suppress space ("you can't lose",
/// "opponents can't win", added alternate win conditions) is enumerated in the
/// `win_loss.rs` test TODO and lands as more cards need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleModifier {
    /// `player` must reach `threshold` lore to win instead of the base 20
    /// (§1.9.1.1) — e.g. Donald Duck – Flustered Sorcerer.
    LoreToWin {
        /// The card whose ability generates this modifier.
        source: CardId,
        /// The affected player.
        player: PlayerId,
        /// The lore threshold this player needs.
        threshold: u32,
    },
}

impl RuleModifier {
    /// The card whose ability generates this modifier.
    #[must_use]
    pub const fn source(self) -> CardId {
        match self {
            Self::LoreToWin { source, .. } => source,
        }
    }
}

/// A continuous **prevention** an effect/keyword places on a card ("can't …" /
/// "takes no …"). Preventions beat permissions (§1.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Restriction {
    /// The character can't quest ("can't quest", granted Reckless).
    CantQuest,
    /// The character can't challenge.
    CantChallenge,
    /// The character/location can't be challenged ("can't be challenged while here").
    CantBeChallenged,
    /// The card can't be **chosen** by an opponent's abilities/effects (Ward,
    /// §10.15; or an effect-granted "can't be chosen this turn"). Challenges are
    /// unaffected.
    CantBeChosen,
    /// The card can't **ready** at its controller's ready step ("can't ready at the
    /// start of their next turn" — freeze, one-shot via
    /// [`ModifierDuration::UntilNextReadyStep`]; or a continuous "can't ready").
    CantReady,
    /// The character takes no damage from challenges (a §7.7 damage replacement —
    /// "takes no damage from challenges this turn", Noi / Nothing We Won't Do).
    TakesNoChallengeDamage,
}

/// A continuous **permission** an effect grants a card ("may …").
///
/// Kept distinct from [`Restriction`] so the two never get conflated (and §1.2.2:
/// a prevention still beats a permission). Some overlap the Alert/Rush keywords;
/// the legality checks OR the two together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    /// May challenge **ready** (non-exerted) characters (Pick a Fight, §4.3.6.7).
    ChallengeReady,
    /// May challenge **Evasive** characters (like Alert, §10.2/§10.6).
    ChallengeEvasive,
    /// May **challenge** the turn it entered play, while still drying (like Rush,
    /// §10.9).
    ChallengeWhileDrying,
    /// May **quest** the turn it entered play, while still drying.
    QuestWhileDrying,
}

/// A continuous boolean property an effect/ability grants to one or more in-play
/// cards: a granted keyword (§10), a [`Restriction`], or a [`Permission`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Property {
    /// A granted keyword (e.g. `Challenger(2)`, `Evasive`).
    Keyword(Keyword),
    /// A granted prevention.
    Restriction(Restriction),
    /// A granted permission.
    Permission(Permission),
}

/// A continuous [`Property`] applied to one or more in-play cards, mirroring
/// [`StatModifier`]. Removed when its source leaves play / at end of turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyModifier {
    source: CardId,
    target: ModifierTarget,
    property: Property,
    duration: ModifierDuration,
    condition: Option<Condition>,
}

impl PropertyModifier {
    /// Create an (unconditional) property modifier.
    #[must_use]
    pub const fn new(
        source: CardId,
        target: ModifierTarget,
        property: Property,
        duration: ModifierDuration,
    ) -> Self {
        Self {
            source,
            target,
            property,
            duration,
            condition: None,
        }
    }

    /// Gate this modifier on a [`Condition`] (builder).
    #[must_use]
    pub const fn with_condition(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// The condition gating this modifier, if any.
    #[must_use]
    pub const fn condition(&self) -> Option<Condition> {
        self.condition
    }

    /// The card whose ability generates this modifier.
    #[must_use]
    pub const fn source(&self) -> CardId {
        self.source
    }

    /// The target this modifier applies to.
    #[must_use]
    pub const fn target(&self) -> &ModifierTarget {
        &self.target
    }

    /// The granted property.
    #[must_use]
    pub const fn property(&self) -> &Property {
        &self.property
    }

    /// The duration.
    #[must_use]
    pub const fn duration(&self) -> ModifierDuration {
        self.duration
    }
}

/// A continuous modifier to a characteristic of one or more in-play cards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatModifier {
    source: CardId,
    target: ModifierTarget,
    stat: Stat,
    delta: i32,
    duration: ModifierDuration,
    condition: Option<Condition>,
    /// If set, the effective delta is `delta × count` (a dynamic "+N for each …"),
    /// evaluated live; `None` means a flat `delta`.
    per: Option<Amount>,
}

impl StatModifier {
    /// Create an (unconditional) stat modifier.
    #[must_use]
    pub const fn new(
        source: CardId,
        target: ModifierTarget,
        stat: Stat,
        delta: i32,
        duration: ModifierDuration,
    ) -> Self {
        Self {
            source,
            target,
            stat,
            delta,
            duration,
            condition: None,
            per: None,
        }
    }

    /// Gate this modifier on a [`Condition`] (builder).
    #[must_use]
    pub const fn with_condition(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Make the effective delta scale by a live [`Amount`] (builder).
    #[must_use]
    pub fn with_count(mut self, per: Amount) -> Self {
        self.per = Some(per);
        self
    }

    /// The dynamic count this modifier scales by, if any.
    #[must_use]
    pub const fn per(&self) -> Option<&Amount> {
        self.per.as_ref()
    }

    /// The condition gating this modifier, if any.
    #[must_use]
    pub const fn condition(&self) -> Option<Condition> {
        self.condition
    }

    /// The card whose ability generates this modifier.
    #[must_use]
    pub const fn source(&self) -> CardId {
        self.source
    }

    /// The target this modifier applies to.
    #[must_use]
    pub const fn target(&self) -> &ModifierTarget {
        &self.target
    }

    /// The characteristic this modifier affects.
    #[must_use]
    pub const fn stat(&self) -> Stat {
        self.stat
    }

    /// The signed delta.
    #[must_use]
    pub const fn delta(&self) -> i32 {
        self.delta
    }

    /// The duration.
    #[must_use]
    pub const fn duration(&self) -> ModifierDuration {
        self.duration
    }
}

/// A continuous reduction to the ink cost of playing matching cards.
///
/// Applies to cards matching `filter` (matched against the printed definition)
/// for player `owner` — "you pay N {I} less to play [classification] characters"
/// (§6, cost modification). Applied at the point of play; the effective cost
/// floors at 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CostModifier {
    source: CardId,
    owner: PlayerId,
    filter: CharacterFilter,
    amount: u32,
    duration: ModifierDuration,
    condition: Option<Condition>,
}

impl CostModifier {
    /// Create a cost reduction of `amount` for `owner`'s plays matching `filter`.
    #[must_use]
    pub const fn new(
        source: CardId,
        owner: PlayerId,
        filter: CharacterFilter,
        amount: u32,
        duration: ModifierDuration,
    ) -> Self {
        Self {
            source,
            owner,
            filter,
            amount,
            duration,
            condition: None,
        }
    }

    /// Gate this modifier on a [`Condition`] (builder).
    #[must_use]
    pub const fn with_condition(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// The card whose ability generates this modifier.
    #[must_use]
    pub const fn source(&self) -> CardId {
        self.source
    }

    /// The player whose plays are discounted.
    #[must_use]
    pub const fn owner(&self) -> PlayerId {
        self.owner
    }

    /// Which cards-to-play this reduction applies to (matched against the def).
    #[must_use]
    pub const fn filter(&self) -> &CharacterFilter {
        &self.filter
    }

    /// The ink reduction.
    #[must_use]
    pub const fn amount(&self) -> u32 {
        self.amount
    }

    /// The condition gating this modifier, if any.
    #[must_use]
    pub const fn condition(&self) -> Option<Condition> {
        self.condition
    }

    /// The duration.
    #[must_use]
    pub const fn duration(&self) -> ModifierDuration {
        self.duration
    }
}
