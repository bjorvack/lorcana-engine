//! Keyword abilities (§10).

use crate::domain::types::card::Classification;
use serde::{Deserialize, Serialize};

/// Which characters a Shift card may be played on top of (§10.10, §10.10.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShiftKind {
    /// Standard Shift: a character with a **same name** (§10.10.1).
    SameName,
    /// [Classification] Shift: one of your characters with the classification
    /// (§10.10.9.1), e.g. Puppy Shift.
    Classification(Classification),
    /// Universal Shift: any of your characters (§10.10.9.2).
    Any,
}

/// The alternate cost paid to Shift (§10.10.1).
///
/// TODO(alternate Shift costs — Slice 8, see "Slice 6c"/"Slice 8" in
/// `docs/planning/IMPLEMENTATION_PLAN.md`): add the non-ink shift costs found in
/// the pool — `Discard { count, card_type }` (Flotsam & Jetsam "Shift: Discard 2
/// cards"; the per-type "Shift: Discard a song/item/… card"), and free-from-
/// discard ("Shift a character from your discard for free"). Also cost reducers
/// (Yokai "pay 1 {I} less to play characters using their Shift") plug into how
/// this cost is paid. These need the alternate-cost / effect machinery of Slice 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShiftCost {
    /// Pay N ink (the standard `Shift N` / `Shift N {I}` form).
    Ink(u32),
}

/// A Shift ability: an alternate way to play a character on top of a valid
/// target (§10.10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftAbility {
    /// The cost to shift.
    pub cost: ShiftCost,
    /// Which characters this may be shifted onto.
    pub kind: ShiftKind,
}

impl ShiftAbility {
    /// A standard same-name Shift for `ink`.
    #[must_use]
    pub const fn ink_same_name(ink: u32) -> Self {
        Self {
            cost: ShiftCost::Ink(ink),
            kind: ShiftKind::SameName,
        }
    }
}

/// A keyword ability printed on a card (§10). The full §10 set is enumerated
/// (keywords are a closed, rules-defined vocabulary); behaviour is wired in
/// incrementally per sub-slice.
///
/// Implemented so far: `Rush`, `Evasive`, `Alert`, `Bodyguard` (challenge
/// restriction), `Resist`, `Challenger` (Slice 6a); `Reckless` (Slice 6b);
/// `Shift` — standard same-name + Universal + [Classification] (Slice 6c).
///
/// TODO(remaining keywords): `Bodyguard` "may enter play exerted" (a play-time
/// choice); `Shift` alternate costs / cost reducers / granted-name + Morph
/// targeting / shift-conditional triggers (Slice 8 — see `ShiftCost` and the
/// reducer TODOs); `Singer`/`SingTogether` (songs — Slice 7); `Support`/`Vanish`
/// (triggers/targeting); `Ward` (effect-targeting restriction); `Boost`
/// (card-under, shares the stack model). See "Slice 6" in
/// `docs/planning/IMPLEMENTATION_PLAN.md`.
//
// Not `Copy`: `Shift` carries a `ShiftAbility` (which can hold a `Classification`
// string). Keyword checks take it by value/ref as needed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Play this on top of a valid target by paying an alternate cost (§10.10).
    Shift(ShiftAbility),
    /// May pay a reduced cost N to sing a song (§10.11).
    Singer(u32),
    /// Multiple characters may sing a song with total cost N (§10.12).
    SingTogether(u32),
    /// Once per turn, may pay N `{I}` to put the top deck card under this (§10.4).
    Boost(u32),
}
