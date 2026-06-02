//! Targets that an effect applies to (§7.1).

use crate::domain::types::card::Classification;
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
/// A character matches when it is on the allowed `side`, has every classification
/// in `classifications`, and satisfies any set numeric / state filters. Cost is
/// the printed cost (cost-modifier interaction on in-play characters is deferred);
/// strength is the **current** `{S}` (modifiers count).
///
/// TODO(filter dimensions — grow as cards need them): name ("named X", 327);
/// and beyond characters, **item** / **location** targets (49 / 59), **player**
/// targets (12), and an "exclude self" flag for **group** targets ("your *other*
/// characters", 61). See "Slice 8" in `docs/planning/IMPLEMENTATION_PLAN.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterFilter {
    /// Which side the character may be on.
    pub side: TargetSide,
    /// Required classifications (all must be present); empty means no filter.
    pub classifications: Vec<Classification>,
    /// Constrain the (printed) ink cost.
    pub cost: Option<NumericFilter>,
    /// Constrain the current `{S}`.
    pub strength: Option<NumericFilter>,
    /// If set, require the character to be damaged (`true`) / undamaged (`false`).
    pub damaged: Option<bool>,
    /// If set, require the character to be exerted (`true`) / ready (`false`).
    pub exerted: Option<bool>,
}

impl CharacterFilter {
    /// A filter matching any character on `side` with no other constraints.
    #[must_use]
    pub const fn any(side: TargetSide) -> Self {
        Self {
            side,
            classifications: Vec::new(),
            cost: None,
            strength: None,
            damaged: None,
            exerted: None,
        }
    }
}

/// What an effect applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// The effect's source card itself ("this card/character").
    SelfCard,
    /// A single character the controller chooses at resolution (§7.1).
    ChosenCharacter {
        /// Which characters are eligible to be chosen.
        filter: CharacterFilter,
        /// If `true`, the source card itself can't be chosen ("another chosen…").
        another: bool,
    },
    /// **Every** character matching the filter, with no choice ("your Pirate
    /// characters", "all opposing characters").
    AllCharacters(CharacterFilter),
    /// A single in-play item the controller chooses ("banish chosen item", §6.4).
    ChosenItem {
        /// Which side the item may be on.
        side: TargetSide,
    },
    /// A single in-play location the controller chooses ("chosen location", §6.5).
    ChosenLocation {
        /// Which side the location may be on.
        side: TargetSide,
    },
}
