//! Represents a step within a phase

use serde::{Deserialize, Serialize};

/// Represents a step within a phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Step {
    // Beginning Phase Steps
    /// Ready step - "during your turn" effects begin and all your cards ready (§4.2.1)
    Ready,
    /// Set step - drying characters become dry and you gain lore from locations (§4.2.2)
    Set,
    /// Draw step - draw a card (the starting player skips this on turn 1) (§4.2.3)
    Draw,

    // Main Phase Steps
    /// Main step - main actions (play cards, quest, challenge, etc.)
    Main,

    // End of Turn Phase Steps
    /// End step - end-of-turn triggers resolve and "this turn" effects end (§4.4)
    End,
}
