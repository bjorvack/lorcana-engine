//! Effect system for card abilities and game actions

pub mod effect;
pub mod target;
pub mod trigger;

// Re-export for convenience
pub use effect::{
    Amount, DeckPosition, DelayedWhen, Destination, DiscardAmount, Effect, MoveSource, PlayerScope,
};
pub use target::{CharacterFilter, Comparison, NumericFilter, Target, TargetSide};
pub use trigger::{CardCategory, TriggerCondition};
