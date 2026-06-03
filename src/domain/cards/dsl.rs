//! The card **ability DSL**: a readable TOML surface for authoring triggered
//! abilities, mapped onto the engine's [`Effect`] AST.
//!
//! Hybrid design: the effect *tree* is structured (verb tables, `do = [..]`
//! sequences), while leaf **selectors** (targets / player scopes) accept a compact
//! string — `"chosen opposing character"`, `"another Villain character"`, `"each
//! opponent"` — *or* the full structured AST form as a fallback (a TOML table,
//! deserialized via the AST type's own serde). Nothing the algebra can express is
//! lost.
//!
//! ```toml
//! [[card.abilities]]
//! on = "play"                       # trigger
//! do = { draw = 1 }                 # effect
//!
//! [[card.abilities]]
//! on = "quest"
//! may = true                        # "you may …"
//! do = [
//!   { deal_damage = 2, to = "chosen opposing character" },
//!   { gain_lore = 1 },
//! ]
//! ```

use super::TriggeredAbility;
use super::loader::keyword_from;
use crate::domain::effects::{
    Amount, CardCategory, CharacterFilter, DiscardAmount, DiscardBy, Effect, PlayerScope, Target,
    TargetSide, TriggerCondition,
};
use crate::domain::game::Property;
use crate::domain::types::card::Classification;
use serde::Deserialize;
use toml::Value;

/// One `[[card.abilities]]` table.
#[derive(Debug, Clone, Deserialize)]
pub struct TomlAbility {
    /// The trigger ("play", "quest", "banish", …).
    pub on: String,
    /// "You may …" — optional ability.
    #[serde(default)]
    pub may: bool,
    /// The effect: a verb table `{ draw = 1 }`, or an array of them (a sequence).
    #[serde(rename = "do")]
    pub effect: Value,
}

impl TomlAbility {
    /// Build the [`TriggeredAbility`], or describe why it couldn't be parsed.
    ///
    /// # Errors
    /// Returns a human-readable detail string on any unmappable trigger/effect.
    pub fn to_ability(&self) -> Result<TriggeredAbility, String> {
        let condition = trigger_from(&self.on)?;
        let effect = effect_from_value(&self.effect)?;
        Ok(if self.may {
            TriggeredAbility::optional(condition, effect)
        } else {
            TriggeredAbility::new(condition, effect)
        })
    }
}

/// Map a trigger name to a [`TriggerCondition`].
fn trigger_from(s: &str) -> Result<TriggerCondition, String> {
    Ok(match s {
        "play" | "play_this" => TriggerCondition::WhenYouPlayThis,
        "quest" => TriggerCondition::WhenThisQuests,
        "challenge" => TriggerCondition::WhenThisChallenges,
        "challenged" => TriggerCondition::WhenChallenged,
        "banish" | "banished" => TriggerCondition::WhenBanished,
        "banished_in_challenge" => TriggerCondition::WhenBanishedInChallenge,
        "banishes_in_challenge" => TriggerCondition::WhenBanishesInChallenge,
        "start_of_turn" => TriggerCondition::AtStartOfTurn,
        "end_of_turn" => TriggerCondition::AtEndOfTurn,
        other => return Err(format!("unknown trigger {other:?}")),
    })
}

/// Parse an effect: a sequence (`[..]` → [`Effect::All`]) or a verb table.
fn effect_from_value(v: &Value) -> Result<Effect, String> {
    match v {
        Value::Array(items) => Ok(Effect::All(
            items
                .iter()
                .map(effect_from_value)
                .collect::<Result<_, _>>()?,
        )),
        Value::Table(t) => effect_from_table(t),
        other => Err(format!("expected an effect table, got {other}")),
    }
}

