//! Integration tests for Slice 5c: the continuous-effects layer — current stats
//! combine the printed base with active modifiers and clamp at the point of use
//! while retaining the true total (§7.8).

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, Classification,
    Condition, Conditions, GameState, GameStatus, Input, ModifierDuration, ModifierTarget,
    PlayerId, Property, Restriction, Stat, StatModifier, StaticAbility, StaticEffect, StaticTarget,
    apply, start,
};

fn started_with(registry: &CardRegistry) -> GameState {
    let mut state = GameState::new(
        vec![
            (0..30).map(CardDefId::from_raw).collect(),
            (0..30).map(CardDefId::from_raw).collect(),
        ],
        7,
    );
    let _ = start(&mut state).expect("start");
    while let GameStatus::AwaitingMulligan(player) = *state.status() {
        let _ = apply(
            &mut state,
            registry,
            Input::Mulligan {
                player,
                put_back: Vec::new(),
            },
        )
        .expect("mulligan");
    }
    state
}

fn started() -> GameState {
    started_with(&CardRegistry::new())
}

fn place_character(state: &mut GameState, owner: PlayerId, raw: u32, strength: u32) -> CardId {
    let id = CardId::from_raw(raw);
    let mut instance = CardInstance::new(
        id,
        CardDefId::from_raw(raw),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(strength, 3, 1)));
    state.player_mut(owner).unwrap().play_mut().push(instance);
    id
}

const fn modifier(source: u32, target: CardId, delta: i32) -> StatModifier {
    StatModifier::new(
        CardId::from_raw(source),
        ModifierTarget::Card(target),
        Stat::Strength,
        delta,
        ModifierDuration::WhileSourceInPlay,
    )
}

#[test]
fn no_modifiers_means_current_equals_base() {
    let mut state = started();
    let active = state.active_player();
    let card = place_character(&mut state, active, 2000, 2);
    assert_eq!(state.current_character_stats(card).unwrap().strength, 2);
}

#[test]
fn modifiers_combine_and_clamp_to_zero_at_use() {
    let mut state = started();
    let active = state.active_player();
    let card = place_character(&mut state, active, 2000, 2);

    state.add_modifier(modifier(9001, card, 1));
    assert_eq!(state.current_character_stats(card).unwrap().strength, 3);

    // +1 then -5 = true total -2, used as 0 (§7.8.2).
    state.add_modifier(modifier(9002, card, -5));
    assert_eq!(state.current_character_stats(card).unwrap().strength, 0);

    // A further +1 combines with the true total (-1), still used as 0.
    state.add_modifier(modifier(9003, card, 1));
    assert_eq!(state.current_character_stats(card).unwrap().strength, 0);
}

#[test]
fn modifiers_end_when_their_source_is_removed() {
    let mut state = started();
    let active = state.active_player();
    let card = place_character(&mut state, active, 2000, 2);

    state.add_modifier(modifier(9001, card, 3));
    assert_eq!(state.current_character_stats(card).unwrap().strength, 5);

    state.remove_modifiers_from_source(CardId::from_raw(9001));
    assert_eq!(state.current_character_stats(card).unwrap().strength, 2);
}

#[test]
fn self_static_modifier_applies_when_played() {
    // Every card is a base-2-strength character with "this character gets +2 {S}".
    let registry: CardRegistry = (0..30)
        .map(|n| {
            CardDefinition::character(CardDefId::from_raw(n), 1, true, 2, 3, 1)
                .with_static(vec![StaticAbility::self_modifier(Stat::Strength, 2)])
        })
        .collect();
    let mut state = started_with(&registry);
    let active = state.active_player();

    let hand: Vec<CardId> = state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .map(CardInstance::id)
        .collect();
    let _ = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: hand[0] },
    )
    .expect("ink");
    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: hand[1],
            shift_onto: None,
        },
    )
    .expect("play");

    // Base 2 + static 2 = current 4 the moment it enters play (§7.6.2).
    assert_eq!(state.current_character_stats(hand[1]).unwrap().strength, 4);
}

