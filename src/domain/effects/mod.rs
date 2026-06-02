//! Effect system for card abilities and game actions

pub mod builtin;
pub mod effect;
pub mod executor;
pub mod trigger;

// Re-export for convenience
pub use effect::Effect;
pub use trigger::TriggerCondition;
