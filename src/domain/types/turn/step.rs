//! Represents a step within a phase

use serde::{Deserialize, Serialize};

/// Represents a step within a phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Step {
    // Beginning Phase Steps
    /// Ready step - ready all cards
    Ready,
    /// Set step - set ink
    Set,
    /// Draw step - draw a card
    Draw,

    // Main Phase Steps
    /// Main step - main actions
    Main,

    // End Phase Steps
    /// End step - end of turn effects
    End,
    /// Cleanup step - cleanup and end turn
    Cleanup,
}
