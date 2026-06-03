//! Self-play / fuzz (Slice 10 robustness): drive random *legal* actions to
//! completion across many seeds and assert the engine never panics, every
//! reported-legal action is accepted, and core invariants hold.

use lorcana_engine::{
    CardDefId, CardDefinition, CardRegistry, Game, GameStatus, PlayerState, lore_to_win,
};

/// A tiny deterministic PRNG so failures reproduce from the seed.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        ((self.0 >> 33) as usize) % bound.max(1)
    }
}

fn registry() -> CardRegistry {
    let mut reg = CardRegistry::new();
    for n in 0..30 {
        reg.insert(CardDefinition::character(
            CardDefId::from_raw(n),
            1,
            true,
            2,
            2,
            1,
        ));
    }
    reg
}

/// Total cards a player owns across every zone (conserved at 30 all game).
fn total_cards(p: &PlayerState) -> usize {
    p.deck().iter().count()
        + p.hand().iter().count()
        + p.play().iter().count()
        + p.inkwell().iter().count()
        + p.discard().iter().count()
}

const fn finished(game: &Game) -> bool {
    matches!(game.status(), GameStatus::Finished { .. })
}

#[test]
fn random_self_play_is_robust_across_seeds() {
    for seed in 0..25u64 {
        let decks = vec![
            (0..30).map(CardDefId::from_raw).collect::<Vec<_>>(),
            (0..30).map(CardDefId::from_raw).collect(),
        ];
        let mut game = Game::new(decks, seed, registry()).expect("new game");
        let mut rng = Lcg(seed.wrapping_add(1));

        for _step in 0..5_000 {
            if finished(&game) {
                break;
            }
            let actions = game.legal_actions();
            assert!(
                !actions.is_empty(),
                "seed {seed}: no legal action but game not finished ({:?})",
                game.status()
            );
            let pick = actions[rng.next(actions.len())].clone();
            let _ = game
                .submit(pick.clone())
                .unwrap_or_else(|e| panic!("seed {seed}: reported-legal {pick:?} rejected: {e:?}"));

            // Invariants.
            for p in game.state().players() {
                assert_eq!(total_cards(p), 30, "seed {seed}: cards conserved");
            }
            if !finished(&game) {
                for p in game.state().players() {
                    assert!(
                        p.lore() < lore_to_win(game.state(), p.id()),
                        "seed {seed}: a player reached the win threshold but the game isn't finished"
                    );
                }
            }
        }
    }
}
