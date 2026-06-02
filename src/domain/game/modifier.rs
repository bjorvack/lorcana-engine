//! Continuous stat modifiers (§7.6, §7.8).
//!
//! A character's current value of a characteristic is its printed base (stored
//! on the [`CardInstance`](super::CardInstance)) **plus** the sum of all active
//! modifiers that apply to it, computed on demand. The sum is taken as a signed
//! value and clamped to 0 only at the point of use (a negative `{S}` deals no
//! damage, a negative `{L}` grants none), while the true value is retained for
//! combining further modifiers (§7.8.1.2/§7.8.2/§7.8.3).

use crate::domain::types::card::Classification;
use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};

/// A modifiable characteristic.
///
/// TODO(locations — Slice 7): locations add further modifiable characteristics —
/// **move cost** (the cost to move a character to the location, §4.3.7), plus
/// location willpower and start-of-turn lore. Add the corresponding `Stat`
/// variants when locations land. See `docs/planning/IMPLEMENTATION_PLAN.md`
/// ("Slice 7").
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
