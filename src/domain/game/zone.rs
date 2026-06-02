//! An ordered collection of card instances in a single zone.

use super::{CardInstance, SeededRng};
use crate::domain::types::ids::CardId;
use serde::{Deserialize, Serialize};

/// An ordered pile of [`CardInstance`]s. Order is significant (e.g. the deck is
/// drawn from the top), so the backing storage is a `Vec`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    cards: Vec<CardInstance>,
}

impl Zone {
    /// Create an empty zone.
    #[must_use]
    pub const fn new() -> Self {
        Self { cards: Vec::new() }
    }

    /// Create a zone from an ordered list of instances.
    #[must_use]
    pub const fn from_cards(cards: Vec<CardInstance>) -> Self {
        Self { cards }
    }

    /// Number of cards in the zone.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.cards.len()
    }

    /// `true` if the zone has no cards.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Iterate over the cards in order.
    pub fn iter(&self) -> impl Iterator<Item = &CardInstance> {
        self.cards.iter()
    }

    /// Mutably iterate over the cards in order (e.g. to ready them).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut CardInstance> {
        self.cards.iter_mut()
    }

    /// `true` if a card with the given id is in this zone.
    #[must_use]
    pub fn contains(&self, id: CardId) -> bool {
        self.cards.iter().any(|c| c.id() == id)
    }

    /// Add a card to the top of the zone.
    pub fn push(&mut self, card: CardInstance) {
        self.cards.push(card);
    }

    /// Remove and return the top card, if any.
    pub fn pop_top(&mut self) -> Option<CardInstance> {
        self.cards.pop()
    }

    /// Add a card to the bottom of the zone (e.g. mulligan put-back, §3.1.6.1).
    pub fn insert_bottom(&mut self, card: CardInstance) {
        self.cards.insert(0, card);
    }

    /// Remove and return the card with the given id, if present.
    pub fn take(&mut self, id: CardId) -> Option<CardInstance> {
        let index = self.cards.iter().position(|c| c.id() == id)?;
        Some(self.cards.remove(index))
    }

    /// Shuffle the zone in place using the game's deterministic RNG.
    pub fn shuffle(&mut self, rng: &mut SeededRng) {
        rng.shuffle(&mut self.cards);
    }
}

#[cfg(test)]
mod tests {
    use super::{CardInstance, Zone};
    use crate::domain::game::Conditions;
    use crate::domain::types::ids::{CardDefId, CardId};

    fn instance(raw: u32) -> CardInstance {
        CardInstance::new(
            CardId::from_raw(raw),
            CardDefId::from_raw(raw),
            Conditions::in_deck(),
        )
    }

    #[test]
    fn new_zone_is_empty() {
        let zone = Zone::new();
        assert!(zone.is_empty());
        assert_eq!(zone.len(), 0);
    }

    #[test]
    fn push_preserves_insertion_order() {
        let mut zone = Zone::new();
        zone.push(instance(0));
        zone.push(instance(1));
        zone.push(instance(2));

        let ids: Vec<u32> = zone.iter().map(|c| c.id().as_raw()).collect();
        assert_eq!(ids, vec![0, 1, 2]);
        assert_eq!(zone.len(), 3);
        assert!(!zone.is_empty());
    }
}
