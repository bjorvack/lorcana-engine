//! Per-player state: lore and the player's zones.

use super::Zone;
use crate::domain::types::ids::PlayerId;
use serde::{Deserialize, Serialize};

/// The state owned by a single player: their lore total and their five
/// card-holding zones (§8). The bag is shared by the whole game and so is not
/// stored here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    id: PlayerId,
    lore: u32,
    deck: Zone,
    hand: Zone,
    inkwell: Zone,
    play: Zone,
    discard: Zone,
    /// `true` once this player has lost or left the game. Eliminated players are
    /// skipped by the win/loss check and trigger last-player-standing (§3.2.1.3).
    eliminated: bool,
    /// Set when this player attempts to draw from an empty deck; read by the
    /// game-state check (§1.9.1.2, "since the last game state check") and cleared
    /// by the check afterwards.
    drew_from_empty_deck: bool,
}

impl PlayerState {
    /// Create a player with the given deck and empty remaining zones.
    #[must_use]
    pub const fn new(id: PlayerId, deck: Zone) -> Self {
        Self {
            id,
            lore: 0,
            deck,
            hand: Zone::new(),
            inkwell: Zone::new(),
            play: Zone::new(),
            discard: Zone::new(),
            eliminated: false,
            drew_from_empty_deck: false,
        }
    }

    /// This player's id.
    #[must_use]
    pub const fn id(&self) -> PlayerId {
        self.id
    }

    /// This player's current lore total.
    #[must_use]
    pub const fn lore(&self) -> u32 {
        self.lore
    }

    /// Add lore to this player's total (e.g. from questing or locations).
    pub const fn add_lore(&mut self, amount: u32) {
        self.lore += amount;
    }

    /// Remove lore from this player's total, clamped at 0.
    pub const fn lose_lore(&mut self, amount: u32) {
        self.lore = self.lore.saturating_sub(amount);
    }

    /// The number of ready ink cards available to pay costs (§8.5, §4.3.4.6).
    #[must_use]
    pub fn ready_ink(&self) -> u32 {
        let count = self.inkwell.iter().filter(|c| c.conditions().ready).count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// Exert `amount` ready ink cards to pay a cost (§4.3.4.6). Ink is fungible
    /// (§8.5.1), so the specific cards chosen don't matter; the first ready ones
    /// are used. The caller must have verified enough ready ink via
    /// [`ready_ink`](Self::ready_ink).
    pub fn exert_ink(&mut self, amount: u32) {
        let mut remaining = amount;
        for card in self.inkwell.iter_mut() {
            if remaining == 0 {
                break;
            }
            if card.conditions().ready {
                card.conditions_mut().ready = false;
                remaining -= 1;
            }
        }
    }

    /// `true` if this player has lost or left the game.
    #[must_use]
    pub const fn is_eliminated(&self) -> bool {
        self.eliminated
    }

    /// Mark this player as having lost or left the game.
    pub const fn eliminate(&mut self) {
        self.eliminated = true;
    }

    /// `true` if this player attempted to draw from an empty deck since the last
    /// game-state check.
    #[must_use]
    pub const fn drew_from_empty_deck(&self) -> bool {
        self.drew_from_empty_deck
    }

    /// Record that this player attempted to draw from an empty deck.
    pub const fn note_drew_from_empty_deck(&mut self) {
        self.drew_from_empty_deck = true;
    }

    /// Clear the empty-deck-draw flag (done by the game-state check).
    pub const fn clear_drew_from_empty_deck(&mut self) {
        self.drew_from_empty_deck = false;
    }

    /// This player's deck.
    #[must_use]
    pub const fn deck(&self) -> &Zone {
        &self.deck
    }

    /// Mutable access to this player's deck.
    pub const fn deck_mut(&mut self) -> &mut Zone {
        &mut self.deck
    }

    /// This player's hand.
    #[must_use]
    pub const fn hand(&self) -> &Zone {
        &self.hand
    }

    /// Mutable access to this player's hand.
    pub const fn hand_mut(&mut self) -> &mut Zone {
        &mut self.hand
    }

    /// This player's inkwell.
    #[must_use]
    pub const fn inkwell(&self) -> &Zone {
        &self.inkwell
    }

    /// Mutable access to this player's inkwell.
    pub const fn inkwell_mut(&mut self) -> &mut Zone {
        &mut self.inkwell
    }

    /// This player's play area.
    #[must_use]
    pub const fn play(&self) -> &Zone {
        &self.play
    }

    /// Mutable access to this player's play area.
    pub const fn play_mut(&mut self) -> &mut Zone {
        &mut self.play
    }

    /// This player's discard.
    #[must_use]
    pub const fn discard(&self) -> &Zone {
        &self.discard
    }

    /// Mutable access to this player's discard.
    pub const fn discard_mut(&mut self) -> &mut Zone {
        &mut self.discard
    }
}
