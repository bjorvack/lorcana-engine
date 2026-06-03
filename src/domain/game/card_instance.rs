//! A specific card instance within a game.

use super::{CharacterStats, Conditions, LocationStats};
use crate::domain::types::card::Classification;
use crate::domain::types::ids::{CardDefId, CardId};
use serde::{Deserialize, Serialize};

/// A physical card in a game: a unique [`CardId`] plus the printed card it
/// represents ([`CardDefId`]) and its current [`Conditions`].
///
/// `stats` holds the live [`CharacterStats`] while the card is an in-play
/// character; it is `None` for cards that aren't in-play characters (in a deck,
/// hand, inkwell, or for non-character types). `classifications` is denormalized
/// from the definition when the card enters play so selector matching (§7.8) can
/// stay state-only.
///
/// Not `Copy`: it owns a `Vec` of classifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardInstance {
    id: CardId,
    definition: CardDefId,
    conditions: Conditions,
    stats: Option<CharacterStats>,
    /// The live location characteristics while this is an in-play location (§6.5).
    location: Option<LocationStats>,
    /// The location this character has been moved to, if any (§4.3.7).
    at_location: Option<CardId>,
    classifications: Vec<Classification>,
    /// The printed ink cost, denormalized from the definition (so filtering by
    /// cost needs no registry, §6.2.6).
    printed_cost: u32,
    /// The card's names, denormalized from the definition ("named X").
    names: Vec<String>,
    /// Cards stacked **under** this one (§5.1.5–5.1.7). Only the top (this
    /// instance) is in play; under-cards are inert (not in play, can't be chosen).
    /// The pile is flat (deepest last) and the **whole** stack moves with the top
    /// when it leaves play (§5.1.7/§10.10.8 — see `take_under`).
    ///
    /// Shared by **Shift** (faceup character cards, §10.10) and **Boost**
    /// (facedown deck cards, §10.4 — deferred keyword). A character can hold both;
    /// Boost just adds entries with `facedown` set. TODO(Boost — Slice 6c+): wire
    /// the Boost activated ability to push the top deck card here facedown.
    under: Vec<Self>,
}

impl CardInstance {
    /// Create a card instance with the given conditions, no in-play stats, and
    /// no classifications.
    #[must_use]
    pub const fn new(id: CardId, definition: CardDefId, conditions: Conditions) -> Self {
        Self {
            id,
            definition,
            conditions,
            stats: None,
            location: None,
            at_location: None,
            classifications: Vec::new(),
            printed_cost: 0,
            names: Vec::new(),
            under: Vec::new(),
        }
    }

    /// The denormalized printed ink cost (§6.2.6).
    #[must_use]
    pub const fn printed_cost(&self) -> u32 {
        self.printed_cost
    }

    /// Set the denormalized printed cost (from the definition when entering play).
    pub const fn set_printed_cost(&mut self, cost: u32) {
        self.printed_cost = cost;
    }

    /// Whether this instance counts as having `name` ("named X").
    #[must_use]
    pub fn has_name(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }

    /// Set the denormalized names (from the definition when entering play).
    pub fn set_names(&mut self, names: Vec<String>) {
        self.names = names;
    }

    /// The instance id (unique within the game).
    #[must_use]
    pub const fn id(&self) -> CardId {
        self.id
    }

    /// The printed card this instance represents.
    #[must_use]
    pub const fn definition(&self) -> CardDefId {
        self.definition
    }

    /// The current conditions on this instance.
    #[must_use]
    pub const fn conditions(&self) -> Conditions {
        self.conditions
    }

    /// Mutable access to this instance's conditions.
    pub const fn conditions_mut(&mut self) -> &mut Conditions {
        &mut self.conditions
    }

    /// The live character stats, if this is an in-play character.
    #[must_use]
    pub const fn stats(&self) -> Option<CharacterStats> {
        self.stats
    }

    /// `true` if this instance is an in-play character (has character stats).
    #[must_use]
    pub const fn is_character(&self) -> bool {
        self.stats.is_some()
    }

    /// Set (or clear) this instance's live character stats.
    pub const fn set_stats(&mut self, stats: Option<CharacterStats>) {
        self.stats = stats;
    }

    /// The live location characteristics, if this is an in-play location (§6.5).
    #[must_use]
    pub const fn location_stats(&self) -> Option<LocationStats> {
        self.location
    }

    /// `true` if this instance is an in-play location.
    #[must_use]
    pub const fn is_location(&self) -> bool {
        self.location.is_some()
    }

    /// Set (or clear) this instance's live location characteristics.
    pub const fn set_location_stats(&mut self, location: Option<LocationStats>) {
        self.location = location;
    }

    /// The location this character is at, if any (§4.3.7).
    #[must_use]
    pub const fn at_location(&self) -> Option<CardId> {
        self.at_location
    }

    /// Record the location this character has moved to (§4.3.7).
    pub const fn set_at_location(&mut self, location: Option<CardId>) {
        self.at_location = location;
    }

    /// This instance's classifications (§6.2.6).
    #[must_use]
    pub fn classifications(&self) -> &[Classification] {
        &self.classifications
    }

    /// Whether this instance has the given classification.
    #[must_use]
    pub fn has_classification(&self, classification: &Classification) -> bool {
        self.classifications.contains(classification)
    }

    /// Set this instance's classifications (denormalized from the definition when
    /// the card enters play).
    pub fn set_classifications(&mut self, classifications: Vec<Classification>) {
        self.classifications = classifications;
    }

    /// The cards stacked under this one (top of the under-pile first, §10.10).
    #[must_use]
    pub fn under(&self) -> &[Self] {
        &self.under
    }

    /// Place `target` (and its own under-pile) directly under this card, forming
    /// or extending a stack when this card is shifted onto `target` (§10.10).
    /// The under-pile is kept flat.
    pub fn stack_onto(&mut self, mut target: Self) {
        let beneath = std::mem::take(&mut target.under);
        self.under.push(target);
        self.under.extend(beneath);
    }

    /// Put a single card under this one (e.g. a facedown deck card via Boost,
    /// §10.4). The card keeps its own conditions (Boost cards stay facedown).
    pub fn push_under(&mut self, card: Self) {
        self.under.push(card);
    }

    /// Remove and return the under-pile (e.g. when the stack leaves play and its
    /// cards move to the same zone, §10.10.8).
    pub fn take_under(&mut self) -> Vec<Self> {
        std::mem::take(&mut self.under)
    }

    /// Dissolve this card's stack into individual cards for a destination zone
    /// (§5.1.7). The top and **every** card under it — whether placed there by
    /// Shift (faceup) or Boost (facedown) — become **separate**, reset instances:
    /// all in-play / stack state (stats, classifications, damage, exerted/drying,
    /// facedown, the under-pile) is dropped and each card takes the destination
    /// zone's **default** `conditions`. E.g. a shifted character returned to hand
    /// becomes two faceup cards in hand; a Boost card stops being facedown unless
    /// the destination (the deck) is itself facedown.
    ///
    /// Every leave-play path (banish now; bounce-to-hand / shuffle-to-deck in
    /// Slice 8) must route through this with the destination zone's default
    /// conditions, so a stack never moves as one card and prior conditions reset.
    #[must_use]
    pub fn dissolve(mut self, conditions: Conditions) -> Vec<Self> {
        let under = std::mem::take(&mut self.under);
        let mut cards = Vec::with_capacity(1 + under.len());
        cards.push(Self::new(self.id, self.definition, conditions));
        for card in under {
            cards.push(Self::new(card.id, card.definition, conditions));
        }
        cards
    }
}
