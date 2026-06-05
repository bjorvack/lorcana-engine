//! The bag: triggered abilities waiting to resolve (§8.7).

use crate::domain::effects::{DelayedWhen, Effect};
use crate::domain::types::ids::{CardId, PlayerId};
use serde::{Deserialize, Serialize};

/// A one-shot **delayed** (floating) triggered ability scheduled by an effect,
/// resolved at a later time (§7.4.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelayedTrigger {
    controller: PlayerId,
    source: CardId,
    when: DelayedWhen,
    effect: Effect,
}

impl DelayedTrigger {
    /// Create a delayed trigger.
    #[must_use]
    pub const fn new(
        controller: PlayerId,
        source: CardId,
        when: DelayedWhen,
        effect: Effect,
    ) -> Self {
        Self {
            controller,
            source,
            when,
            effect,
        }
    }

    /// The player who controls (resolves) it.
    #[must_use]
    pub const fn controller(&self) -> PlayerId {
        self.controller
    }

    /// The card that scheduled it.
    #[must_use]
    pub const fn source(&self) -> CardId {
        self.source
    }

    /// When it fires.
    #[must_use]
    pub const fn when(&self) -> DelayedWhen {
        self.when
    }

    /// The effect to resolve when it fires.
    #[must_use]
    pub fn effect(&self) -> Effect {
        self.effect.clone()
    }
}

/// A stable, deterministic id for a bag entry (allocated sequentially).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TriggerId(u32);

impl TriggerId {
    /// Create a `TriggerId` from a raw value.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// The underlying raw value.
    #[must_use]
    pub const fn as_raw(self) -> u32 {
        self.0
    }
}

/// A triggered ability instance waiting in the bag to resolve (§8.7).
// Not `Copy` on purpose: `Effect` will gain non-`Copy` variants when the effect
// DSL lands (Slice 8), so deriving `Copy` now would be churn to undo later.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BagEntry {
    id: TriggerId,
    controller: PlayerId,
    source: CardId,
    effect: Effect,
}

impl BagEntry {
    /// Create a bag entry.
    #[must_use]
    pub const fn new(id: TriggerId, controller: PlayerId, source: CardId, effect: Effect) -> Self {
        Self {
            id,
            controller,
            source,
            effect,
        }
    }

    /// This entry's id.
    #[must_use]
    pub const fn id(&self) -> TriggerId {
        self.id
    }

    /// The player who controls (and resolves) this triggered ability.
    #[must_use]
    pub const fn controller(&self) -> PlayerId {
        self.controller
    }

    /// The card whose ability this is.
    #[must_use]
    pub const fn source(&self) -> CardId {
        self.source
    }

    /// The effect to apply when this entry resolves. ("You may …" optionality is
    /// part of the effect itself via [`Effect::May`].)
    #[must_use]
    pub fn effect(&self) -> Effect {
        self.effect.clone()
    }
}
