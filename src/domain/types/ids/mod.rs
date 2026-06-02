//! Type-safe identifiers for game entities

pub mod card_def_id;
pub mod card_id;
pub mod game_id;
pub mod player_id;
pub mod zone_id;

// Re-export for convenience
pub use card_def_id::CardDefId;
pub use card_id::CardId;
pub use game_id::GameId;
pub use player_id::PlayerId;
pub use zone_id::ZoneId;
