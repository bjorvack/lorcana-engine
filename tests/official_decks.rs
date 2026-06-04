//! The committed official decklists under `decks/` must each resolve against the
//! full card pool and satisfy the deck-building rules (§2.1.1). Starter decks
//! include reprints from earlier sets, so they're validated against a **combined**
//! registry built from every `cards/sets/*.toml` (unique ids via `load_toml_from`).

use std::fs;
use std::path::Path;

use lorcana_engine::{CardRegistry, Deck, load_toml_from};

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

#[test]
fn official_decklists_resolve_and_are_legal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let decks_dir = root.join("decks");
    if !decks_dir.exists() {
        return;
    }
    let registry = combined_registry(root);
    let mut checked = 0usize;
    for entry in fs::read_dir(&decks_dir).expect("read decks/") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read decklist");
        let deck =
            Deck::from_text(&text, &registry).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            deck.total(),
            60,
            "{} should be exactly 60 cards, was {}",
            path.display(),
            deck.total()
        );
        if let Err(errs) = deck.validate(&registry) {
            panic!("{} is not a legal deck: {errs:?}", path.display());
        }
        checked += 1;
    }
    eprintln!("validated {checked} official decklists");
}
