//! Targets that an effect applies to (§7.1).

use serde::{Deserialize, Serialize};

/// What an effect applies to.
///
/// Grows incrementally: Slice 8a starts with `SelfCard`; `ChosenCharacter`
/// (with side / "another" / classification / cost filters) and `EachOpponent`
/// etc. are added as the targeting machinery lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    /// The effect's source card itself ("this card/character").
    SelfCard,
}
