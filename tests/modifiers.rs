//! Integration tests for Slice 5c: the continuous-effects layer — current stats
//! combine the printed base with active modifiers and clamp at the point of use
//! while retaining the true total (§7.8).

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, Conditions,
    GameState, GameStatus, Input, ModifierDuration, ModifierTarget, PlayerId, Stat, StatModifier,
    StaticAbility, apply, start,
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
        .map(|c| c.id())
        .collect();
    let _ = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: hand[0] },
    )
    .expect("ink");
    let _ = apply(&mut state, &registry, Input::PlayCard { card: hand[1] }).expect("play");

    // Base 2 + static 2 = current 4 the moment it enters play (§7.6.2).
    assert_eq!(state.current_character_stats(hand[1]).unwrap().strength, 4);
}
