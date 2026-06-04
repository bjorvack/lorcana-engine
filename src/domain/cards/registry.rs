//! Card registry: lookup of card definitions by id.

use super::definition::CardDefinition;
use crate::domain::types::ids::CardDefId;
use std::collections::BTreeMap;

/// A lookup from [`CardDefId`] to its [`CardDefinition`].
///
/// This is reference data, kept separate from `GameState` (which holds only the
/// mutable, reproducible game state). A `BTreeMap` keeps iteration order
/// deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardRegistry {
    definitions: BTreeMap<CardDefId, CardDefinition>,
}

impl CardRegistry {
    /// Create an empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            definitions: BTreeMap::new(),
        }
    }

    /// Insert (or replace) a definition.
    pub fn insert(&mut self, definition: CardDefinition) {
        let _ = self.definitions.insert(definition.id(), definition);
    }

    /// Look up a definition by id.
    #[must_use]
    pub fn get(&self, id: CardDefId) -> Option<&CardDefinition> {
        self.definitions.get(&id)
    }

    /// Iterate all `(id, definition)` pairs (ascending id).
    pub fn iter(&self) -> impl Iterator<Item = (CardDefId, &CardDefinition)> {
        self.definitions.iter().map(|(id, def)| (*id, def))
    }

    /// The id of a card whose full name (or one of its names) matches `name` —
    /// the lowest such id, since printings share a name (§2.1.1.3). Used to
    /// resolve a plain-text deck list (which carries names, not printings).
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<CardDefId> {
        self.definitions
            .iter()
            .find(|(_, def)| def.has_name(name))
            .map(|(id, _)| *id)
    }
}

impl FromIterator<CardDefinition> for CardRegistry {
    fn from_iter<I: IntoIterator<Item = CardDefinition>>(iter: I) -> Self {
        let mut registry = Self::new();
        for definition in iter {
            registry.insert(definition);
        }
        registry
    }
}
