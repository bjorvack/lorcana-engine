//! Integration tests for the core determinism guarantee: the same seed and
//! inputs must always produce identical game state.

use lorcana_engine::{CardDefId, GameState};

/// Build two simple decks of distinct printed cards, one per player.
fn sample_decks() -> Vec<Vec<CardDefId>> {
    let deck_a: Vec<CardDefId> = (0..30).map(CardDefId::from_raw).collect();
    let deck_b: Vec<CardDefId> = (100..130).map(CardDefId::from_raw).collect();
    vec![deck_a, deck_b]
}

#[test]
fn same_seed_produces_identical_state() {
    let a = GameState::new(sample_decks(), 0x00C0_FFEE);
    let b = GameState::new(sample_decks(), 0x00C0_FFEE);
    assert_eq!(a, b);
}

#[test]
fn different_seeds_produce_different_shuffles() {
    let a = GameState::new(sample_decks(), 1);
    let b = GameState::new(sample_decks(), 2);

    // The starting state metadata matches, but the shuffled decks should differ.
    let order = |state: &GameState| -> Vec<u32> {
        state.players()[0]
            .deck()
            .iter()
            .map(|c| c.definition().as_raw())
            .collect()
    };
    assert_ne!(order(&a), order(&b));
}

#[test]
fn decks_are_populated_and_initial_turn_is_set() {
    let state = GameState::new(sample_decks(), 7);

    assert_eq!(state.players().len(), 2);
    assert_eq!(state.players()[0].deck().len(), 30);
    assert_eq!(state.players()[1].deck().len(), 30);
    assert_eq!(state.turn_number(), 1);
    assert_eq!(state.active_player().index(), 0);
}
