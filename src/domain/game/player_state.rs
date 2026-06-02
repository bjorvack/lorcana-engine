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

    /// This player's inkwell.
    #[must_use]
    pub const fn inkwell(&self) -> &Zone {
        &self.inkwell
    }

    /// This player's play area.
    #[must_use]
    pub const fn play(&self) -> &Zone {
        &self.play
    }

    /// This player's discard.
    #[must_use]
    pub const fn discard(&self) -> &Zone {
        &self.discard
    }
}
