//! Continuous stat modifiers (§7.6, §7.8).
//!
//! A character's current value of a characteristic is its printed base (stored
//! on the [`CardInstance`](super::CardInstance)) **plus** the sum of all active
//! modifiers that apply to it, computed on demand. The sum is taken as a signed
//! value and clamped to 0 only at the point of use (a negative `{S}` deals no
//! damage, a negative `{L}` grants none), while the true value is retained for
//! combining further modifiers (§7.8.1.2/§7.8.2/§7.8.3).

use crate::domain::cards::Keyword;
use crate::domain::types::card::Classification;
use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};

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

/// How long a modifier lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierDuration {
    /// Active for as long as the source card is in play (§7.6.4).
    WhileSourceInPlay,
    /// Active until the end of the current turn; expires at cleanup.
    UntilEndOfTurn,
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

/// A continuous **prevention** an effect/keyword places on a card ("can't …").
/// Preventions beat permissions (§1.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Restriction {
    /// The character can't quest ("can't quest", granted Reckless).
    CantQuest,
    /// The character can't challenge.
    CantChallenge,
    /// The character/location can't be challenged ("can't be challenged while here").
    CantBeChallenged,
}

/// A continuous **permission** an effect grants a card ("can …"). Kept distinct
/// from [`Restriction`] so the two never get conflated (and §1.2.2: a prevention
/// still beats a permission).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    /// The character may challenge **ready** characters, not just exerted ones
    /// (Pick a Fight).
    ChallengeReady,
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
}

impl PropertyModifier {
    /// Create a property modifier.
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
        }
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
}

impl StatModifier {
    /// Create a stat modifier.
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
        }
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
