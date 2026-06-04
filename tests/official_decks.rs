//! The committed official decklists under `decks/` must be exactly 60 cards and,
//! for the cards present in our pool, satisfy the deck-building rules (§2.1.1).
//! Starter decks include reprints from earlier sets, so they're checked against a
//! **combined** registry built from every `cards/sets/*.toml`. A few cards are not
//! yet in the pool (known generation gaps); those are reported, not failed.

use std::fs;
use std::path::Path;

use lorcana_engine::{CardRegistry, Deck, DeckCard, load_toml_from};

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
fn official_decklists_are_60_cards_and_legal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let decks_dir = root.join("decks");
    if !decks_dir.exists() {
        return;
    }
    let registry = combined_registry(root);
    let mut checked = 0usize;
    let mut missing = 0usize;
    for entry in fs::read_dir(&decks_dir).expect("read decks/") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read decklist");

        // Lenient parse: sum the full official count, resolve the cards we have.
        let mut total = 0u32;
        let mut resolved = Deck::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (count, name) = line.split_once(char::is_whitespace).expect("count name");
            let count: u32 = count.trim_end_matches('x').parse().expect("count");
            total += count;
            if let Some(id) = registry.find_by_name(name.trim()) {
                resolved.cards.push(DeckCard { card: id, count });
            } else {
                eprintln!("{}: card not yet in pool: {name:?}", path.display());
                missing += 1;
            }
        }
        assert_eq!(total, 60, "{} must be exactly 60 cards", path.display());

        // The cards we DO have must not violate the ink / copy rules.
        if let Err(errs) = resolved.validate(&registry) {
            let serious: Vec<_> = errs
                .into_iter()
                .filter(|e| !matches!(e, lorcana_engine::DeckError::TooFewCards { .. }))
                .collect();
            assert!(serious.is_empty(), "{}: {serious:?}", path.display());
        }
        checked += 1;
    }
    eprintln!("checked {checked} official decklists ({missing} cards not yet in pool)");
}
