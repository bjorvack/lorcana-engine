//! Integration test for game-state serialization: a `GameState` must survive a
//! serialize/deserialize round-trip unchanged, including its RNG state.

use lorcana_engine::{CardDefId, GameState};

#[test]
fn game_state_survives_json_round_trip() {
    let decks = vec![
        (0..40).map(CardDefId::from_raw).collect::<Vec<_>>(),
        (40..80).map(CardDefId::from_raw).collect::<Vec<_>>(),
    ];
    let original = GameState::new(decks, 0xABCD_1234);

    let json = serde_json::to_string(&original).expect("serialization should succeed");
    let restored: GameState = serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(original, restored);
}
