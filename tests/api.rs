//! The application-layer `Game` facade (Slice 10): create, submit, and the
//! `legal_actions` enumeration, exercised end-to-end.

use lorcana_engine::{CardDefId, CardDefinition, CardRegistry, Game, GameStatus, Input};

fn registry() -> CardRegistry {
    let mut reg = CardRegistry::new();
    for n in 0..30 {
        reg.insert(CardDefinition::character(
            CardDefId::from_raw(n),
            1,
            true,
            1,
            1,
            1,
        ));
    }
    reg
}

fn new_game() -> Game {
    let decks = vec![
        (0..30).map(CardDefId::from_raw).collect::<Vec<_>>(),
        (0..30).map(CardDefId::from_raw).collect(),
    ];
    Game::new(decks, 7, registry()).expect("new game")
}

#[test]
fn legal_actions_drive_a_game_and_are_all_accepted() {
    let mut game = new_game();

    // Mulligan phase: the only action is the (keep-all) mulligan.
    while matches!(game.status(), GameStatus::AwaitingMulligan(_)) {
        let actions = game.legal_actions();
        assert_eq!(actions.len(), 1, "exactly the mulligan is offered");
        assert!(matches!(actions[0], Input::Mulligan { .. }));
        let a = actions[0].clone();
        let _ = game.submit(a).expect("mulligan");
    }

    // Turn 1: ending the turn and inking/playing hand cards are all available.
    let actions = game.legal_actions();
    assert!(actions.contains(&Input::EndTurn), "can end the turn");
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, Input::PutCardInInkwell { .. })),
        "can ink a hand card"
    );

    // Invariant: everything legal_actions reports is actually accepted by apply.
    assert!(!actions.is_empty());
    for action in &actions {
        let mut probe = game.clone();
        assert!(
            probe.submit(action.clone()).is_ok(),
            "reported-legal action was accepted: {action:?}"
        );
    }
}

#[test]
fn a_full_turn_can_be_played_via_the_facade() {
    let mut game = new_game();
    while matches!(game.status(), GameStatus::AwaitingMulligan(_)) {
        let a = game.legal_actions()[0].clone();
        let _ = game.submit(a).expect("mulligan");
    }
    let active_before = game.state().active_player();

    // Ink a card, then end the turn — the active player should change.
    let ink = game
        .legal_actions()
        .into_iter()
        .find(|a| matches!(a, Input::PutCardInInkwell { .. }))
        .expect("an ink action");
    let _ = game.submit(ink).expect("ink");
    let _ = game.submit(Input::EndTurn).expect("end turn");

    assert_ne!(
        game.state().active_player(),
        active_before,
        "turn passed to the opponent"
    );
}
