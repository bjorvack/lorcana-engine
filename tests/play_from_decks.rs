//! End-to-end: load the real card pool + two official decklists, then start and
//! drive a game through the `Game` facade — the full deck -> play pipeline.

use std::fs;
use std::path::Path;

use lorcana_engine::{CardRegistry, Deck, Game, GameStatus, load_toml_from};

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

fn deck(root: &Path, file: &str, registry: &CardRegistry) -> Deck {
    let text = fs::read_to_string(root.join("decks").join(file)).expect("read decklist");
    Deck::from_text(&text, registry).expect("decklist resolves")
}

#[test]
fn a_game_starts_from_two_official_decklists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry = combined_registry(root);

    let p1 = deck(root, "set01-amber-amethyst.txt", &registry);
    let p2 = deck(root, "set01-emerald-ruby.txt", &registry);
    assert_eq!(p1.total(), 60);
    assert_eq!(p2.total(), 60);

    let mut game = Game::from_decks(&[p1, p2], 42, registry).expect("legal decks start a game");

    // After start, the engine is in the pre-game mulligan phase.
    assert!(
        matches!(game.status(), GameStatus::AwaitingMulligan(_)),
        "expected mulligan phase, got {:?}",
        game.status()
    );

    // Drive the first legal action for a few steps; nothing should panic and every
    // reported action must be accepted (the facade's core invariant).
    for _ in 0..20 {
        let Some(action) = game.legal_actions().into_iter().next() else {
            break;
        };
        let _ = game
            .submit(action)
            .expect("a reported legal action is accepted");
    }
}

#[test]
fn an_illegal_deck_is_rejected_by_from_decks() {
    use lorcana_engine::SetupError;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry = combined_registry(root);

    // A 1-card "deck" violates the 60-card minimum.
    let legal = deck(root, "set01-amber-amethyst.txt", &registry);
    let mut tiny = Deck::default();
    tiny.cards.push(legal.cards[0]);

    let err = Game::from_decks(&[legal, tiny], 1, registry).unwrap_err();
    assert!(matches!(err, SetupError::IllegalDeck { index: 1, .. }));
}
