//! Integration tests for Slice 1: setup, mulligan, the turn loop, the
//! once-per-turn inkwell action, first-turn draw skip, and deck-out loss.

use lorcana_engine::{
    CardDefId, CardDefinition, CardRegistry, GameEvent, GameState, GameStatus, Input, Phase,
    PlayerId, Step, apply, start,
};

/// A registry where every card id used in tests has the inkwell symbol.
fn inkable_registry(max_def: u32) -> CardRegistry {
    (0..max_def)
        .map(|n| CardDefinition::new(CardDefId::from_raw(n), true))
        .collect()
}

fn two_decks(size: u32) -> Vec<Vec<CardDefId>> {
    let deck_a: Vec<CardDefId> = (0..size).map(CardDefId::from_raw).collect();
    let deck_b: Vec<CardDefId> = (0..size).map(CardDefId::from_raw).collect();
    vec![deck_a, deck_b]
}

/// Drive both players through a no-op mulligan, returning the game in `Playing`
/// along with every event produced.
fn start_and_skip_mulligans(state: &mut GameState, registry: &CardRegistry) -> Vec<GameEvent> {
    let mut events = start(state).expect("start");
    while let GameStatus::AwaitingMulligan(player) = *state.status() {
        let mulligan = Input::Mulligan {
            player,
            put_back: Vec::new(),
        };
        events.extend(apply(state, registry, mulligan).expect("mulligan"));
    }
    events
}

#[test]
fn setup_deals_seven_and_awaits_mulligan() {
    let mut state = GameState::new(two_decks(30), 7);
    assert!(matches!(state.status(), GameStatus::NotStarted));

    let events = start(&mut state).expect("start");

    assert!(events.contains(&GameEvent::HandsDealt));
    assert!(matches!(state.status(), GameStatus::AwaitingMulligan(_)));
    for player in state.players() {
        assert_eq!(player.hand().len(), 7);
        assert_eq!(player.deck().len(), 23);
    }
}

#[test]
fn mulligan_returns_cards_and_redraws_to_seven() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = inkable_registry(30);
    let _ = start(&mut state).expect("start");

    let GameStatus::AwaitingMulligan(first) = *state.status() else {
        panic!("expected mulligan");
    };
    let put_back: Vec<_> = state
        .player(first)
        .unwrap()
        .hand()
        .iter()
        .take(3)
        .map(|c| c.id())
        .collect();

    let events = apply(
        &mut state,
        &registry,
        Input::Mulligan {
            player: first,
            put_back,
        },
    )
    .expect("mulligan");

    assert!(events.contains(&GameEvent::MulliganResolved {
        player: first,
        returned: 3,
    }));
    assert_eq!(state.player(first).unwrap().hand().len(), 7);
    assert_eq!(state.player(first).unwrap().deck().len(), 23);
}

#[test]
fn play_begins_after_all_mulligans_and_first_turn_skips_draw() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = inkable_registry(30);
    let _ = start_and_skip_mulligans(&mut state, &registry);

    assert!(matches!(state.status(), GameStatus::Playing));
    assert_eq!(state.phase(), Phase::Main);
    assert_eq!(state.turn_number(), 1);

    // First turn skips Draw (§4.2.3.2): the starting player still has 7 cards.
    let starter = state.active_player();
    assert_eq!(state.player(starter).unwrap().hand().len(), 7);
}

#[test]
fn second_player_draws_on_their_first_turn() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = inkable_registry(30);
    let _ = start_and_skip_mulligans(&mut state, &registry);

    let starter = state.active_player();
    let events = apply(&mut state, &registry, Input::EndTurn).expect("end turn");

    // Turn passed to the other player, who does draw (8 cards now).
    let second = state.active_player();
    assert_ne!(starter, second);
    assert_eq!(state.turn_number(), 2);
    assert_eq!(state.player(second).unwrap().hand().len(), 8);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::CardDrawn { player, .. } if *player == second))
    );
}

#[test]
fn inkwell_action_is_once_per_turn() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = inkable_registry(30);
    let _ = start_and_skip_mulligans(&mut state, &registry);

    let active = state.active_player();
    let cards: Vec<_> = state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .take(2)
        .map(|c| c.id())
        .collect();

    let _ = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: cards[0] },
    )
    .expect("first ink ok");
    assert_eq!(state.player(active).unwrap().inkwell().len(), 1);

    let second = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: cards[1] },
    );
    assert!(
        second.is_err(),
        "second ink in the same turn must be rejected"
    );
    // Rejected input does not mutate.
    assert_eq!(state.player(active).unwrap().inkwell().len(), 1);
}

