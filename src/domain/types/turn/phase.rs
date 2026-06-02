//! Represents a phase in a turn

use super::Step;
use serde::{Deserialize, Serialize};

/// Represents a phase in a turn
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Phase {
    /// Beginning phase - ready, set, draw steps
    Beginning,
    /// Main phase - play cards, quest, challenge
    Main,
    /// End phase - end of turn effects and cleanup
    End,
}

impl Phase {
    /// Get the steps for this phase in order
    #[must_use]
    pub fn steps(&self) -> Vec<Step> {
        match self {
            Self::Beginning => vec![Step::Ready, Step::Set, Step::Draw],
            Self::Main => vec![Step::Main],
            Self::End => vec![Step::End],
        }
    }
}
