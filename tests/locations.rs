//! Integration tests for Slice 7b: locations & movement — playing a location,
//! Set-step lore (§6.5.6), willpower banishment (§6.5.5), and moving a character
//! to a location for its move cost (§4.3.7).

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, Conditions,
    GameState, GameStatus, Input, LocationStats, PlayerId, apply, game_state_check, start,
};

fn started(reg: &CardRegistry) -> GameState {
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
            reg,
            Input::Mulligan {
                player,
                put_back: Vec::new(),
            },
        )
        .expect("mulligan");
    }
    state
}

fn inject_location(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    willpower: u32,
    lore: u32,
    move_cost: u32,
    damage: u32,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut inst = CardInstance::new(
        id,
        CardDefId::from_raw(raw),
        Conditions {
            ready: true,
            damage,
            drying: false,
            facedown: false,
        },
    );
    inst.set_location_stats(Some(LocationStats::new(willpower, lore, move_cost)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn inject_character(state: &mut GameState, owner: PlayerId, raw: u32) -> CardId {
    let id = CardId::from_raw(raw);
    let mut inst = CardInstance::new(
        id,
        CardDefId::from_raw(raw),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    inst.set_stats(Some(CharacterStats::new(2, 3, 1)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn in_play(state: &GameState, owner: PlayerId, card: CardId) -> Option<&CardInstance> {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
}

#[test]
fn playing_a_location_puts_it_into_play() {
    let reg: CardRegistry = (0..30)
        .map(|n| CardDefinition::location(CardDefId::from_raw(n), 0, true, 1, 3, 1))
        .collect();
    let mut state = started(&reg);
    let active = state.active_player();
    let card = state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .next()
        .unwrap()
        .id();

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card,
            shift_onto: None,
        },
    )
    .expect("play location");

    let inst = in_play(&state, active, card).expect("location is in play");
    assert!(inst.is_location());
}

#[test]
fn a_location_grants_lore_at_the_set_step() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let _loc = inject_location(&mut state, me, 5000, 3, 2, 1, 0); // lore 2

    // Cycle back to my turn; my Set step grants the location's lore.
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end my turn");
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end opponent turn");

    assert_eq!(state.active_player(), me, "back to my turn");
    assert_eq!(
        state.player(me).unwrap().lore(),
        2,
        "gained the location's lore at Set"
    );
}

#[test]
fn a_location_is_banished_when_damage_reaches_its_willpower() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let location = inject_location(&mut state, me, 5000, 2, 0, 1, 2); // willpower 2, damage 2

    let _ = game_state_check(&mut state);

    assert!(
        in_play(&state, me, location).is_none(),
        "the location was banished"
    );
    assert!(state.player(me).unwrap().discard().contains(location));
}

#[test]
fn a_character_moves_to_a_location_and_is_recorded_there() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let character = inject_character(&mut state, me, 100);
    let location = inject_location(&mut state, me, 5000, 3, 1, 0, 0); // move cost 0

    let _ = apply(
        &mut state,
        &reg,
        Input::MoveCharacter {
            character,
            location,
        },
    )
    .expect("move");

    assert_eq!(
        in_play(&state, me, character).unwrap().at_location(),
        Some(location)
    );
}

#[test]
fn moving_to_a_non_location_is_rejected() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let character = inject_character(&mut state, me, 100);
    let other = inject_character(&mut state, me, 101); // a character, not a location

    assert!(
        apply(
            &mut state,
            &reg,
            Input::MoveCharacter {
                character,
                location: other,
            },
        )
        .is_err()
    );
}
