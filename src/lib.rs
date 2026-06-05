//! Lorcana Engine
//!
//! A headless, deterministic game engine for Disney's Lorcana trading card game.

// Domain modules
pub mod application;
pub mod domain;

// Re-export commonly used types for convenience
pub use application::{Game, SetupError};
pub use domain::cards::{
    AbilityCost, ActivatedAbility, CardDefinition, CardKind, CardRegistry, GameRuleStatic, Keyword,
    LoadError, ShiftAbility, ShiftCost, ShiftKind, StaticAbility, StaticTarget, TomlCard,
    TriggeredAbility, TurnGate, load_toml, load_toml_from,
};
pub use domain::deck::{Deck, DeckCard, DeckError};
pub use domain::effects::{
    Amount, CardCategory, CharacterFilter, Comparison, CountCondition, DeckPosition, DelayedWhen,
    Destination, DiscardAmount, DiscardBy, Effect, MoveSource, NumericFilter, PlayerScope,
    ScopedEvent, Target, TargetSide, TriggerCondition,
};
pub use domain::engine::{Decision, Input, Rejected, apply, start};
pub use domain::game::{
    BagEntry, CardInstance, CharacterStats, ChoiceRef, ChoiceThen, Condition, Conditions,
    GameEvent, GameState, GameStatus, LocationStats, ModifierDuration, ModifierTarget,
    PendingDecision, Permission, PlayerState, Property, PropertyModifier, Restriction,
    RuleModifier, SeededRng, Stat, StatModifier, TriggerId, Zone, ZoneKind,
};
pub use domain::rules::{RequiredAction, check_win_loss, game_state_check, lore_to_win};
pub use domain::types::{
    card::{CardType, Classification, InkType, Rarity, SetInfo},
    ids::{CardDefId, CardId, GameId, PlayerId, ZoneId},
    turn::{Phase, Step},
};
