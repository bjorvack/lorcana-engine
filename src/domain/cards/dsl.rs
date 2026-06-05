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
    Amount, CardCategory, CharacterFilter, Comparison, CountCondition, DeckPosition, Destination,
    DiscardAmount, DiscardBy, Effect, MoveSource, NumericFilter, PlayerScope, ScopedEvent,
    SourceZone, Target, TargetSide, TriggerCondition,
};
use crate::domain::game::{Condition, Property, Restriction, Stat};
use crate::domain::types::card::Classification;
use serde::Deserialize;
use std::cell::RefCell;
use toml::Value;

thread_local! {
    /// Classifications known to the current parse, set by the loader from the
    /// cards being loaded. Lets `parse_filter` match **multi-word**
    /// classifications (e.g. "Seven Dwarfs", or any a future set introduces)
    /// without hardcoding — derived from the data, scoped to a `load_toml*` call.
    /// Not game state; purely loader-time parse context.
    static KNOWN_CLASSIFICATIONS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` with `classes` registered as the parse's known classifications,
/// restoring the previous set afterwards. The loader wraps card parsing in this
/// so selectors can resolve multi-word classification names dynamically.
pub(crate) fn with_classifications<R>(classes: Vec<String>, f: impl FnOnce() -> R) -> R {
    let prev = KNOWN_CLASSIFICATIONS.with(|c| std::mem::replace(&mut *c.borrow_mut(), classes));
    let result = f();
    KNOWN_CLASSIFICATIONS.with(|c| *c.borrow_mut() = prev);
    result
}

/// One `[[card.abilities]]` table.
// A flat deserialization struct that mirrors the optional `[[card.abilities]]`
// keys; each boolean maps to an independent ability modifier, so the count is
// inherent rather than a state-machine smell.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Deserialize)]
pub struct TomlAbility {
    /// The trigger ("play", "quest", "banish", …).
    pub on: String,
    /// A classification that narrows the trigger's scope — "whenever you play a
    /// **Floodborn** character" / "whenever one of your **Illusion** characters
    /// quests". For `play_character` it threads into the watched
    /// [`CardCategory::Character`]; for a scoped per-character event it is
    /// AND-combined onto the [`CharacterFilter`] scope. Only valid on those
    /// triggers.
    #[serde(default)]
    pub classification: Option<String>,
    /// "You may …" — optional ability.
    #[serde(default)]
    pub may: bool,
    /// "During your turn, …" — the trigger only fires while its controller is the
    /// active player.
    #[serde(default)]
    pub during_your_turn: bool,
    /// "During the opponent's turn, …" — the trigger only fires while its
    /// controller is *not* the active player.
    #[serde(default)]
    pub during_opponents_turn: bool,
    /// "Once during your turn, …" — the trigger fires only the first matching
    /// time each turn; later matching events that turn do nothing.
    #[serde(default)]
    pub once_per_turn: bool,
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
        let mut condition = trigger_from(&self.on)?;
        if let Some(name) = &self.classification {
            condition = narrow_by_classification(condition, Classification::new(name), &self.on)?;
        }
        let effect = effect_from_value(&self.effect)?;
        let ability = if self.may {
            TriggeredAbility::optional(condition, effect)
        } else {
            TriggeredAbility::new(condition, effect)
        };
        if self.during_your_turn && self.during_opponents_turn {
            return Err("during_your_turn and during_opponents_turn are mutually exclusive".into());
        }
        let ability = if self.during_your_turn {
            ability.during_your_turn()
        } else if self.during_opponents_turn {
            ability.during_opponents_turn()
        } else {
            ability
        };
        Ok(if self.once_per_turn {
            ability.once_per_turn()
        } else {
            ability
        })
    }
}

/// Map a trigger name to a [`TriggerCondition`].
fn trigger_from(s: &str) -> Result<TriggerCondition, String> {
    use ScopedEvent::{
        Banished, BanishesInChallenge, Challenged, Challenges, DamageRemoved, DealtDamage,
        LeavesPlay, Quests, Readies, Sings,
    };
    // Scope helpers (the CharacterFilter algebra): "this" = IsSource; "one of your
    // characters" = Side(Yours); "one of your other characters" = Yours ∧ ¬IsSource;
    // "an opposing character" = Side(Opposing).
    let this = || CharacterFilter::IsSource;
    let yours = || CharacterFilter::Side(TargetSide::Yours);
    let yours_other = || {
        CharacterFilter::And(vec![
            CharacterFilter::Side(TargetSide::Yours),
            CharacterFilter::Not(Box::new(CharacterFilter::IsSource)),
        ])
    };
    let opposing = || CharacterFilter::Side(TargetSide::Opposing);
    let ev = |event: ScopedEvent, scope: CharacterFilter| TriggerCondition::WhenCharacterEvent {
        event,
        scope,
    };
    Ok(match s {
        "play" | "play_this" => TriggerCondition::WhenYouPlayThis,
        "play_with_shift" => TriggerCondition::WhenYouPlayThisWithShift,
        "play_action" => TriggerCondition::WhenYouPlay(CardCategory::Action),
        "play_song" => TriggerCondition::WhenYouPlay(CardCategory::Song),
        "play_character" => TriggerCondition::WhenYouPlay(CardCategory::Character(None)),
        "play_item" => TriggerCondition::WhenYouPlay(CardCategory::Item),
        "play_location" => TriggerCondition::WhenYouPlay(CardCategory::Location),
        "quest" => ev(Quests, this()),
        "yours_quests" | "your_character_quests" => ev(Quests, yours()),
        "challenge" => ev(Challenges, this()),
        "challenged" => ev(Challenged, this()),
        "banish" | "banished" => ev(
            Banished {
                requires_challenge: false,
            },
            this(),
        ),
        "yours_banished" | "your_other_character_banished" => ev(
            Banished {
                requires_challenge: false,
            },
            yours_other(),
        ),
        "banished_in_challenge" => ev(
            Banished {
                requires_challenge: true,
            },
            this(),
        ),
        "yours_banished_in_challenge" => ev(
            Banished {
                requires_challenge: true,
            },
            yours_other(),
        ),
        "banishes_in_challenge" => ev(BanishesInChallenge, this()),
        "start_of_turn" => TriggerCondition::AtStartOfTurn,
        "end_of_turn" => TriggerCondition::AtEndOfTurn,
        "dealt_damage" => ev(DealtDamage, this()),
        "opposing_dealt_damage" => ev(DealtDamage, opposing()),
        "damage_removed" => ev(DamageRemoved, this()),
        "readies" => ev(Readies, this()),
        "leaves_play" | "leave_play" => ev(LeavesPlay, this()),
        "sings" | "sings_song" | "sing_song" => ev(Sings, this()),
        "yours_sings" | "your_character_sings" => ev(Sings, yours()),
        "card_put_in_inkwell" => TriggerCondition::WhenCardPutInInkwell,
        "draw" | "you_draw" => TriggerCondition::WhenYouDraw,
        other => return Err(format!("unknown trigger {other:?}")),
    })
}

/// Narrow a trigger's scope by a classification, composing with the existing
/// algebra rather than adding a parallel trigger kind: a "whenever you play a
/// character" trigger gains the watched [`CardCategory::Character`]
/// classification ("a Floodborn character"), and a scoped per-character event
/// AND-combines [`CharacterFilter::Classification`] onto its scope ("one of your
/// Illusion characters quests"). Any other trigger has no classification slot, so
/// it is an authoring error.
fn narrow_by_classification(
    condition: TriggerCondition,
    class: Classification,
    on: &str,
) -> Result<TriggerCondition, String> {
    match condition {
        TriggerCondition::WhenYouPlay(CardCategory::Character(_)) => Ok(
            TriggerCondition::WhenYouPlay(CardCategory::Character(Some(class))),
        ),
        TriggerCondition::WhenCharacterEvent { event, scope } => {
            Ok(TriggerCondition::WhenCharacterEvent {
                event,
                scope: scope.and(CharacterFilter::Classification(class)),
            })
        }
        _ => Err(format!(
            "classification is not applicable to trigger {on:?}"
        )),
    }
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

    if let Some(steps) = t.get("then_to") {
        // `{ apply_to = "<selector>", then_to = [<effect>, ...] }` — resolve the
        // target once, then apply each sub-effect to it in order ([`Effect::OnTarget`]).
        let target = t
            .get("apply_to")
            .map_or(Ok(Target::SelfCard), target_from_value)?;
        let Value::Array(items) = steps else {
            return Err("`then_to` must be an array of effects".to_string());
        };
        let effects = items
            .iter()
            .map(effect_from_value)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Effect::OnTarget { target, effects });
    }
    if t.contains_key("draw") {
        let who = t.get("who").and_then(Value::as_str);
        Ok(Effect::Draw {
            who: if let Some(s) = who {
                scope_from_str(s).ok_or_else(|| format!("unknown scope {s:?}"))?
            } else {
                PlayerScope::You
            },
            amount: amt("draw")?,
        })
    } else if t.contains_key("gain_lore") {
        let who = t.get("who").and_then(Value::as_str);
        Ok(Effect::Lore {
            who: if let Some(s) = who {
                scope_from_str(s).ok_or_else(|| format!("unknown scope {s:?}"))?
            } else {
                PlayerScope::You
            },
            amount: amt("gain_lore")?,
        })
    } else if t.contains_key("lose_lore") {
        let who = t.get("who").and_then(Value::as_str);
        Ok(Effect::Lore {
            who: if let Some(s) = who {
                scope_from_str(s).ok_or_else(|| format!("unknown scope {s:?}"))?
            } else {
                PlayerScope::EachOpponent
            },
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
        Ok(Effect::GiveStatThisTurn {
            target: tgt()?,
            stat: Stat::Strength,
            amount: amt("give_strength")?,
        })
    } else if t.contains_key("give_lore") {
        Ok(Effect::GiveStatThisTurn {
            target: tgt()?,
            stat: Stat::Lore,
            amount: amt("give_lore")?,
        })
    } else if t.contains_key("give_willpower") {
        Ok(Effect::GiveStatThisTurn {
            target: tgt()?,
            stat: Stat::Willpower,
            amount: amt("give_willpower")?,
        })
    } else if let Some(v) = t.get("banish") {
        Ok(Effect::Banish(target_from_value(v)?))
    } else if let Some(v) = t.get("exert") {
        Ok(Effect::Exert(target_from_value(v)?))
    } else if t.contains_key("boost") {
        Ok(Effect::Boost {
            count: amt("boost")?,
        })
    } else if let Some(v) = t.get("ready") {
        Ok(Effect::Ready(target_from_value(v)?))
    } else if let Some(v) = t.get("freeze") {
        Ok(Effect::Freeze(target_from_value(v)?))
    } else if let Some(v) = t.get("return_to_hand") {
        Ok(Effect::Move {
            what: MoveSource::Card(target_from_value(v)?),
            to: Destination::Hand,
        })
    } else if let Some(Value::String(sel)) = t.get("return_from_discard") {
        // "Return a <selector> card from your discard to your hand" — `who`
        // defaults to You; the selector parses to a printed-predicate filter.
        let filter = parse_filter(sel).ok_or_else(|| format!("unparseable filter {sel:?}"))?;
        Ok(Effect::Move {
            what: MoveSource::ChosenFrom {
                zone: SourceZone::Discard,
                who: scope(PlayerScope::You)?,
                filter,
            },
            to: Destination::Hand,
        })
    } else if let Some(v) = t.get("into_inkwell") {
        Ok(Effect::Move {
            what: MoveSource::Card(target_from_value(v)?),
            to: Destination::Inkwell,
        })
    } else if let Some(Value::String(sel)) = t.get("inkwell_from_hand") {
        // "Put a <selector> card from your hand into your inkwell facedown & exerted."
        let filter = parse_filter(sel).ok_or_else(|| format!("unparseable filter {sel:?}"))?;
        Ok(Effect::Move {
            what: MoveSource::ChosenFrom {
                zone: SourceZone::Hand,
                who: scope(PlayerScope::You)?,
                filter,
            },
            to: Destination::Inkwell,
        })
    } else if t.contains_key("discard") {
        Ok(Effect::Discard {
            who: scope(PlayerScope::You)?,
            amount: DiscardAmount::Count(u32::try_from(int("discard")?).unwrap_or(0)),
            by: DiscardBy::Owner,
        })
    } else if let Some(Value::String(sel)) = t.get("play_free") {
        // "(You may) play a <selector> card from your hand for free" (§6).
        let filter = parse_filter(sel).ok_or_else(|| format!("unparseable filter {sel:?}"))?;
        Ok(Effect::PlayFreeFromHand { filter })
    } else if let Some(Value::String(cond)) = t.get("if_you_have") {
        let filter = parse_filter(cond).ok_or_else(|| format!("unparseable filter {cond:?}"))?;
        let then = t
            .get("then")
            .ok_or_else(|| "`if_you_have` needs a `then` effect".to_string())?;
        let at_least = t
            .get("at_least")
            .and_then(Value::as_integer)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(1);
        Ok(Effect::IfControl {
            filter,
            at_least,
            then: Box::new(effect_from_value(then)?),
        })
    } else if let Some(Value::String(cond)) = t.get("if_count") {
        // Count-based conditions: "if you have more than 3 cards in your hand", etc.
        let condition = parse_count_condition(cond)?;
        let then = t
            .get("then")
            .ok_or_else(|| "`if_count` needs a `then` effect".to_string())?;
        Ok(Effect::IfCount {
            condition,
            then: Box::new(effect_from_value(then)?),
        })
    } else if let Some(Value::String(kw)) = t.get("grant_keyword") {
        let keyword = keyword_from(kw).ok_or_else(|| format!("unknown keyword {kw:?}"))?;
        let property = Property::Keyword(keyword);
        let target = tgt()?;
        // `duration = "permanent"` ("gains X") vs the default this-turn ("gains X
        // this turn").
        match t.get("duration").and_then(Value::as_str) {
            Some("permanent") => Ok(Effect::Grant { target, property }),
            Some("next_turn") => Ok(Effect::GrantNextTurn { target, property }),
            Some("this_turn") | None => Ok(Effect::GrantThisTurn { target, property }),
            Some(other) => Err(format!("unknown grant duration {other:?}")),
        }
    } else if t.contains_key("look_at_top") {
        // "Look at the top N cards. You may take up to M <filter> to your hand. Put the
        // rest on the bottom/top, or shuffle. You may reorder before taking."
        // `take` can be a string filter or an integer count; defaults to 1 any card.
        // `rest` defaults to bottom; `who` defaults to the controller's own deck.
        // `reorder` defaults to false.
        let count = u32::try_from(int("look_at_top")?).unwrap_or(0);
        // Handle `take` as either a filter string or an integer count
        let (take_count, filter) = match t.get("take") {
            Some(Value::String(s)) => {
                // `take = "a character"` -> filter, default take_count = 1
                let f = parse_filter(s).unwrap_or_else(|| CharacterFilter::any(TargetSide::Any));
                (1, f)
            }
            Some(Value::Integer(n)) => {
                // `take = 2` -> take count, default filter = any
                (
                    u32::try_from(*n).unwrap_or(1),
                    CharacterFilter::any(TargetSide::Any),
                )
            }
            None => (1, CharacterFilter::any(TargetSide::Any)),
            Some(other) => {
                return Err(format!(
                    "expected `take` to be a string or integer, got {other}"
                ));
            }
        };
        // Override take_count if explicitly set
        let take_count = t
            .get("take_count")
            .and_then(Value::as_integer)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(take_count);
        let rest = match t.get("rest").and_then(Value::as_str) {
            Some("top") => DeckPosition::Top,
            Some("shuffle") => DeckPosition::Shuffle,
            Some("bottom") | None => DeckPosition::Bottom,
            Some(other) => return Err(format!("unknown rest position {other:?}")),
        };
        let reorder = t.get("reorder").and_then(Value::as_bool).unwrap_or(false);
        // Parse rest_per_card if present (for split top/bottom effects like Dr. Facilier)
        let rest_per_card = if let Some(Value::Array(arr)) = t.get("rest_per_card") {
            let positions: Result<Vec<DeckPosition>, String> = arr
                .iter()
                .map(|v| {
                    if let Value::String(s) = v {
                        match s.as_str() {
                            "top" => Ok(DeckPosition::Top),
                            "bottom" => Ok(DeckPosition::Bottom),
                            other => Err(format!("unknown rest_per_card position {other:?}")),
                        }
                    } else {
                        Err("rest_per_card must be an array of strings".to_string())
                    }
                })
                .collect();
            Some(positions?)
        } else {
            None
        };
        Ok(Effect::LookAtTopAndTake {
            whose: scope(PlayerScope::You)?,
            count,
            take_count,
            filter,
            rest,
            reorder,
            rest_per_card,
        })
    } else if t.contains_key("search") {
        // "Search your deck for up to N <filter> and take them into hand, then shuffle."
        // `search` is the filter; `take` is the count (defaults to 1).
        let filter_str = t
            .get("search")
            .and_then(Value::as_str)
            .ok_or_else(|| "`search` needs a filter string".to_string())?;
        let filter = parse_filter(filter_str)
            .ok_or_else(|| format!("unparseable search filter {filter_str:?}"))?;
        let take_count = t
            .get("take")
            .and_then(Value::as_integer)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(1);
        Ok(Effect::SearchDeckAndTake {
            whose: scope(PlayerScope::You)?,
            take_count,
            filter,
        })
    } else if t.contains_key("move_damage") {
        // "Move up to N damage from <a> to <b>."
        let from = t
            .get("from")
            .ok_or_else(|| "`move_damage` needs a `from` target".to_string())?;
        let to = t
            .get("to")
            .ok_or_else(|| "`move_damage` needs a `to` target".to_string())?;
        Ok(Effect::MoveDamage {
            from: target_from_value(from)?,
            to: target_from_value(to)?,
            amount: amt("move_damage")?,
        })
    } else if let Some(Value::String(r)) = t.get("restrict") {
        // Grant a restriction ("can't quest/challenge/…"), this turn or permanently.
        let property = Property::Restriction(restriction_from(r)?);
        let target = tgt()?;
        match t.get("duration").and_then(Value::as_str) {
            Some("permanent") => Ok(Effect::Grant { target, property }),
            Some("next_turn") => Ok(Effect::GrantNextTurn { target, property }),
            Some("this_turn") | None => Ok(Effect::GrantThisTurn { target, property }),
            Some(other) => Err(format!("unknown grant duration {other:?}")),
        }
    } else if t.contains_key("choose_one") {
        // "Choose one: [A] • [B] • [C]" — pick one effect to resolve.
        // `choose_one` is an array of effect tables; `optional` defaults to false.
        let optional = t.get("optional").and_then(Value::as_bool).unwrap_or(false);
        let options_array = t.get("choose_one").ok_or("`choose_one` needs an array")?;
        let options: Vec<_> = if let Value::Array(arr) = options_array {
            arr.iter()
                .map(effect_from_value)
                .collect::<Result<_, _>>()?
        } else {
            return Err(format!(
                "`choose_one` must be an array, got {options_array:?}"
            ));
        };
        if options.len() < 2 {
            return Err(format!(
                "`choose_one` needs at least 2 options, got {}",
                options.len()
            ));
        }
        Ok(Effect::ChooseOne { options, optional })
    } else if t.contains_key("may_choose_one") {
        // "You may choose one: [A] • [B]" — optional variant.
        let options_array = t
            .get("may_choose_one")
            .ok_or("`may_choose_one` needs an array")?;
        let options: Vec<_> = if let Value::Array(arr) = options_array {
            arr.iter()
                .map(effect_from_value)
                .collect::<Result<_, _>>()?
        } else {
            return Err(format!(
                "`may_choose_one` must be an array, got {options_array:?}"
            ));
        };
        if options.len() < 2 {
            return Err(format!(
                "`may_choose_one` needs at least 2 options, got {}",
                options.len()
            ));
        }
        Ok(Effect::ChooseOne {
            options,
            optional: true,
        })
    } else {
        Err(format!("no known effect verb in {t:?}"))
    }
}

/// Map a restriction name to a [`Restriction`].
fn restriction_from(s: &str) -> Result<Restriction, String> {
    Ok(match s {
        "cant_quest" => Restriction::CantQuest,
        "cant_challenge" => Restriction::CantChallenge,
        "cant_be_challenged" => Restriction::CantBeChallenged,
        "cant_be_chosen" => Restriction::CantBeChosen,
        "cant_ready" => Restriction::CantReady,
        "takes_no_challenge_damage" => Restriction::TakesNoChallengeDamage,
        other => return Err(format!("unknown restriction {other:?}")),
    })
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
    // "the challenging / challenged character" — the other combatant bound by a
    // challenge trigger (substituted at the firing site).
    if lower.contains("challenging character") || lower.contains("challenged character") {
        return Some(Target::TriggerCard);
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

/// Words the filter grammar treats as structure (side / count / category / the
/// threshold & name vocabulary) rather than as a classification name.
const STRUCTURAL: &[&str] = &[
    "all",
    "chosen",
    "another",
    "other",
    "opposing",
    "your",
    "yours",
    "mine",
    "of",
    "own",
    "character",
    "characters",
    "item",
    "items",
    "location",
    "locations",
    "permanent",
    "permanents",
    "action",
    "actions",
    "song",
    "songs",
    "card",
    "cards",
    // name + numeric-threshold vocabulary
    "named",
    "with",
    "and",
    "or",
    "than",
    "less",
    "fewer",
    "more",
    "greater",
    "cost",
    "strength",
    "willpower",
    "lore",
    "{s}",
    "{w}",
    "{l}",
];

/// The four numeric characteristics the compact filter grammar can threshold.
#[derive(Clone, Copy)]
enum NumericKind {
    Cost,
    Strength,
    Willpower,
    Lore,
}

/// Map a stat keyword — a word or a `{S}`/`{W}`/`{L}` symbol — to its kind.
fn numeric_kind(tok: &str) -> Option<NumericKind> {
    Some(match tok {
        "cost" => NumericKind::Cost,
        "strength" | "{s}" => NumericKind::Strength,
        "willpower" | "{w}" => NumericKind::Willpower,
        "lore" | "{l}" => NumericKind::Lore,
        _ => return None,
    })
}

/// The token at `idx` parsed as a non-negative threshold value, if present.
fn token_u32(tokens: &[String], idx: usize) -> Option<u32> {
    tokens.get(idx).and_then(|t| t.parse::<u32>().ok())
}

/// The comparison implied by the words after a `<stat> <N>` clause: `or less` /
/// `or fewer` ⇒ at-most, `or more` / `or greater` ⇒ at-least, absent ⇒ exactly N.
/// Stops at the next stat keyword so adjacent clauses don't bleed together.
fn comparison_after(tokens: &[String], from: usize) -> Comparison {
    for tok in tokens.iter().skip(from) {
        if numeric_kind(tok).is_some() {
            break;
        }
        match tok.as_str() {
            "less" | "fewer" => return Comparison::AtMost,
            "more" | "greater" => return Comparison::AtLeast,
            _ => {}
        }
    }
    Comparison::Exactly
}

/// Push state predicates ("damaged"/"exerted"/"ready") and the one multi-word
/// classification ("Seven Dwarfs"), marking their tokens consumed so the leftover
/// pass doesn't misread them as single-word classifications.
fn push_state_and_multiword_predicates(
    lower_tokens: &[String],
    consumed: &mut [bool],
    predicates: &mut Vec<CharacterFilter>,
) {
    for i in 0..lower_tokens.len() {
        let predicate = match lower_tokens[i].as_str() {
            "damaged" => CharacterFilter::Damaged(true),
            "exerted" => CharacterFilter::Exerted(true),
            "ready" => CharacterFilter::Exerted(false),
            _ => continue,
        };
        predicates.push(predicate);
        consumed[i] = true;
    }
    // Multi-word classifications (e.g. "Seven Dwarfs") known to the current parse:
    // greedily match a run of consecutive unconsumed tokens that forms one, so the
    // whitespace tokenizer doesn't split it into bogus single-word classifications.
    KNOWN_CLASSIFICATIONS.with(|vocab| {
        for class in vocab.borrow().iter() {
            let parts: Vec<String> = class.split_whitespace().map(str::to_lowercase).collect();
            if parts.len() < 2 {
                continue;
            }
            for start in 0..=lower_tokens.len().saturating_sub(parts.len()) {
                let matches = parts
                    .iter()
                    .enumerate()
                    .all(|(k, p)| !consumed[start + k] && &lower_tokens[start + k] == p);
                if matches {
                    predicates.push(CharacterFilter::Classification(Classification::new(
                        class.clone(),
                    )));
                    for k in 0..parts.len() {
                        consumed[start + k] = true;
                    }
                }
            }
        }
    });
}

/// Push card type predicates (action/song/character/item/location) into the
/// predicates list, marking their tokens consumed. This handles cases where the
/// category appears in a position that wouldn't be caught by the early detection.
fn push_card_type_predicates(
    lower_tokens: &[String],
    consumed: &mut [bool],
    predicates: &mut Vec<CharacterFilter>,
) {
    for i in 0..lower_tokens.len() {
        if consumed[i] {
            continue;
        }
        let predicate = match lower_tokens[i].as_str() {
            "action" => Some(CharacterFilter::Category(CardCategory::Action)),
            "song" => Some(CharacterFilter::Category(CardCategory::Song)),
            "item" => Some(CharacterFilter::Category(CardCategory::Item)),
            "location" => Some(CharacterFilter::Category(CardCategory::Location)),
            _ => None,
        };
        if let Some(p) = predicate {
            predicates.push(p);
            consumed[i] = true;
        }
    }
}

/// Parse a compact card filter. Combines side + category + classifications +
/// `another` with the richer leaf predicates: a name (`named X`), state
/// (`damaged`/`exerted`), and numeric thresholds on cost / `{S}` / `{W}` / `{L}`
/// (`"with cost 3 or less"`, `"with 3 {S} or more"`). These compose, e.g.
/// `"another Villain character with cost 3 or less"`. Requires a noun so it's
/// distinguishable from free text. Anything the grammar can't express still
/// round-trips via the structured AST fallback.
#[allow(clippy::too_many_lines)]
fn parse_filter(s: &str) -> Option<CharacterFilter> {
    let lower = s.to_lowercase();
    if ![
        "character",
        "item",
        "location",
        "permanent",
        "action",
        "song",
        "card",
    ]
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
    } else if lower.contains("song") {
        Some(CardCategory::Song)
    } else if lower.contains("action") && !lower.contains("song") {
        Some(CardCategory::Action)
    } else {
        None // characters (the default) need no Category gate
    };
    // "a character or item" / "an item or location" — an OR of category leaves
    // (only when "or" actually joins ≥2 categories, so single-category selectors
    // keep their existing single-`Category` shape). Includes the character category.
    let category_or: Option<CharacterFilter> = lower
        .contains(" or ")
        .then(|| {
            let mut cats = Vec::new();
            if lower.contains("character") {
                cats.push(CharacterFilter::Category(CardCategory::Character(None)));
            }
            if lower.contains("item") {
                cats.push(CharacterFilter::Category(CardCategory::Item));
            }
            if lower.contains("location") {
                cats.push(CharacterFilter::Category(CardCategory::Location));
            }
            cats
        })
        .filter(|cats| cats.len() >= 2)
        .map(CharacterFilter::Or);

    // Tokenize once: original-case (for classification names) alongside a
    // lowercased view, with a `consumed` flag so phrases lifted out as predicates
    // (names, thresholds) aren't also misread as classifications.
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let lower_tokens: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
    let mut consumed = vec![false; tokens.len()];
    let mut predicates: Vec<CharacterFilter> = Vec::new();

    // `named X`: capture the (possibly multi-word) name following "named" — all
    // tokens up to the next structural/numeric word (e.g. "named Peter Pan",
    // "named Mr. Smee"). Matches the card's counts-as names (§6.2.1).
    for i in 0..tokens.len() {
        if lower_tokens[i] != "named" || consumed[i] {
            continue;
        }
        consumed[i] = true;
        let mut parts = Vec::new();
        let mut j = i + 1;
        while j < tokens.len()
            && !STRUCTURAL.contains(&lower_tokens[j].as_str())
            && token_u32(&lower_tokens, j).is_none()
        {
            parts.push(tokens[j]);
            consumed[j] = true;
            j += 1;
        }
        if !parts.is_empty() {
            predicates.push(CharacterFilter::Named(parts.join(" ")));
        }
    }

    push_state_and_multiword_predicates(&lower_tokens, &mut consumed, &mut predicates);
    push_card_type_predicates(&lower_tokens, &mut consumed, &mut predicates);

    // Numeric thresholds: a stat keyword adjacent to an integer, optionally
    // followed by `or less` / `or more` (else exactly N), §7.1.
    for i in 0..lower_tokens.len() {
        if consumed[i] {
            continue;
        }
        let Some(kind) = numeric_kind(&lower_tokens[i]) else {
            continue;
        };
        // The threshold value is the integer right after the keyword ("cost 3")
        // or right before it ("3 {S}").
        let before = i.checked_sub(1).and_then(|j| token_u32(&lower_tokens, j));
        let (value, value_idx) = match (token_u32(&lower_tokens, i + 1), before) {
            (Some(n), _) => (n, i + 1),
            (None, Some(n)) => (n, i - 1),
            (None, None) => continue,
        };
        let comparison = comparison_after(&lower_tokens, value_idx.max(i) + 1);
        let nf = NumericFilter { comparison, value };
        predicates.push(match kind {
            NumericKind::Cost => CharacterFilter::Cost(nf),
            NumericKind::Strength => CharacterFilter::Strength(nf),
            NumericKind::Willpower => CharacterFilter::Willpower(nf),
            NumericKind::Lore => CharacterFilter::Lore(nf),
        });
        consumed[i] = true;
        consumed[value_idx] = true;
    }

    let mut filter = CharacterFilter::any(side);
    if let Some(or) = category_or {
        // The OR of categories replaces single-category gating: drop the per-token
        // `Category` leaves `push_card_type_predicates` added (they'd AND-narrow it).
        predicates.retain(|p| !matches!(p, CharacterFilter::Category(_)));
        filter = filter.and(or);
    } else if let Some(cat) = category {
        filter = filter.and(CharacterFilter::Category(cat));
    }
    for predicate in predicates {
        filter = filter.and(predicate);
    }
    for (i, word) in tokens.iter().enumerate() {
        let lw = lower_tokens[i].as_str();
        if consumed[i] || STRUCTURAL.contains(&lw) || token_u32(&lower_tokens, i).is_some() {
            continue;
        }
        filter = filter.and(CharacterFilter::Classification(Classification::new(*word)));
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
    // "that much" / "that many" / "damage dealt" — the amount the trigger carries.
    if matches!(
        lower.as_str(),
        "that much" | "that many" | "damage dealt" | "damage_dealt"
    ) {
        return Some(Amount::TriggerAmount);
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
    /// Whether activating banishes the source ("Banish this item — …").
    #[serde(default)]
    pub banish: bool,
}

impl TomlActivated {
    /// Build the [`ActivatedAbility`].
    ///
    /// # Errors
    /// Returns a detail string if the effect can't be parsed.
    pub fn to_ability(&self) -> Result<ActivatedAbility, String> {
        let mut cost = AbilityCost::new(self.cost.exert, self.cost.ink);
        if self.cost.banish {
            cost = cost.banishing_self();
        }
        Ok(ActivatedAbility::new(
            cost,
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
                // The full Amount vocabulary ("cards in hand", "damage on self",
                // "<stat> of self", "per <filter>"), falling back to a bare filter
                // string ("another Villain") as for-each-character.
                amount_from_str(p)
                    .or_else(|| parse_filter(p).map(Amount::PerMatchingCharacter))
                    .ok_or_else(|| format!("unparseable `per` amount {p:?}"))
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

/// Parse a count-based condition string like "more than 3 cards in your hand".
fn parse_count_condition(s: &str) -> Result<CountCondition, String> {
    use crate::domain::effects::CountCondition;
    let s = s.to_lowercase();

    // Parse patterns like:
    // - "3 or more cards in your hand" -> HandSizeAtLeast(3)
    // - "more than 3 cards in your hand" -> HandSizeMoreThan(3)
    // - "3 or more lore" -> LoreAtLeast(3)
    // - "more than 3 lore" -> LoreMoreThan(3)
    // - "more lore than opponent" -> LoreMoreThanOpponent

    if s.contains("cards in your hand") || s.contains("cards in hand") {
        let n = extract_number(&s)?;
        if s.contains("or more") {
            Ok(CountCondition::HandSizeAtLeast(n))
        } else if s.contains("more than") {
            Ok(CountCondition::HandSizeMoreThan(n))
        } else {
            Err(format!("unknown hand size condition: {s}"))
        }
    } else if s.contains("lore") {
        if s.contains("than opponent") {
            Ok(CountCondition::LoreMoreThanOpponent)
        } else {
            let n = extract_number(&s)?;
            if s.contains("or more") {
                Ok(CountCondition::LoreAtLeast(n))
            } else if s.contains("more than") {
                Ok(CountCondition::LoreMoreThan(n))
            } else {
                Err(format!("unknown lore condition: {s}"))
            }
        }
    } else {
        Err(format!("unknown count condition: {s}"))
    }
}

/// Extract the first number from a string.
fn extract_number(s: &str) -> Result<u32, String> {
    s.split_whitespace()
        .find_map(|word| word.parse::<u32>().ok())
        .ok_or_else(|| format!("no number found in {s}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_count_condition, parse_filter, parse_selector, with_classifications};
    use crate::domain::effects::{
        CharacterFilter, Comparison, CountCondition, NumericFilter, Target, TargetSide,
    };
    use crate::domain::types::card::Classification;

    const fn at_least(value: u32) -> NumericFilter {
        NumericFilter::at_least(value)
    }
    const fn at_most(value: u32) -> NumericFilter {
        NumericFilter::at_most(value)
    }
    const fn exactly(value: u32) -> NumericFilter {
        NumericFilter {
            comparison: Comparison::Exactly,
            value,
        }
    }

    #[test]
    fn parses_named_predicate() {
        assert_eq!(
            parse_filter("character named Stitch"),
            Some(
                CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Named("Stitch".into()))
            )
        );
        // Composes with side + `chosen` framing via the selector.
        assert_eq!(
            parse_selector("chosen opposing character named Stitch"),
            Some(Target::ChosenCharacter {
                filter: CharacterFilter::any(TargetSide::Opposing)
                    .and(CharacterFilter::Named("Stitch".into())),
            })
        );
    }

    #[test]
    fn parses_cost_thresholds() {
        assert_eq!(
            parse_filter("character with cost 3 or less"),
            Some(CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Cost(at_most(3))))
        );
        assert_eq!(
            parse_filter("character with cost 4 or more"),
            Some(CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Cost(at_least(4))))
        );
        // No comparison words means exactly N.
        assert_eq!(
            parse_filter("character with cost 2"),
            Some(CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Cost(exactly(2))))
        );
    }

    #[test]
    fn parses_strength_thresholds_word_and_symbol() {
        let expected =
            CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Strength(at_least(3)));
        // `{S}` symbol with the number before it.
        assert_eq!(
            parse_filter("character with 3 {S} or more"),
            Some(expected.clone())
        );
        // The `strength` word with the number after it.
        assert_eq!(
            parse_filter("character with strength 3 or more"),
            Some(expected)
        );
        assert_eq!(
            parse_filter("character with 2 strength or less"),
            Some(CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Strength(at_most(2))))
        );
    }

    #[test]
    fn parses_willpower_and_lore_thresholds() {
        assert_eq!(
            parse_filter("character with 4 {W} or more"),
            Some(
                CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Willpower(at_least(4)))
            )
        );
        assert_eq!(
            parse_filter("character with willpower 2 or less"),
            Some(CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Willpower(at_most(2))))
        );
        assert_eq!(
            parse_filter("character with 2 {L} or more"),
            Some(CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Lore(at_least(2))))
        );
        assert_eq!(
            parse_filter("character with lore 1 or more"),
            Some(CharacterFilter::any(TargetSide::Any).and(CharacterFilter::Lore(at_least(1))))
        );
    }

    #[test]
    fn predicates_compose_with_side_classification_and_another() {
        // "another Villain character with cost 3 or less".
        let expected = CharacterFilter::any(TargetSide::Any)
            .and(CharacterFilter::Cost(at_most(3)))
            .and(CharacterFilter::Classification(Classification::new(
                "Villain",
            )))
            .and(CharacterFilter::negate(CharacterFilter::IsSource));
        assert_eq!(
            parse_filter("another Villain character with cost 3 or less"),
            Some(expected)
        );
    }

    #[test]
    fn existing_classification_filters_still_parse() {
        // Regression: the original grammar (no new predicates) is unchanged.
        let villains = CharacterFilter::any(TargetSide::Yours)
            .and(CharacterFilter::Classification(Classification::new(
                "Villain",
            )))
            .and(CharacterFilter::negate(CharacterFilter::IsSource));
        assert_eq!(
            parse_filter("your other Villain characters"),
            Some(villains)
        );
        assert_eq!(
            parse_filter("chosen opposing character"),
            Some(CharacterFilter::any(TargetSide::Opposing))
        );
    }

    #[test]
    fn parses_state_multiword_and_of_yours_without_bogus_classifications() {
        // "damaged" / "exerted" -> boolean predicates, not classifications.
        let damaged = format!("{:?}", parse_filter("your damaged characters").unwrap());
        assert!(damaged.contains("Damaged(true)"), "{damaged}");
        assert!(!damaged.contains("Classification"), "{damaged}");

        // "of yours" -> side Yours with no leftover "of"/"yours" classifications.
        let of_yours = format!("{:?}", parse_filter("chosen character of yours").unwrap());
        assert!(of_yours.contains("Yours"), "{of_yours}");
        assert!(!of_yours.contains("Classification"), "{of_yours}");

        // A multi-word classification known to the parse ("Seven Dwarfs", or any a
        // future set adds) is matched as one classification, not split into bogus
        // tokens — driven by the registered vocabulary, nothing hardcoded.
        let dwarfs = with_classifications(vec!["Seven Dwarfs".to_string()], || {
            format!(
                "{:?}",
                parse_filter("your other Seven Dwarfs characters").unwrap()
            )
        });
        assert!(dwarfs.contains("Seven Dwarfs"), "{dwarfs}");
        assert!(!dwarfs.contains("\"Seven\""), "{dwarfs}");
    }

    #[test]
    fn parses_multiword_named() {
        let f = format!(
            "{:?}",
            parse_filter("chosen character named Peter Pan").unwrap()
        );
        assert!(f.contains("Named(\"Peter Pan\")"), "{f}");
        // multi-word name followed by a structural word stops correctly
        let g = format!(
            "{:?}",
            parse_filter("your characters named Peter Pan").unwrap()
        );
        assert!(g.contains("Named(\"Peter Pan\")"), "{g}");
    }

    #[test]
    fn parses_card_type_predicates() {
        // "an action card" should parse with Category(Action)
        let action = format!("{:?}", parse_filter("an action card").unwrap());
        assert!(action.contains("Category(Action)"), "{action}");

        // "a song" should parse with Category(Song)
        let song = format!("{:?}", parse_filter("a song").unwrap());
        assert!(song.contains("Category(Song)"), "{song}");

        // "an item" should parse with Category(Item)
        let item = format!("{:?}", parse_filter("an item").unwrap());
        assert!(item.contains("Category(Item)"), "{item}");

        // "a location" should parse with Category(Location)
        let location = format!("{:?}", parse_filter("a location").unwrap());
        assert!(location.contains("Category(Location)"), "{location}");

        // "your action cards" should combine side and category
        let your_action = format!("{:?}", parse_filter("your action cards").unwrap());
        assert!(your_action.contains("Category(Action)"), "{your_action}");
        assert!(your_action.contains("Yours"), "{your_action}");
    }

    #[test]
    fn parses_count_conditions() {
        assert_eq!(
            parse_count_condition("3 or more cards in your hand"),
            Ok(CountCondition::HandSizeAtLeast(3))
        );
        assert_eq!(
            parse_count_condition("more than 3 cards in hand"),
            Ok(CountCondition::HandSizeMoreThan(3))
        );
        assert_eq!(
            parse_count_condition("3 or more lore"),
            Ok(CountCondition::LoreAtLeast(3))
        );
        assert_eq!(
            parse_count_condition("more than 3 lore"),
            Ok(CountCondition::LoreMoreThan(3))
        );
        assert_eq!(
            parse_count_condition("more lore than opponent"),
            Ok(CountCondition::LoreMoreThanOpponent)
        );
    }

    #[test]
    fn parse_count_condition_invalid() {
        assert!(parse_count_condition("invalid condition").is_err());
        assert!(parse_count_condition("").is_err());
    }
}