/// A `StaticEffect::Grant` static applies a continuous restriction to its targets
/// while in play (§7.6) — here "this character can't be challenged".
#[test]
fn a_grant_static_applies_a_continuous_restriction() {
    let registry: CardRegistry = (0..30)
        .map(|n| {
            CardDefinition::character(CardDefId::from_raw(n), 1, true, 2, 3, 1).with_static(vec![
                StaticAbility {
                    target: StaticTarget::SelfCard,
                    effect: StaticEffect::Grant(Property::Restriction(
                        Restriction::CantBeChallenged,
                    )),
                    condition: None,
                },
            ])
        })
        .collect();
    let mut state = started_with(&registry);
    let active = state.active_player();
    let hand: Vec<CardId> = state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .map(CardInstance::id)
        .collect();
    let _ = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: hand[0] },
    )
    .expect("ink");
    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: hand[1],
            shift_onto: None,
        },
    )
    .expect("play");

    assert!(
        state.has_restriction(hand[1], Restriction::CantBeChallenged),
        "the played character has the continuously-granted restriction"
    );
}

/// Place an in-play character with the given strength and classifications.
fn place_classed(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    strength: u32,
    classes: &[&str],
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut instance = CardInstance::new(
        id,
        CardDefId::from_raw(raw),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(strength, 3, 1)));
    instance.set_classifications(classes.iter().map(|c| Classification::new(*c)).collect());
    state.player_mut(owner).unwrap().play_mut().push(instance);
    id
}

#[test]
fn until_end_of_turn_modifiers_expire_at_end_of_turn() {
    let mut state = started();
    let active = state.active_player();
    let card = place_character(&mut state, active, 2000, 2);

    state.add_modifier(StatModifier::new(
        CardId::from_raw(999),
        ModifierTarget::Card(card),
        Stat::Strength,
        2,
        ModifierDuration::UntilEndOfTurn,
    ));
    assert_eq!(state.current_character_stats(card).unwrap().strength, 4);

    let registry = CardRegistry::new();
    let _ = apply(&mut state, &registry, Input::EndTurn).expect("end turn");

    // The "this turn" modifier has expired (§7.6.1).
    assert_eq!(state.current_character_stats(card).unwrap().strength, 2);
}

#[test]
fn selector_static_buffs_only_matching_owned_characters() {
    let mut state = started();
    let active = state.active_player();
    let foe = state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != active)
        .unwrap();

    let villain = place_classed(&mut state, active, 200, 2, &["Villain"]);
    let hero = place_classed(&mut state, active, 201, 2, &["Hero"]);
    let foe_villain = place_classed(&mut state, foe, 300, 2, &["Villain"]);

    // "Your Villain characters get +2 {S}."
    state.add_modifier(StatModifier::new(
        CardId::from_raw(999),
        ModifierTarget::OwnedCharacters {
            owner: active,
            classifications: vec![Classification::new("Villain")],
            except: None,
        },
        Stat::Strength,
        2,
        ModifierDuration::WhileSourceInPlay,
    ));

    assert_eq!(state.current_character_stats(villain).unwrap().strength, 4);
    assert_eq!(
        state.current_character_stats(hero).unwrap().strength,
        2,
        "wrong classification"
    );
    assert_eq!(
        state.current_character_stats(foe_villain).unwrap().strength,
        2,
        "opponent's characters are not 'your' characters"
    );

    // A Villain that enters later is also affected (§7.6.2, dynamic set).
    let late = place_classed(&mut state, active, 202, 2, &["Villain"]);
    assert_eq!(state.current_character_stats(late).unwrap().strength, 4);
}

