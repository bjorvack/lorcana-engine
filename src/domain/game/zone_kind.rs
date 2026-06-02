//! The kinds of zones a card can occupy.

use serde::{Deserialize, Serialize};

/// The zones defined by the comprehensive rules (§8).
///
/// Note: banished cards go to the discard (§8.6.2) — there is no separate
/// "banished" zone. The bag (§8.7) holds triggered abilities, not cards, and is
/// shared by the whole game rather than owned per player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ZoneKind {
    /// A player's deck (private, facedown, ordered).
    Deck,
    /// A player's hand (private to its owner).
    Hand,
    /// A player's inkwell (private, facedown; each card is 1 ink).
    Inkwell,
    /// A player's play area (public).
    Play,
    /// A player's discard (public).
    Discard,
}
