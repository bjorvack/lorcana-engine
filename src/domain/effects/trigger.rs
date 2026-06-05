//! Trigger conditions for triggered abilities (§7.4).

use super::target::{CharacterFilter, TargetSide};
use crate::domain::types::card::Classification;
use serde::{Deserialize, Serialize};

/// A per-character game event that a trigger watches for (the "what happened").
///
/// Paired with a [`CharacterFilter`] scope in
/// [`TriggerCondition::WhenCharacterEvent`] (the "to which character"). The scope
/// expresses *self* (`IsSource` — "this character"), *relational* ("one of your
/// other characters" = `And([Side(Yours), Not(IsSource)])`, "an opposing
/// character" = `Side(Opposing)`), and anything else the filter algebra allows —
/// so no event needs a per-scope trigger variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopedEvent {
    /// A character quests (§4.3.5).
    Quests,
    /// A character sings a song (§6.3.3).
    Sings,
    /// A character challenges another character (the actor is the challenger,
    /// §4.3.6).
    Challenges,
    /// A character is challenged (the actor is the challenge target, §4.3.6).
    Challenged,
    /// A character banishes another character in a challenge (the actor is the
    /// banisher, §4.3.6.16).
    BanishesInChallenge,
    /// A character is banished (§1.9.1.1); `requires_challenge` restricts it to
    /// banishment that happened in a challenge ("…is banished in a challenge").
    Banished {
        /// Only fire when the banishment happened in a challenge.
        requires_challenge: bool,
    },
    /// A character is dealt damage (the trigger amount carries how much, §4.3.6.16).
    DealtDamage,
    /// Damage is removed from a character (§9.4).
    DamageRemoved,
    /// A character is readied (at the start of turn, or via an effect).
    Readies,
    /// A character leaves play — by any departure (banished, returned to hand,
    /// put into the inkwell or deck). §1.9.
    LeavesPlay,
}

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
/// Per-character events (quest / sing / challenge / is-challenged /
/// banishes-in-challenge / is-banished ± in a challenge / dealt-damage /
/// damage-removed / readies) are a single [`Self::WhenCharacterEvent`] carrying a
/// [`ScopedEvent`] and a [`CharacterFilter`] scope, so "this" / "one of your other
/// characters" / "an opposing character" all fall out of the filter algebra and
/// `enqueue_character_event` (no per-scope variants). The remaining variants are
/// the non-character events (play-this, play-a-category, card-under, turn
/// boundaries, card-into-inkwell).
///
/// Remaining taxonomy gaps (tracked in `docs/planning/IMPLEMENTATION_PLAN.md`):
/// move-to-location / "while here" (locations), "leaves play" (generalizing
/// banish/bounce/inkwell), and "draw" triggers — add a [`ScopedEvent`] (for
/// character-scoped events) or a new variant (for player/zone events), plus the
/// firing site and a scenario test.
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
    /// "Whenever you play a [category]" — fires on another card the controller
    /// plays that matches the category (~90 cards).
    WhenYouPlay(CardCategory),
    /// "Whenever a card is put under this character" — fires when a card is placed
    /// under this one (e.g. via Boost, §10.4).
    WhenCardPutUnder,
    /// "At the start of your turn" — fires for the active player's cards and
    /// resolves in the Set step (§4.2.2.3).
    AtStartOfTurn,
    /// "At the end of your turn" — fires for the active player's cards and
    /// resolves in the End of Turn phase (§4.4.1).
    AtEndOfTurn,
    /// "Whenever a card is put into your inkwell" — fires when a card is moved
    /// to the inkwell zone.
    WhenCardPutInInkwell,
    /// "Whenever you draw a card" — fires for the drawing player's in-play cards,
    /// once per card drawn (natural draw step and effect-driven draws; not the
    /// opening hand).
    WhenYouDraw,
    /// A per-character event scoped by a [`CharacterFilter`]: fires for the watcher
    /// when a character matching `scope` (relative to the watcher) performs
    /// `event`. Covers self ("this character quests" — `IsSource`), relational
    /// ("one of your other characters is banished" — `And([Side(Yours),
    /// Not(IsSource)])`; "an opposing character is dealt damage" —
    /// `Side(Opposing)`), and any other algebraic scope, so quest / sing /
    /// challenge / banish / damage / ready triggers need no per-scope variants.
    WhenCharacterEvent {
        /// What happened.
        event: ScopedEvent,
        /// Which character it happened to, relative to the watcher.
        scope: CharacterFilter,
    },
}

impl TriggerCondition {
    /// A scoped per-character event with the given scope filter.
    #[must_use]
    pub const fn character_event(event: ScopedEvent, scope: CharacterFilter) -> Self {
        Self::WhenCharacterEvent { event, scope }
    }

    /// Self (`IsSource`) sugar for a scoped event — "this character …".
    #[must_use]
    const fn this(event: ScopedEvent) -> Self {
        Self::character_event(event, CharacterFilter::IsSource)
    }

    /// "Whenever this character quests" (§4.3.5).
    #[must_use]
    pub const fn when_this_quests() -> Self {
        Self::this(ScopedEvent::Quests)
    }
    /// "Whenever this character sings a song" (§6.3.3).
    #[must_use]
    pub const fn when_this_sings() -> Self {
        Self::this(ScopedEvent::Sings)
    }
    /// "Whenever this character challenges another character" (§4.3.6).
    #[must_use]
    pub const fn when_this_challenges() -> Self {
        Self::this(ScopedEvent::Challenges)
    }
    /// "Whenever this character is challenged" (§4.3.6).
    #[must_use]
    pub const fn when_challenged() -> Self {
        Self::this(ScopedEvent::Challenged)
    }
    /// "Whenever this character banishes another character in a challenge" (§4.3.6.16).
    #[must_use]
    pub const fn when_banishes_in_challenge() -> Self {
        Self::this(ScopedEvent::BanishesInChallenge)
    }
    /// "When this character is banished" (§1.9.1.1).
    #[must_use]
    pub const fn when_banished() -> Self {
        Self::this(ScopedEvent::Banished {
            requires_challenge: false,
        })
    }
    /// "When this character is banished in a challenge".
    #[must_use]
    pub const fn when_banished_in_challenge() -> Self {
        Self::this(ScopedEvent::Banished {
            requires_challenge: true,
        })
    }
    /// "Whenever this character is dealt damage" (§4.3.6.16).
    #[must_use]
    pub const fn when_this_dealt_damage() -> Self {
        Self::this(ScopedEvent::DealtDamage)
    }
    /// "Whenever an opposing character is dealt damage".
    #[must_use]
    pub const fn when_opposing_dealt_damage() -> Self {
        Self::character_event(
            ScopedEvent::DealtDamage,
            CharacterFilter::Side(TargetSide::Opposing),
        )
    }
    /// "Whenever you remove damage from this character" (§9.4).
    #[must_use]
    pub const fn when_damage_removed_from_this() -> Self {
        Self::this(ScopedEvent::DamageRemoved)
    }
    /// "Whenever this character is readied".
    #[must_use]
    pub const fn when_this_readies() -> Self {
        Self::this(ScopedEvent::Readies)
    }
}
