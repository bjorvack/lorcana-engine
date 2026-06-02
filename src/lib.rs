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
pub use domain::cards::{
    AbilityCost, ActivatedAbility, CardDefinition, CardKind, CardRegistry, GameRuleStatic, Keyword,
    ShiftAbility, ShiftCost, ShiftKind, StaticAbility, StaticTarget, TriggeredAbility,
};
pub use domain::effects::{
    CardCategory, CharacterFilter, Effect, Target, TargetSide, TriggerCondition,
};
pub use domain::engine::{Decision, Input, Rejected, apply, start};
pub use domain::game::{
    BagEntry, CardInstance, CharacterStats, Conditions, GameEvent, GameState, GameStatus,
    LocationStats, ModifierDuration, ModifierTarget, PendingDecision, PlayerState, RuleModifier,
    SeededRng, Stat, StatModifier, TriggerId, Zone, ZoneKind,
};
pub use domain::rules::{RequiredAction, check_win_loss, game_state_check, lore_to_win};
pub use domain::types::{
    card::{CardType, Classification, InkType, Rarity, SetInfo},
    ids::{CardDefId, CardId, GameId, PlayerId, ZoneId},
    turn::{Phase, Step},
};
