//! Rules: legality checks and the game-state check (§1.9).

pub mod game_state_check;
pub mod required_action;
pub mod win_loss;

// Re-export for convenience
pub use game_state_check::game_state_check;
pub use required_action::RequiredAction;
pub use win_loss::{check_win_loss, lore_to_win};
