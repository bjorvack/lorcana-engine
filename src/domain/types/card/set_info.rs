//! Represents the set a card belongs to

use serde::{Deserialize, Serialize};

/// Represents the set a card belongs to
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SetInfo {
    /// Set number (e.g., 1 for The First Chapter)
    pub set_number: u32,
    /// Card number within the set
    pub card_number: u32,
}

impl SetInfo {
    /// Create new set information
    #[must_use]
    pub const fn new(set_number: u32, card_number: u32) -> Self {
        Self {
            set_number,
            card_number,
        }
    }
}
