//! Self-play / fuzz over the **official decklists**: play full games between real
//! 60-card decks across many seeds and matchups, asserting the engine never
//! panics, every reported-legal action is accepted, and core invariants hold.
//! This dogfoods the ~300 authored cards far more than isolated unit tests.

use std::fs;
use std::path::Path;

use lorcana_engine::{
    CardRegistry, Deck, Game, GameStatus, PlayerState, load_toml_from, lore_to_win,
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

fn combined_registry(root: &Path) -> CardRegistry {
    let mut registry = CardRegistry::new();
    let mut next_id = 0u32;
    let mut files: Vec<_> = fs::read_dir(root.join("cards/sets"))
        .expect("read cards/sets")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    files.sort();
    for path in files {
        let toml = fs::read_to_string(&path).expect("read set");
        let defs = load_toml_from(&toml, next_id).expect("load set");
        next_id += u32::try_from(defs.len()).unwrap_or(0) + 1;
        for def in defs {
            registry.insert(def);
        }
    }
    registry
}

fn official_decklists(root: &Path, registry: &CardRegistry) -> Vec<(String, Deck)> {
    let mut files: Vec<_> = fs::read_dir(root.join("decks"))
        .expect("read decks")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("txt"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&p).expect("read decklist");
            (
                name,
                Deck::from_text(&text, registry).expect("decklist resolves"),
            )
        })
        .collect()
}

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
fn official_decklists_self_play_is_robust() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry = combined_registry(root);
    let decklists = official_decklists(root, &registry);
    assert!(decklists.len() >= 20, "expected the official decklists");

    let mut finishes = 0usize;
    let games = 30usize;
    for game_idx in 0..games {
        let seed = game_idx as u64;
        // Vary the matchup across all decklists.
        let i = game_idx % decklists.len();
        let j = (i + 1 + game_idx / decklists.len()) % decklists.len();
        let (n1, d1) = &decklists[i];
        let (n2, d2) = &decklists[j];
        let matchup = format!("{n1} vs {n2} @ seed {seed}");

        let mut game = Game::from_decks(&[d1.clone(), d2.clone()], seed, registry.clone())
            .unwrap_or_else(|e| panic!("{matchup}: setup failed: {e}"));
        let mut rng = Lcg(seed.wrapping_add(1));

        for _ in 0..10_000 {
            if finished(&game) {
                break;
            }
            let actions = game.legal_actions();
            assert!(
                !actions.is_empty(),
                "{matchup}: no legal action but not finished ({:?})",
                game.status()
            );
            let pick = actions[rng.next(actions.len())].clone();
            let _ = game
                .submit(pick.clone())
                .unwrap_or_else(|e| panic!("{matchup}: reported-legal {pick:?} rejected: {e:?}"));

            for p in game.state().players() {
                assert_eq!(total_cards(p), 60, "{matchup}: cards conserved");
            }
            if !finished(&game) {
                for p in game.state().players() {
                    assert!(
                        p.lore() < lore_to_win(game.state(), p.id()),
                        "{matchup}: win threshold reached but game not finished"
                    );
                }
            }
        }
        if finished(&game) {
            finishes += 1;
        }
    }
    // Random play should actually finish the large majority of games.
    assert!(
        finishes >= games / 2,
        "only {finishes}/{games} games finished within the step budget"
    );
}
