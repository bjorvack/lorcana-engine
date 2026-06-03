//! Game state, turn structure, zones, and events

pub mod bag;
pub mod card_instance;
pub mod character_stats;
pub mod conditions;
pub mod events;
pub mod modifier;
pub mod pending;
pub mod player_state;
pub mod rng;
pub mod state;
pub mod status;
pub mod zone;
pub mod zone_kind;

// Re-export for convenience
pub use bag::{BagEntry, TriggerId};
pub use card_instance::CardInstance;
pub use character_stats::{CharacterStats, LocationStats};
pub use conditions::Conditions;
pub use events::GameEvent;
pub use modifier::{
    Condition, ModifierDuration, ModifierTarget, Permission, Property, PropertyModifier,
    Restriction, RuleModifier, Stat, StatModifier,
};
pub use pending::PendingDecision;
pub use player_state::PlayerState;
pub use rng::SeededRng;
pub use state::GameState;
pub use status::GameStatus;
pub use zone::Zone;
pub use zone_kind::ZoneKind;
