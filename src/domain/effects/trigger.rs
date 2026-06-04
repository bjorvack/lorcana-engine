//! Trigger conditions for triggered abilities (§7.4).

use crate::domain::types::card::Classification;
use serde::{Deserialize, Serialize};

/// A category of card a "whenever you play a …" trigger watches for (§7.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardCategory {
    /// A character, optionally filtered by a classification ("a Floodborn
    /// character"); `None` matches any character.
    Character(Option<Classification>),
    /// An action.
    Action,
    /// A song (an action with the Song classification).
    Song,
    /// An item.
    Item,
    /// A location.
    Location,
}

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerCondition {
    /// "When you play this character/item/location" — fires on the source card
    /// entering play (the dominant trigger, ~480 cards).
    WhenYouPlayThis,
    /// "When you play this character/item/location via Shift" — fires only if
    /// the card was played using its Shift ability (~23 cards).
    WhenYouPlayThisWithShift,
    /// "Whenever this character quests" (~200 cards).
    WhenThisQuests,
    /// "Whenever you play a [category]" — fires on another card the controller
    /// plays that matches the category (~90 cards).
    WhenYouPlay(CardCategory),
    /// "Whenever this character challenges another character" — fires for the
    /// challenger when it is declared (§4.3.6).
    WhenThisChallenges,
    /// "Whenever this character is challenged" — fires for the challenged
    /// character (§4.3.6).
    WhenChallenged,
    /// "Whenever this character banishes another character in a challenge"
    /// (§4.3.6.16) — fires for the challenger when its challenge target is
    /// banished.
    WhenBanishesInChallenge,
    /// "When this character/location is banished" — fires for a card as it leaves
    /// play to the discard (§1.9.1.1, §9.4).
    WhenBanished,
    /// "When this character is banished **in a challenge**" — the banished-side
    /// counterpart of `WhenBanishesInChallenge` (Marshmallow, `HeiHei`).
    WhenBanishedInChallenge,
    /// "Whenever a card is put under this character" — fires when a card is placed
    /// under this one (e.g. via Boost, §10.4).
    WhenCardPutUnder,
    /// "At the start of your turn" — fires for the active player's cards and
    /// resolves in the Set step (§4.2.2.3).
    AtStartOfTurn,
    /// "At the end of your turn" — fires for the active player's cards and
    /// resolves in the End of Turn phase (§4.4.1).
    AtEndOfTurn,
    /// "Whenever this character is dealt damage" — fires when damage is marked
    /// on the character (~16 cards).
    WhenThisIsDealtDamage,
    /// "Whenever an opposing character is dealt damage" — fires when an opponent's
    /// character takes damage.
    WhenOpposingIsDealtDamage,
    /// "Whenever you remove damage from this character" — fires when damage
    /// counters are removed from the character.
    WhenDamageRemovedFromThis,
    /// "Whenever this character is readied" — fires when the character becomes
    /// ready (at the start of turn, or via an effect).
    WhenThisReadies,
}
