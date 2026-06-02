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

/// Which characters an effect may apply to / choose from (§7.1, §6.2.6).
///
/// A character matches when it is on the allowed `side` and has every
/// classification in `classifications`.
///
/// TODO(filter dimensions — grow as cards need them; counts from the set):
/// `cost`/`{S}` comparisons ("with cost N or less", 185; "with N {S} or less",
/// 43); state filters ("damaged", 63; "exerted", 28); name ("named X", 327).
/// And beyond characters: **item** / **location** targets (49 / 59), **player**
/// targets (12), and an "exclude self" flag for **group** targets ("your *other*
/// characters", 61). See "Slice 8" in `docs/planning/IMPLEMENTATION_PLAN.md`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterFilter {
    /// Which side the character may be on.
    pub side: TargetSide,
    /// Required classifications (all must be present); empty means no filter.
    pub classifications: Vec<Classification>,
}

impl CharacterFilter {
    /// A filter matching any character on `side` with no classification filter.
    #[must_use]
    pub const fn any(side: TargetSide) -> Self {
        Self {
            side,
            classifications: Vec::new(),
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
}
