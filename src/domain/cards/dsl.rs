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

use super::loader::keyword_from;
use super::{AbilityCost, ActivatedAbility, StaticAbility, StaticTarget, TriggeredAbility};
use crate::domain::effects::{
    Amount, CardCategory, CharacterFilter, Destination, DiscardAmount, DiscardBy, Effect,
    MoveSource, PlayerScope, Target, TargetSide, TriggerCondition,
};
use crate::domain::game::{Condition, Property, Stat};
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
        "play_action" => TriggerCondition::WhenYouPlay(CardCategory::Action),
        "play_song" => TriggerCondition::WhenYouPlay(CardCategory::Song),
        "play_character" => TriggerCondition::WhenYouPlay(CardCategory::Character(None)),
        "play_item" => TriggerCondition::WhenYouPlay(CardCategory::Item),
        "play_location" => TriggerCondition::WhenYouPlay(CardCategory::Location),
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
    // An amount that may be a literal int or a dynamic form (e.g. "per …").
    let amt = |key: &str| -> Result<Amount, String> {
        t.get(key)
            .map_or_else(|| Err(format!("{key}: missing")), amount_from_value)
    };
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
            amount: amt("draw")?,
        })
    } else if t.contains_key("gain_lore") {
        Ok(Effect::Lore {
            who: scope(PlayerScope::You)?,
            amount: amt("gain_lore")?,
        })
    } else if t.contains_key("lose_lore") {
        Ok(Effect::Lore {
            who: scope(PlayerScope::EachOpponent)?,
            amount: Amount::fixed(-int("lose_lore")?),
        })
    } else if t.contains_key("deal_damage") {
        Ok(Effect::DealDamage {
            target: tgt()?,
            amount: amt("deal_damage")?,
        })
    } else if t.contains_key("remove_damage") {
        Ok(Effect::RemoveDamage {
            target: tgt()?,
            amount: amt("remove_damage")?,
        })
    } else if t.contains_key("give_strength") {
        Ok(Effect::GiveStrengthThisTurn {
            target: tgt()?,
            amount: amt("give_strength")?,
        })
    } else if let Some(v) = t.get("banish") {
        Ok(Effect::Banish(target_from_value(v)?))
    } else if let Some(v) = t.get("exert") {
        Ok(Effect::Exert(target_from_value(v)?))
    } else if let Some(v) = t.get("ready") {
        Ok(Effect::Ready(target_from_value(v)?))
    } else if let Some(v) = t.get("freeze") {
        Ok(Effect::Freeze(target_from_value(v)?))
    } else if let Some(v) = t.get("return_to_hand") {
        Ok(Effect::Move {
            what: MoveSource::Card(target_from_value(v)?),
            to: Destination::Hand,
        })
    } else if let Some(v) = t.get("into_inkwell") {
        Ok(Effect::Move {
            what: MoveSource::Card(target_from_value(v)?),
            to: Destination::Inkwell,
        })
    } else if t.contains_key("discard") {
        Ok(Effect::Discard {
            who: scope(PlayerScope::You)?,
            amount: DiscardAmount::Count(u32::try_from(int("discard")?).unwrap_or(0)),
            by: DiscardBy::Owner,
        })
    } else if let Some(Value::String(cond)) = t.get("if_you_have") {
        let filter = parse_filter(cond).ok_or_else(|| format!("unparseable filter {cond:?}"))?;
        let then = t
            .get("then")
            .ok_or_else(|| "`if_you_have` needs a `then` effect".to_string())?;
        Ok(Effect::IfControl {
            filter,
            then: Box::new(effect_from_value(then)?),
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
    let filter = parse_filter(s)?;
    let is_permanent = lower.contains("item") || lower.contains("location");
    Some(if is_permanent {
        Target::ChosenPermanent { filter }
    } else if lower.starts_with("all") {
        Target::AllCharacters { filter }
    } else {
        Target::ChosenCharacter { filter }
    })
}

/// Parse a compact card filter — side + category + classifications + `another` —
/// e.g. `"your other Villain characters"`, `"opposing item"`. Requires a noun so
/// it's distinguishable from free text.
fn parse_filter(s: &str) -> Option<CharacterFilter> {
    let lower = s.to_lowercase();
    if !["character", "item", "location", "permanent"]
        .iter()
        .any(|n| lower.contains(n))
    {
        return None;
    }
    let side = if lower.contains("opposing") {
        TargetSide::Opposing
    } else if lower.contains("your") || lower.contains("own") {
        TargetSide::Yours
    } else {
        TargetSide::Any
    };
    let another = lower.contains("another") || lower.contains(" other ");
    let category = if lower.contains("item") {
        Some(CardCategory::Item)
    } else if lower.contains("location") {
        Some(CardCategory::Location)
    } else {
        None // characters (the default) need no Category gate
    };
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
    let mut filter = CharacterFilter::any(side);
    if let Some(cat) = category {
        filter = filter.and(CharacterFilter::Category(cat));
    }
    for c in s
        .split_whitespace()
        .filter(|w| !known.contains(&w.to_lowercase().as_str()))
    {
        filter = filter.and(CharacterFilter::Classification(Classification::new(c)));
    }
    if another {
        filter = filter.and(CharacterFilter::negate(CharacterFilter::IsSource));
    }
    Some(filter)
}

/// An amount: an integer (`Fixed`), a dynamic string, or the structured AST form.
/// Strings: `"per <filter>"` (for-each), `"cards in hand"`, `"damage on self"`,
/// `"<stat> of self"`.
fn amount_from_value(v: &Value) -> Result<Amount, String> {
    match v {
        Value::Integer(n) => i32::try_from(*n)
            .map(Amount::fixed)
            .map_err(|_| format!("amount {n} out of range")),
        Value::String(s) => amount_from_str(s).ok_or_else(|| format!("unparseable amount {s:?}")),
        table @ Value::Table(_) => table
            .clone()
            .try_into::<Amount>()
            .map_err(|e| format!("bad structured amount: {e}")),
        other => Err(format!("expected an amount, got {other}")),
    }
}

fn amount_from_str(s: &str) -> Option<Amount> {
    let lower = s.to_lowercase();
    if lower == "cards in hand" {
        return Some(Amount::CardsInHand);
    }
    if lower == "damage on self" || lower == "damage on this" {
        return Some(Amount::DamageOnSource);
    }
    if lower.starts_with("per ") {
        // Slice the original (case-preserving) string so classifications keep case.
        return parse_filter(s["per ".len()..].trim()).map(Amount::PerMatchingCharacter);
    }
    for (word, stat) in [
        ("strength", Stat::Strength),
        ("willpower", Stat::Willpower),
        ("lore", Stat::Lore),
    ] {
        if lower.starts_with(word) {
            return Some(Amount::StatOf {
                stat,
                target: Target::SelfCard,
            });
        }
    }
    None
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

/// One `[[card.activated]]` table: a cost and an effect.
#[derive(Debug, Clone, Deserialize)]
pub struct TomlActivated {
    /// The activation cost (`{ exert = true, ink = 1 }`).
    #[serde(default)]
    pub cost: TomlCost,
    /// The effect, in the same verb-table form as triggered abilities.
    #[serde(rename = "do")]
    pub effect: Value,
}

/// An activated-ability cost.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct TomlCost {
    /// Whether activating exerts the source (`{E}`).
    #[serde(default)]
    pub exert: bool,
    /// Ink to pay.
    #[serde(default)]
    pub ink: u32,
}

impl TomlActivated {
    /// Build the [`ActivatedAbility`].
    ///
    /// # Errors
    /// Returns a detail string if the effect can't be parsed.
    pub fn to_ability(&self) -> Result<ActivatedAbility, String> {
        Ok(ActivatedAbility::new(
            AbilityCost::new(self.cost.exert, self.cost.ink),
            effect_from_value(&self.effect)?,
        ))
    }
}

/// One `[[card.statics]]` table: a continuous stat modifier on a selector.
#[derive(Debug, Clone, Deserialize)]
pub struct TomlStatic {
    /// `+N`/`-N` to `{S}` (set exactly one of strength/willpower/lore).
    pub strength: Option<i32>,
    /// `+N`/`-N` to `{W}`.
    pub willpower: Option<i32>,
    /// `+N`/`-N` to `{L}`.
    pub lore: Option<i32>,
    /// Who it applies to: "this" / "your characters" / "your other Villain characters".
    #[serde(rename = "to")]
    pub target: String,
    /// "+N for each …": scales the delta by a count of matching cards.
    pub per: Option<String>,
    /// A gating condition ("while …"); currently only "exerted".
    #[serde(rename = "while")]
    pub while_: Option<String>,
}

impl TomlStatic {
    /// Build the [`StaticAbility`].
    ///
    /// # Errors
    /// Returns a detail string if exactly-one-stat or the target can't be resolved.
    pub fn to_static(&self) -> Result<StaticAbility, String> {
        let (stat, delta) = match (self.strength, self.willpower, self.lore) {
            (Some(d), None, None) => (Stat::Strength, d),
            (None, Some(d), None) => (Stat::Willpower, d),
            (None, None, Some(d)) => (Stat::Lore, d),
            _ => return Err("a static must set exactly one of strength/willpower/lore".into()),
        };
        let target = static_target_from_str(&self.target)
            .ok_or_else(|| format!("unparseable static target {:?}", self.target))?;
        let per = self
            .per
            .as_deref()
            .map(|p| {
                parse_filter(p)
                    .map(Amount::PerMatchingCharacter)
                    .ok_or_else(|| format!("unparseable `per` filter {p:?}"))
            })
            .transpose()?;
        let condition = self
            .while_
            .as_deref()
            .map(|w| match w.to_lowercase().as_str() {
                "exerted" => Ok(Condition::SourceExerted),
                other => Err(format!("unknown condition {other:?}")),
            })
            .transpose()?;
        Ok(StaticAbility {
            target,
            stat,
            delta,
            condition,
            per,
        })
    }
}

/// Parse a static target: "this" / "your [other] [Class…] characters".
fn static_target_from_str(s: &str) -> Option<StaticTarget> {
    let lower = s.to_lowercase();
    if lower == "self" || lower == "this" {
        return Some(StaticTarget::SelfCard);
    }
    if !lower.contains("character") {
        return None; // statics buff characters
    }
    let include_self = !(lower.contains("other") || lower.contains("another"));
    let known = ["your", "own", "other", "another", "character", "characters"];
    let classifications = s
        .split_whitespace()
        .filter(|w| !known.contains(&w.to_lowercase().as_str()))
        .map(Classification::new)
        .collect();
    Some(StaticTarget::OwnedCharacters {
        classifications,
        include_self,
    })
}