#[test]
fn inkwell_resets_each_turn() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = inkable_registry(30);
    let _ = start_and_skip_mulligans(&mut state, &registry);

    let p0 = state.active_player();
    let card0 = state.player(p0).unwrap().hand().iter().next().unwrap().id();
    let _ = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: card0 },
    )
    .expect("ink t1");
    let _ = apply(&mut state, &registry, Input::EndTurn).expect("end t1");

    // New active player can ink again on their turn.
    let p1 = state.active_player();
    let card1 = state.player(p1).unwrap().hand().iter().next().unwrap().id();
    let _ = apply(
        &mut state,
        &registry,
        Input::PutCardInInkwell { card: card1 },
    )
    .expect("ink t2");
    assert_eq!(state.player(p1).unwrap().inkwell().len(), 1);
}

#[test]
fn drawing_from_empty_deck_loses_the_game() {
    // Each deck has exactly 7 cards: dealt to the opening hand, leaving an empty
    // deck. The starting player skips draw on turn 1; the second player must draw
    // on turn 2 from an empty deck and loses.
    let mut state = GameState::new(two_decks(7), 7);
    let registry = inkable_registry(7);
    let _ = start_and_skip_mulligans(&mut state, &registry);

    let starter = state.active_player();
    let events = apply(&mut state, &registry, Input::EndTurn).expect("end turn");

    let GameStatus::Finished { winners } = state.status() else {
        panic!("game should be finished after the deck-out");
    };
    assert_eq!(winners, &vec![starter]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::GameEnded { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::PlayerLost { .. }))
    );
}

#[test]
fn no_input_is_accepted_after_the_game_ends() {
    let mut state = GameState::new(two_decks(7), 7);
    let registry = inkable_registry(7);
    let _ = start_and_skip_mulligans(&mut state, &registry);
    let _ = apply(&mut state, &registry, Input::EndTurn).expect("end turn ends game");

    assert!(state.is_finished());
    assert!(apply(&mut state, &registry, Input::EndTurn).is_err());
}

#[test]
fn same_seed_and_inputs_are_deterministic() {
    let run = || {
        let mut state = GameState::new(two_decks(30), 99);
        let registry = inkable_registry(30);
        let mut events = start_and_skip_mulligans(&mut state, &registry);
        events.extend(apply(&mut state, &registry, Input::EndTurn).expect("end turn"));
        (state, events)
    };
    let (state_a, events_a) = run();
    let (state_b, events_b) = run();
    assert_eq!(state_a, state_b);
    assert_eq!(events_a, events_b);
}

#[test]
fn first_turn_emits_phase_and_step_events() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = inkable_registry(30);
    let events = start_and_skip_mulligans(&mut state, &registry);

    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::TurnStarted { turn: 1, .. }))
    );
    for step in [Step::Ready, Step::Set, Step::Draw, Step::Main] {
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::StepEntered { step: s } if *s == step)),
            "missing StepEntered for {step:?}"
        );
    }
}

#[test]
fn inkwell_rejects_card_without_symbol() {
    let mut state = GameState::new(two_decks(30), 7);
    // Registry where nothing is inkable.
    let registry: CardRegistry = (0..30)
        .map(|n| CardDefinition::new(CardDefId::from_raw(n), false))
        .collect();
    let _ = start_and_skip_mulligans(&mut state, &registry);

    let active = state.active_player();
    let card = state
        .player(active)
        .unwrap()
        .hand()
        .iter()
        .next()
        .unwrap()
        .id();
    let result = apply(&mut state, &registry, Input::PutCardInInkwell { card });
    assert!(result.is_err());
    assert_eq!(state.player(active).unwrap().inkwell().len(), 0);
}

#[test]
fn wrong_player_for_mulligan_is_rejected() {
    let mut state = GameState::new(two_decks(30), 7);
    let registry = inkable_registry(30);
    let _ = start(&mut state).expect("start");

    // The non-awaited player cannot mulligan first.
    let GameStatus::AwaitingMulligan(awaited) = *state.status() else {
        panic!("expected mulligan");
    };
    let other = if awaited == PlayerId::from_index(0) {
        PlayerId::from_index(1)
    } else {
        PlayerId::from_index(0)
    };
    let result = apply(
        &mut state,
        &registry,
        Input::Mulligan {
            player: other,
            put_back: Vec::new(),
        },
    );
    assert!(result.is_err());
}
