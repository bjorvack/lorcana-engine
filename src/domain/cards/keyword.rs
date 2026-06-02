//! Keyword abilities (§10).

use serde::{Deserialize, Serialize};

/// A keyword ability printed on a card (§10). The full §10 set is enumerated
/// (keywords are a closed, rules-defined vocabulary); behaviour is wired in
/// incrementally per sub-slice.
///
/// Implemented so far (Slice 6a, challenge cluster): `Rush`, `Evasive`, `Alert`,
/// `Bodyguard` (challenge-targeting restriction), `Resist`, `Challenger`.
///
/// TODO(remaining keywords): `Bodyguard` "may enter play exerted" (a play-time
/// choice); `Singer`/`SingTogether` (songs — Slice 7); `Support`/`Vanish`
/// (triggers/targeting); `Ward` (effect-targeting restriction); `Reckless`
/// (quest/end-turn restrictions); `Boost` (card-under); `Shift` (card stacks).
/// See "Slice 6" in `docs/planning/IMPLEMENTATION_PLAN.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Keyword {
    /// Can only be challenged by characters with Evasive (§10.6).
    Evasive,
    /// May enter play exerted; opponents must challenge a Bodyguard if able
    /// (§10.3).
    Bodyguard,
    /// Can challenge the turn it's played (ignores drying, §10.9).
    Rush,
    /// Ignores Evasive's challenging restriction (§10.2).
    Alert,
    /// Opponents can't choose this card for effects (§10.15).
    Ward,
    /// Can't quest and must challenge if able (§10.7).
    Reckless,
    /// Banished when chosen by an opponent's effect (§10.14).
    Vanish,
    /// On quest, may add this character's `{S}` to another character (§10.13).
    Support,
    /// Damage dealt to this character is reduced by N (§10.8); stacks.
    Resist(u32),
    /// While challenging, this character gets +N `{S}` (§10.5); stacks.
    Challenger(u32),
    /// Pay N to play this on top of a same-named character (§10.10).
    Shift(u32),
    /// May pay a reduced cost N to sing a song (§10.11).
    Singer(u32),
    /// Multiple characters may sing a song with total cost N (§10.12).
    SingTogether(u32),
    /// Once per turn, may pay N `{I}` to put the top deck card under this (§10.4).
    Boost(u32),
}
