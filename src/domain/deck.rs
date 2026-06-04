//! Decks: a list of `(card, count)` plus construction/validation against the
//! deck-building rules (§2.1.1) and the community plain-text share format.
//!
//! A deck stores only **card ids + counts**; all card data (name, ink, copy
//! limit) is fetched from the [`CardRegistry`]. The plain-text format carries
//! `count` + full **name** (no printing), so importing resolves each name to some
//! printing and exporting groups printings back by name.

use crate::domain::cards::CardRegistry;
use crate::domain::types::card::InkType;
use crate::domain::types::ids::CardDefId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One deck entry: a card and how many copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckCard {
    /// The card (a specific printing).
    pub card: CardDefId,
    /// How many copies.
    pub count: u32,
}

/// A deck: a name and its cards (ids + counts only).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deck {
    /// The deck's display name.
    #[serde(default)]
    pub name: String,
    /// The cards and their counts.
    #[serde(default)]
    pub cards: Vec<DeckCard>,
}

/// A deck-building rule violation (§2.1.1).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeckError {
    /// Fewer than 60 cards (§2.1.1.1).
    #[error("deck has {total} cards; at least 60 are required")]
    TooFewCards {
        /// The deck's total card count.
        total: u32,
    },
    /// More than two ink types (§2.1.1.2).
    #[error("deck uses {count} ink types; at most 2 are allowed")]
    TooManyInks {
        /// How many distinct ink types the deck uses.
        count: usize,
    },
    /// More than the allowed copies of one full name (§2.1.1.3).
    #[error("{name:?}: {count} copies exceed the limit of {max}")]
    TooManyCopies {
        /// The full name.
        name: String,
        /// How many copies the deck has.
        count: u32,
        /// The allowed maximum (default 4, or the card's override).
        max: u32,
    },
    /// A deck entry references a card not in the registry.
    #[error("unknown card id {0:?}")]
    UnknownCard(CardDefId),
    /// A plain-text line named a card not in the registry.
    #[error("unknown card name {0:?}")]
    UnknownCardName(String),
    /// A plain-text line couldn't be parsed as `<count> <name>`.
    #[error("malformed deck line {0:?}")]
    BadLine(String),
}

impl Deck {
    /// Total number of cards.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.cards.iter().map(|c| c.count).sum()
    }

    /// Add `count` copies of `card` (merging with any existing entry).
    pub fn add(&mut self, card: CardDefId, count: u32) {
        if let Some(entry) = self.cards.iter_mut().find(|c| c.card == card) {
            entry.count += count;
        } else {
            self.cards.push(DeckCard { card, count });
        }
    }

    /// Expand to the flat list of card ids (each repeated `count` times), e.g. to
    /// seed a game via `GameState::new`.
    #[must_use]
    pub fn expand(&self) -> Vec<CardDefId> {
        let mut out = Vec::with_capacity(self.total() as usize);
        for entry in &self.cards {
            for _ in 0..entry.count {
                out.push(entry.card);
            }
        }
        out
    }

    /// Check the deck against the deck-building rules, returning **all** violations.
    ///
    /// # Errors
    /// Returns every [`DeckError`] found (empty `Ok` if the deck is legal).
    pub fn validate(&self, registry: &CardRegistry) -> Result<(), Vec<DeckError>> {
        let mut errors = Vec::new();

        // §2.1.1.1 — at least 60 cards.
        let total = self.total();
        if total < 60 {
            errors.push(DeckError::TooFewCards { total });
        }

        // Resolve definitions; collect ink identity and per-full-name counts.
        let mut inks: Vec<InkType> = Vec::new();
        let mut by_name: BTreeMap<String, (u32, u32)> = BTreeMap::new(); // name -> (count, max)
        for entry in &self.cards {
            let Some(def) = registry.get(entry.card) else {
                errors.push(DeckError::UnknownCard(entry.card));
                continue;
            };
            for ink in def.ink_types() {
                if !inks.contains(ink) {
                    inks.push(*ink);
                }
            }
            let name = def.names().first().cloned().unwrap_or_default();
            let slot = by_name
                .entry(name)
                .or_insert_with(|| (0, def.max_deck_copies()));
            slot.0 += entry.count;
            slot.1 = slot.1.max(def.max_deck_copies());
        }

        // §2.1.1.2 — at most two ink types (a dual-ink card commits both colours).
        if inks.len() > 2 {
            errors.push(DeckError::TooManyInks { count: inks.len() });
        }

        // §2.1.1.3 — copies of one full name (printings share the budget).
        for (name, (count, max)) in by_name {
            if count > max {
                errors.push(DeckError::TooManyCopies { name, count, max });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Render the deck in the community plain-text format (`<count> <full name>`
    /// per line), grouping printings by name (sorted). The printing is lost.
    #[must_use]
    pub fn to_text(&self, registry: &CardRegistry) -> String {
        let mut by_name: BTreeMap<String, u32> = BTreeMap::new();
        for entry in &self.cards {
            let name = registry
                .get(entry.card)
                .and_then(|d| d.names().first().cloned())
                .unwrap_or_else(|| format!("#{:?}", entry.card));
            *by_name.entry(name).or_default() += entry.count;
        }
        let mut out = String::new();
        for (name, count) in by_name {
            let _ = writeln!(out, "{count} {name}");
        }
        out
    }

    /// Parse the community plain-text format (`<count> <full name>` per line;
    /// blank lines and `#` comments ignored), resolving each name to a printing.
    ///
    /// # Errors
    /// Returns [`DeckError::BadLine`] / [`DeckError::UnknownCardName`] on the first
    /// unparseable or unresolvable line.
    pub fn from_text(text: &str, registry: &CardRegistry) -> Result<Self, DeckError> {
        let mut deck = Self::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (count_str, name) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| DeckError::BadLine(line.to_string()))?;
            let count: u32 = count_str
                .trim_end_matches('x')
                .parse()
                .map_err(|_| DeckError::BadLine(line.to_string()))?;
            let name = name.trim();
            let card = registry
                .find_by_name(name)
                .ok_or_else(|| DeckError::UnknownCardName(name.to_string()))?;
            deck.add(card, count);
        }
        Ok(deck)
    }
}
