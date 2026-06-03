//! Targets that an effect applies to (§7.1).

use super::trigger::CardCategory;
use crate::domain::types::card::Classification;
use crate::domain::types::ids::CardId;
use serde::{Deserialize, Serialize};

/// Which side a character may be on, relative to the effect's controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetSide {
    /// Any character (yours or an opponent's).
    Any,
    /// One of your characters.
    Yours,
    /// An opposing character.
    Opposing,
}

/// How a numeric characteristic is compared against a threshold (§7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Comparison {
    /// "N or less".
    AtMost,
    /// "N or more".
    AtLeast,
    /// Exactly N.
    Exactly,
}

/// A `comparison` against a numeric `value` (e.g. cost 2 or less).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumericFilter {
    /// The comparison.
    pub comparison: Comparison,
    /// The threshold.
    pub value: u32,
}

impl NumericFilter {
    /// "value or less".
    #[must_use]
    pub const fn at_most(value: u32) -> Self {
        Self {
            comparison: Comparison::AtMost,
            value,
        }
    }
    /// "value or more".
    #[must_use]
    pub const fn at_least(value: u32) -> Self {
        Self {
            comparison: Comparison::AtLeast,
            value,
        }
    }
    /// Whether `actual` satisfies this comparison.
    #[must_use]
    pub const fn matches(self, actual: u32) -> bool {
        match self.comparison {
            Comparison::AtMost => actual <= self.value,
            Comparison::AtLeast => actual >= self.value,
            Comparison::Exactly => actual == self.value,
        }
    }
}

/// Which characters an effect may apply to / choose from (§7.1, §6.2.6).
///
/// A small **boolean algebra** of predicates: leaf predicates (side,
/// classification, name, cost, current `{S}`, damaged/exerted, the source card, a
/// specific card) combined with `And` / `Or` / `Not`. This composes — "another
/// Villain you have in play with cost 3 or less" is just
/// `And([Side(Yours), Classification(Villain), Cost(≤3), Not(IsSource)])` — and
/// the same exclusion predicates express "another …" and "not the already-chosen
/// card" uniformly. Cost is the printed cost; strength is the current `{S}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharacterFilter {
    /// Matches any character.
    Any,
    /// On the given side relative to the effect's controller.
    Side(TargetSide),
    /// Has the given classification.
    Classification(Classification),
    /// Is of the given card category (character / action / song / item /
    /// location). Lets one filter vocabulary cover hand/deck cards too (§6).
    Category(CardCategory),
    /// Counts as the given name ("named X").
    Named(String),
    /// Printed ink cost satisfies the numeric filter.
    Cost(NumericFilter),
    /// Current `{S}` satisfies the numeric filter.
    Strength(NumericFilter),
    /// Damaged (`true`) or undamaged (`false`).
    Damaged(bool),
    /// Exerted (`true`) or ready (`false`).
    Exerted(bool),
    /// The effect's source card itself.
    IsSource,
    /// A specific card (e.g. an already-chosen target).
    IsCard(CardId),
    /// All sub-filters match (empty ⇒ matches anything).
    And(Vec<Self>),
    /// At least one sub-filter matches (empty ⇒ matches nothing).
    Or(Vec<Self>),
    /// The sub-filter does not match.
    Not(Box<Self>),
}

impl CharacterFilter {
    /// A filter matching any character on `side`.
    #[must_use]
    pub const fn any(side: TargetSide) -> Self {
        Self::Side(side)
    }

    /// Combine with another predicate via AND (flattening nested `And`s).
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        match self {
            Self::And(mut fs) => {
                fs.push(other);
                Self::And(fs)
            }
            first => Self::And(vec![first, other]),
        }
    }

    /// Negate a filter (`Not`), boxing for you.
    #[must_use]
    pub fn negate(inner: Self) -> Self {
        Self::Not(Box::new(inner))
    }
}

/// What an effect applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// The effect's source card itself ("this card/character").
    SelfCard,
    /// A specific, already-resolved card — the outcome of resolving a chosen
    /// target (e.g. the first pick of a two-target move-damage). Not authored on
    /// cards directly.
    Card(CardId),
    /// A single character the controller chooses at resolution (§7.1). "Another
    /// chosen …" is expressed by the `filter` (`Not(IsSource)`).
    ChosenCharacter {
        /// Which characters are eligible to be chosen.
        filter: CharacterFilter,
    },
    /// **Every** character matching the filter, with no choice ("your Pirate
    /// characters", "all opposing characters"). "Your *other* …" is expressed by
    /// the `filter` (`Not(IsSource)`).
    AllCharacters {
        /// Which characters are affected.
        filter: CharacterFilter,
    },
    /// **Up to `max`** distinct chosen characters matching the filter (§7.1.8);
    /// the effect applies to each. 0 is a legal choice ("Up to 2 chosen
    /// characters get -1 {S}").
    UpToCharacters {
        /// Which characters are eligible.
        filter: CharacterFilter,
        /// The maximum number that may be chosen.
        max: u32,
    },
    /// A single in-play **permanent** (character / item / location) the controller
    /// chooses, described by the filter algebra — e.g. "chosen item" is
    /// `Category(Item)`, "chosen opposing location" is
    /// `And([Side(Opposing), Category(Location)])` (§6.4, §6.5).
    ChosenPermanent {
        /// Which permanents are eligible to be chosen.
        filter: CharacterFilter,
    },
}
