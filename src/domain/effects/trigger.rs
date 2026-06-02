//! Trigger conditions for triggered abilities (§7.4).

use serde::{Deserialize, Serialize};

/// The condition that makes a triggered ability fire (§7.4.2).
///
/// Kept deliberately small: only the conditions Slice 4 actually wires up are
/// modeled. New variants are added as later slices need them — see the TODO
/// below for the full space found by surveying the card pool (2,314 cards with
/// text). When adding a variant, also add: (a) detection in the engine (match it
/// against the relevant `GameEvent`), and (b) a scenario test.
///
/// TODO(trigger taxonomy — add variants as needed, grounded in the card survey):
/// The rollout (which slice each kind lands in, plus the cross-scope
/// event→trigger matcher) is tracked in `docs/planning/IMPLEMENTATION_PLAN.md`
/// under "Trigger taxonomy rollout" (after Slice 4).
/// Most conditions also carry a *scope* filter naming which card the trigger
/// watches: `This` | `YoursOther` | `Yours` | `Any` | `Opposing` (and locations'
/// "while here"). Approximate frequencies in parentheses.
///   - Play / enters-play of another card by type/classification (~90):
///     "Whenever you play a song / action / character / Floodborn / [class]…".
///     (Self ETB and self-quest are the two implemented below.)
///   - Banish (~85): "When this character is banished", "…is banished in a
///     challenge", "…is challenged and banished", "whenever one of your
///     characters is banished", "whenever this character banishes another
///     character in a challenge".
///   - Challenge (~50): "whenever this character challenges", "…is challenged".
///   - Turn boundaries: "at the start of your turn" (41), "at the end of your
///     turn" (32).
///   - Damage (~16): "whenever this character is dealt damage", "whenever an
///     opposing character is damaged", "whenever you remove damage…".
///   - Sing a song (6): "whenever this character sings a song".
///   - Card put under a character (Boost, ~10); card put into the inkwell;
///     "whenever you ready this character"; move to a location / "quests while
///     here" (location); draw; leaves play.
///
/// These pair with the effect DSL (see `effects::effect`) and the bag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerCondition {
    /// "When you play this character/item/location" — fires on the source card
    /// entering play (the dominant trigger, ~480 cards).
    WhenYouPlayThis,
    /// "Whenever this character quests" (~200 cards).
    WhenThisQuests,
}