#[allow(clippy::too_many_lines)] // one verb-dispatch table
fn effect_from_table(t: &toml::Table) -> Result<Effect, String> {
    // The integer argument carried directly by a verb key (e.g. `draw = 1`).
    let int = |key: &str| -> Result<i32, String> {
        t.get(key)
            .and_then(Value::as_integer)
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| format!("{key}: expected an integer"))
    };
    let fixed = |key: &str| int(key).map(Amount::fixed);
    // A target from `to` / `from` / `target` (default: the source itself).
    let tgt = || -> Result<Target, String> {
        t.get("to")
            .or_else(|| t.get("from"))
            .or_else(|| t.get("target"))
            .map_or(Ok(Target::SelfCard), target_from_value)
    };
    let scope = |default: PlayerScope| -> Result<PlayerScope, String> {
        match t.get("who") {
            Some(Value::String(s)) => {
                scope_from_str(s).ok_or_else(|| format!("unknown scope {s:?}"))
            }
            Some(other) => Err(format!("expected a player scope string, got {other}")),
            None => Ok(default),
        }
    };

    if t.contains_key("draw") {
        Ok(Effect::Draw {
            who: scope(PlayerScope::You)?,
            amount: fixed("draw")?,
        })
    } else if t.contains_key("gain_lore") {
        Ok(Effect::Lore {
            who: scope(PlayerScope::You)?,
            amount: fixed("gain_lore")?,
        })
    } else if t.contains_key("lose_lore") {
        Ok(Effect::Lore {
            who: scope(PlayerScope::EachOpponent)?,
            amount: Amount::fixed(-int("lose_lore")?),
        })
    } else if t.contains_key("deal_damage") {
        Ok(Effect::DealDamage {
            target: tgt()?,
            amount: fixed("deal_damage")?,
        })
    } else if t.contains_key("remove_damage") {
        Ok(Effect::RemoveDamage {
            target: tgt()?,
            amount: fixed("remove_damage")?,
        })
    } else if t.contains_key("give_strength") {
        Ok(Effect::GiveStrengthThisTurn {
            target: tgt()?,
            amount: fixed("give_strength")?,
        })
    } else if let Some(v) = t.get("banish") {
        Ok(Effect::Banish(target_from_value(v)?))
    } else if let Some(v) = t.get("exert") {
        Ok(Effect::Exert(target_from_value(v)?))
    } else if let Some(v) = t.get("ready") {
        Ok(Effect::Ready(target_from_value(v)?))
    } else if let Some(v) = t.get("freeze") {
        Ok(Effect::Freeze(target_from_value(v)?))
    } else if t.contains_key("discard") {
        Ok(Effect::Discard {
            who: scope(PlayerScope::You)?,
            amount: DiscardAmount::Count(u32::try_from(int("discard")?).unwrap_or(0)),
            by: DiscardBy::Owner,
        })
    } else if let Some(Value::String(kw)) = t.get("grant_keyword") {
        let keyword = keyword_from(kw).ok_or_else(|| format!("unknown keyword {kw:?}"))?;
        Ok(Effect::GrantThisTurn {
            target: tgt()?,
            property: Property::Keyword(keyword),
        })
    } else {
        Err(format!("no known effect verb in {t:?}"))
    }
}

/// A target: a compact selector string, or the structured AST form (a table).
fn target_from_value(v: &Value) -> Result<Target, String> {
    match v {
        Value::String(s) => parse_selector(s).ok_or_else(|| format!("unparseable target {s:?}")),
        table @ Value::Table(_) => table
            .clone()
            .try_into::<Target>()
            .map_err(|e| format!("bad structured target: {e}")),
        other => Err(format!("expected a target, got {other}")),
    }
}

/// Parse a compact target selector, e.g. `"chosen opposing Villain character"`,
/// `"another chosen character"`, `"all your characters"`, `"chosen item"`.
fn parse_selector(s: &str) -> Option<Target> {
    let lower = s.to_lowercase();
    if lower == "self" || lower == "this" {
        return Some(Target::SelfCard);
    }
    // Must name what's being selected, else it isn't a selector we understand.
    if !["character", "item", "location", "permanent"]
        .iter()
        .any(|n| lower.contains(n))
    {
        return None;
    }
    let all = lower.starts_with("all");
    let side = if lower.contains("opposing") {
        TargetSide::Opposing
    } else if lower.contains("your") || lower.contains("own") {
        TargetSide::Yours
    } else {
        TargetSide::Any
    };
    let another = lower.contains("another") || lower.contains(" other ");

    // The noun decides character vs item/location.
    let known = [
        "all",
        "chosen",
        "another",
        "other",
        "opposing",
        "your",
        "own",
        "character",
        "characters",
        "item",
        "items",
        "location",
        "locations",
        "permanent",
        "permanents",
    ];
    let classifications: Vec<&str> = s
        .split_whitespace()
        .filter(|w| !known.contains(&w.to_lowercase().as_str()))
        .collect();

    let category = if lower.contains("item") {
        Some(CardCategory::Item)
    } else if lower.contains("location") {
        Some(CardCategory::Location)
    } else {
        None // characters (the default) need no Category gate
    };

    let is_permanent = category.is_some();
    let mut filter = CharacterFilter::any(side);
    if let Some(cat) = category {
        filter = filter.and(CharacterFilter::Category(cat));
    }
    for c in classifications {
        filter = filter.and(CharacterFilter::Classification(Classification::new(c)));
    }
    if another {
        filter = filter.and(CharacterFilter::negate(CharacterFilter::IsSource));
    }

    Some(if is_permanent {
        Target::ChosenPermanent { filter }
    } else if all {
        Target::AllCharacters { filter }
    } else {
        Target::ChosenCharacter { filter }
    })
}

/// Map a player-scope string to a [`PlayerScope`].
fn scope_from_str(s: &str) -> Option<PlayerScope> {
    Some(match s.to_lowercase().as_str() {
        "you" | "yourself" => PlayerScope::You,
        "each opponent" | "opponents" => PlayerScope::EachOpponent,
        "each player" | "all players" => PlayerScope::EachPlayer,
        "chosen opponent" => PlayerScope::ChosenOpponent,
        "chosen player" => PlayerScope::ChosenPlayer,
        _ => return None,
    })
}
