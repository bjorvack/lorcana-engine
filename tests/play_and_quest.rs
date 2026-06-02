//! Integration tests for Slice 2: playing characters (paying ink, entering
//! drying), drying → dry across turns, questing for lore, and winning at 20.

use lorcana_engine::{
    CardDefId, CardDefinition, CardKind, CardRegistry, GameEvent, GameState, GameStatus, Input,
    apply, start,
};

/// A registry of inkable cost-1 characters with the given lore (and 1/1 stats).
fn character_registry(max_def: u32, lore: u32) -> CardRegistry {
    (0..max_def)
        .map(|n| CardDefinition::character(CardDefId::from_raw(n), 1, true, 1, 1, lore))
        .collect()
}

fn two_decks(size: u32) -> Vec<Vec<CardDefId>> {
    vec![
        (0..size).map(CardDefId::from_raw).collect(),
        (0..size).map(CardDefId::from_raw).collect(),
    ]
}

fn start_to_play(state: &mut GameState, registry: &CardRegistry) {
    let _ = start(state).expect("start");
    while let GameStatus::AwaitingMulligan(player) = *state.status() {
        let _ = apply(
            state,
            registry,
            Input::Mulligan {
                player,
                put_back: Vec::new(),
            },
        )
        .expect("mulligan");
    }
}

fn active_hand_card(state: &GameState, nth: usize) -> lorcana_engine::CardId {
    let active = state.active_player();
    state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .nth(nth)
        .unwrap()
        .id()
}

#[test]
fn playing_a_character_pays_ink_and_enters_drying() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = character_registry(30, 2);
    start_to_play(&mut state, &registry);

    let active = state.active_player();
    let ink = active_hand_card(&state, 0);
    let subject = active_hand_card(&state, 1);

    let _ = apply(&mut state, &registry, Input::PutCardInInkwell { card: ink }).expect("ink");
    let events = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    )
    .expect("play");

    assert!(events.contains(&GameEvent::CardPlayed {
        player: active,
        card: subject,
    }));
    let owner = state.player(active).unwrap();
    assert_eq!(owner.play().len(), 1);
    // The played character is drying and the ink was spent.
    let in_play = owner.play().iter().next().unwrap();
    assert!(in_play.conditions().drying);
    assert_eq!(owner.ready_ink(), 0);
    assert_eq!(owner.inkwell().len(), 1);
}

#[test]
fn insufficient_ink_is_rejected_without_mutation() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = character_registry(30, 2);
    start_to_play(&mut state, &registry);

    let active = state.active_player();
    let subject = active_hand_card(&state, 0);
    let hand_before = state.player(active).unwrap().hand().len();

    // No ink put down this turn, so a cost-1 character cannot be paid for.
    let result = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    );
    assert!(result.is_err());
    assert_eq!(state.player(active).unwrap().play().len(), 0);
    assert_eq!(state.player(active).unwrap().hand().len(), hand_before);
}

#[test]
fn cannot_quest_a_drying_character() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = character_registry(30, 2);
    start_to_play(&mut state, &registry);

    let ink = active_hand_card(&state, 0);
    let subject = active_hand_card(&state, 1);
    let _ = apply(&mut state, &registry, Input::PutCardInInkwell { card: ink }).expect("ink");
    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    )
    .expect("play");

    let result = apply(&mut state, &registry, Input::Quest { character: subject });
    assert!(result.is_err(), "a drying character cannot quest");
}

#[test]
fn character_dries_next_turn_and_quests_for_lore() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = character_registry(30, 2);
    start_to_play(&mut state, &registry);

    let starter = state.active_player();
    let ink = active_hand_card(&state, 0);
    let subject = active_hand_card(&state, 1);
    let _ = apply(&mut state, &registry, Input::PutCardInInkwell { card: ink }).expect("ink");
    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    )
    .expect("play");

    // Pass back to the starter (2 player): T1 end → opponent → end → starter T3.
    let _ = apply(&mut state, &registry, Input::EndTurn).expect("end t1");
    let _ = apply(&mut state, &registry, Input::EndTurn).expect("end t2");
    assert_eq!(state.active_player(), starter);

    // The character has dried; questing exerts it and gains its lore.
    let lore_before = state.player(starter).unwrap().lore();
    let events = apply(&mut state, &registry, Input::Quest { character: subject }).expect("quest");

    assert_eq!(state.player(starter).unwrap().lore(), lore_before + 2);
    assert!(events.contains(&GameEvent::LoreGained {
        player: starter,
        amount: 2,
    }));
    let character = state
        .player(starter)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == subject)
        .unwrap();
    assert!(
        !character.conditions().ready,
        "questing exerts the character"
    );
}

#[test]
fn questing_can_win_the_game_at_twenty_lore() {
    let mut state = GameState::new(two_decks(30), 7);
    // A single quest of a 20-lore character reaches the win threshold.
    let registry = character_registry(30, 20);
    start_to_play(&mut state, &registry);

    let starter = state.active_player();
    let ink = active_hand_card(&state, 0);
    let subject = active_hand_card(&state, 1);
    let _ = apply(&mut state, &registry, Input::PutCardInInkwell { card: ink }).expect("ink");
    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    )
    .expect("play");
    let _ = apply(&mut state, &registry, Input::EndTurn).expect("end t1");
    let _ = apply(&mut state, &registry, Input::EndTurn).expect("end t2");

    let events = apply(&mut state, &registry, Input::Quest { character: subject }).expect("quest");

    let GameStatus::Finished { winners } = state.status() else {
        panic!("reaching 20 lore should finish the game");
    };
    assert_eq!(winners, &vec![starter]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::GameEnded { .. }))
    );
}

#[test]
fn quest_for_a_character_not_in_play_is_rejected() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = character_registry(30, 2);
    start_to_play(&mut state, &registry);

    // A hand card is not in play and cannot quest.
    let hand_card = active_hand_card(&state, 0);
    let result = apply(
        &mut state,
        &registry,
        Input::Quest {
            character: hand_card,
        },
    );
    assert!(result.is_err());
}

#[test]
fn non_character_cards_cannot_be_played_yet() {
    let mut state = GameState::new(two_decks(30), 7);
    // All ids map to Item cards (items/locations aren't playable yet; actions are).
    let registry: CardRegistry = (0..30)
        .map(|n| CardDefinition::new(CardDefId::from_raw(n), 1, true, CardKind::Item))
        .collect();
    start_to_play(&mut state, &registry);

    let ink = active_hand_card(&state, 0);
    let subject = active_hand_card(&state, 1);
    let _ = apply(&mut state, &registry, Input::PutCardInInkwell { card: ink }).expect("ink");
    let result = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    );
    assert!(result.is_err());
    assert_eq!(state.player(state.active_player()).unwrap().play().len(), 0);
}
