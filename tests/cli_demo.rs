//! The text host's auto-play demo runs a full game to completion (Slice 10).

use lorcana_engine::application::host;

#[test]
fn the_demo_plays_a_game_to_completion() {
    for seed in 0..5u64 {
        let transcript = host::demo(seed, 5_000);
        assert!(
            transcript.contains("Game over:"),
            "seed {seed}: demo finished within the step budget"
        );
        assert!(
            transcript.contains("step 0:"),
            "seed {seed}: at least one action taken"
        );
    }
}
