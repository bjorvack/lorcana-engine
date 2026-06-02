//! Per-instance card conditions.
//!
//! Conditions are the mutable states a card carries while in a zone, as
//! described in the comprehensive rules (§5). Only the conditions needed by the
//! current slice are modeled; stack membership (under/on-top, introduced by
//! Shift) is deferred until the keyword that needs it.

use serde::{Deserialize, Serialize};

/// The conditions currently applied to a card instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    /// `true` if ready, `false` if exerted (turned sideways).
    pub ready: bool,
    /// Accumulated damage counters (§9). Persistent until removed or banished.
    pub damage: u32,
    /// `true` while the card is "drying" (summoning sick) and so cannot quest,
    /// be declared as a challenger, or exert to pay a cost (§5.1.11).
    pub drying: bool,
    /// `true` if facedown (e.g. in the deck or inkwell), `false` if faceup.
    pub facedown: bool,
}

impl Conditions {
    /// Conditions for a facedown card sitting in a deck (§5.1.13.5).
    #[must_use]
    pub const fn in_deck() -> Self {
        Self {
            ready: true,
            damage: 0,
            drying: false,
            facedown: true,
        }
    }

    /// Conditions for a card placed in the inkwell: facedown and ready
    /// (§8.5.2). Identical in shape to a deck card today, but named for intent.
    #[must_use]
    pub const fn in_inkwell() -> Self {
        Self {
            ready: true,
            damage: 0,
            drying: false,
            facedown: true,
        }
    }

    /// Default conditions for a card in a faceup public pile (discard / hand):
    /// faceup and undamaged. Used when a card (or a dissolved stack) moves there.
    #[must_use]
    pub const fn faceup_idle() -> Self {
        Self {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        }
    }

    /// Conditions for a character as it enters play: ready, undamaged, drying,
    /// and faceup (§5.1.13.1).
    #[must_use]
    pub const fn entering_play() -> Self {
        Self {
            ready: true,
            damage: 0,
            drying: true,
            facedown: false,
        }
    }
}