#[test]
fn selector_static_can_exclude_the_source() {
    // Two characters, each with "your OTHER characters get +1 {S}", buff each
    // other but not themselves, played through the normal flow (cost 0).
    let registry: CardRegistry = (0..30)
        .map(|n| {
            CardDefinition::character(CardDefId::from_raw(n), 0, true, 2, 3, 1).with_static(vec![
                StaticAbility::owned_characters(Vec::new(), false, Stat::Strength, 1),
            ])
        })
        .collect();
    let mut state = started_with(&registry);
    let active = state.active_player();
    let hand: Vec<CardId> = state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .map(CardInstance::id)
        .collect();

    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: hand[0],
            shift_onto: None,
        },
    )
    .expect("play 1");
    // Only itself in play and it excludes itself → no buff yet.
    assert_eq!(state.current_character_stats(hand[0]).unwrap().strength, 2);

    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: hand[1],
            shift_onto: None,
        },
    )
    .expect("play 2");
    // Each buffs the other (not itself): both at base 2 + 1 = 3.
    assert_eq!(state.current_character_stats(hand[0]).unwrap().strength, 3);
    assert_eq!(state.current_character_stats(hand[1]).unwrap().strength, 3);
}

#[test]
fn a_while_exerted_static_applies_only_while_the_source_is_exerted() {
    // Base-2 characters with "while this character is exerted, it gets +2 {S}".
    let registry: CardRegistry = (0..30)
        .map(|n| {
            CardDefinition::character(CardDefId::from_raw(n), 1, true, 2, 3, 1).with_static(vec![
                StaticAbility::self_modifier(Stat::Strength, 2)
                    .with_condition(Condition::SourceExerted),
            ])
        })
        .collect();
    let mut state = started_with(&registry);
    let active = state.active_player();
    let hand: Vec<CardId> = state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .map(CardInstance::id)
        .collect();
    let _ = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: hand[0] },
    )
    .expect("ink");
    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: hand[1],
            shift_onto: None,
        },
    )
    .expect("play");
    let card = hand[1];

    // Entered play ready → the condition doesn't hold → no buff.
    assert_eq!(state.current_character_stats(card).unwrap().strength, 2);

    // Exert it → condition holds → +2.
    set_ready(&mut state, active, card, false);
    assert_eq!(state.current_character_stats(card).unwrap().strength, 4);

    // Ready again → buff gone.
    set_ready(&mut state, active, card, true);
    assert_eq!(state.current_character_stats(card).unwrap().strength, 2);
}

fn set_ready(state: &mut GameState, owner: PlayerId, card: CardId, ready: bool) {
    state
        .player_mut(owner)
        .unwrap()
        .play_mut()
        .iter_mut()
        .find(|c| c.id() == card)
        .unwrap()
        .conditions_mut()
        .ready = ready;
}

#[test]
fn lore_scales_with_each_other_villain_in_play() {
    // "This character gets +1 {L} for each other Villain character you have in
    // play" (Hades) — a dynamic static whose delta scales with a live count.
    let mut state = started();
    let active = state.active_player();
    let lord = place_classed(&mut state, active, 200, 2, &["Villain"]); // base {L} 1
    state.add_modifier(
        StatModifier::new(
            lord,
            ModifierTarget::Card(lord),
            Stat::Lore,
            1,
            ModifierDuration::WhileSourceInPlay,
        )
        .with_count(lorcana_engine::Amount::PerMatchingCharacter(
            lorcana_engine::CharacterFilter::any(lorcana_engine::TargetSide::Yours)
                .and(lorcana_engine::CharacterFilter::Classification(
                    Classification::new("Villain"),
                ))
                .and(lorcana_engine::CharacterFilter::negate(
                    lorcana_engine::CharacterFilter::IsSource,
                )),
        )),
    );

    // No other Villains yet: just the base lore.
    assert_eq!(state.current_character_stats(lord).unwrap().lore, 1);

    // Two more Villains enter -> +2 {L}; a Hero doesn't count.
    let _ = place_classed(&mut state, active, 201, 2, &["Villain"]);
    let _ = place_classed(&mut state, active, 202, 2, &["Villain"]);
    let _ = place_classed(&mut state, active, 203, 2, &["Hero"]);
    assert_eq!(
        state.current_character_stats(lord).unwrap().lore,
        1 + 2,
        "live count: +1 {{L}} per other Villain"
    );
}
