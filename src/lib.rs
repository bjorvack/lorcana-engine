//! Lorcana Engine
//!
//! A headless, deterministic game engine for Disney's Lorcana trading card game.

// Domain modules
pub mod domain;

// Infrastructure modules
pub mod infrastructure;

// Application modules
pub mod application;

// Shared utilities
pub mod shared;

// Re-export commonly used types for convenience
pub use domain::cards::{CardDefinition, CardRegistry};
pub use domain::engine::{Input, Rejected, apply, start};
pub use domain::game::{
    CardInstance, Conditions, GameEvent, GameState, GameStatus, PlayerState, SeededRng, Zone,
    ZoneKind,
};
pub use domain::rules::{RequiredAction, check_win_loss, game_state_check, lore_to_win};
pub use domain::types::{
    card::{CardType, InkType, Rarity, SetInfo},
    ids::{CardDefId, CardId, GameId, PlayerId, ZoneId},
    turn::{Phase, Step},
};
