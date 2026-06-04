//! Deck construction, validation (§2.1.1), and the plain-text share format.

use lorcana_engine::{CardDefId, CardDefinition, CardRegistry, Deck, DeckCard, DeckError, InkType};

fn def(id: u32, name: &str, ink: InkType) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(id), 1, true, 1, 1, 1)
        .with_names(vec![name.to_string()])
        .with_ink_types(vec![ink])
}

fn registry() -> CardRegistry {
    let mut r = CardRegistry::new();
    // 15 distinct Ruby cards (ids 0..15) for building a legal deck.
    for i in 0..15 {
        r.insert(def(i, &format!("Ruby {i}"), InkType::Ruby));
    }
    r.insert(def(100, "Amber Guy", InkType::Amber));
    // A dual-ink Ruby/Sapphire card.
    r.insert(
        CardDefinition::character(CardDefId::from_raw(101), 1, true, 1, 1, 1)
            .with_names(vec!["Dual".to_string()])
            .with_ink_types(vec![InkType::Ruby, InkType::Sapphire]),
    );
    // Copy-limit overrides.
    r.insert(def(102, "Dalmatian Puppy", InkType::Amber).with_max_deck_copies(Some(99)));
    r.insert(def(103, "The Glass Slipper", InkType::Steel).with_max_deck_copies(Some(2)));
    // Two printings of the same name (different ids).
    r.insert(def(104, "Dragon Fire", InkType::Ruby));
    r.insert(def(105, "Dragon Fire", InkType::Ruby));
    r
}

fn has<F: Fn(&DeckError) -> bool>(errs: &[DeckError], f: F) -> bool {
    errs.iter().any(f)
}

#[test]
fn a_legal_deck_validates() {
    let reg = registry();
    let mut deck = Deck::default();
    for i in 0..15 {
        deck.add(CardDefId::from_raw(i), 4); // 15 names x 4 = 60, one ink, <=4 each
    }
    assert_eq!(deck.total(), 60);
    assert!(deck.validate(&reg).is_ok());
}

#[test]
fn fewer_than_60_cards_is_rejected() {
    let reg = registry();
    let mut deck = Deck::default();
    for i in 0..14 {
        deck.add(CardDefId::from_raw(i), 4); // 56
    }
    let errs = deck.validate(&reg).unwrap_err();
    assert!(has(&errs, |e| matches!(
        e,
        DeckError::TooFewCards { total: 56 }
    )));
}

#[test]
fn a_dual_ink_card_commits_both_of_its_colours() {
    // Dual Ruby/Sapphire + an Amber card => 3 ink types => illegal.
    let reg = registry();
    let mut deck = Deck::default();
    deck.add(CardDefId::from_raw(101), 1); // Ruby + Sapphire
    deck.add(CardDefId::from_raw(100), 1); // Amber
    let errs = deck.validate(&reg).unwrap_err();
    assert!(has(&errs, |e| matches!(
        e,
        DeckError::TooManyInks { count: 3 }
    )));
}

#[test]
fn copy_limit_defaults_to_four_and_respects_overrides() {
    let reg = registry();

    // 5 copies of a normal card -> over the default 4.
    let mut deck = Deck::default();
    deck.add(CardDefId::from_raw(0), 5);
    let errs = deck.validate(&reg).unwrap_err();
    assert!(has(&errs, |e| matches!(
        e,
        DeckError::TooManyCopies {
            max: 4,
            count: 5,
            ..
        }
    )));

    // The Glass Slipper override lowers the limit to 2.
    let mut slipper = Deck::default();
    slipper.add(CardDefId::from_raw(103), 3);
    let errs = slipper.validate(&reg).unwrap_err();
    assert!(has(&errs, |e| matches!(
        e,
        DeckError::TooManyCopies {
            max: 2,
            count: 3,
            ..
        }
    )));

    // Dalmatian Puppy allows up to 99, so a 60-copy single-card deck is fully legal.
    let mut pups = Deck::default();
    pups.add(CardDefId::from_raw(102), 60);
    assert!(
        pups.validate(&reg).is_ok(),
        "99-copy override permits 60 Dalmatians"
    );
}

#[test]
fn copies_are_counted_by_name_across_printings() {
    // 3 + 3 copies of two printings of "Dragon Fire" = 6 of that name > 4.
    let reg = registry();
    let mut deck = Deck::default();
    deck.add(CardDefId::from_raw(104), 3);
    deck.add(CardDefId::from_raw(105), 3);
    let errs = deck.validate(&reg).unwrap_err();
    assert!(has(
        &errs,
        |e| matches!(e, DeckError::TooManyCopies { name, count: 6, max: 4 } if name == "Dragon Fire")
    ));
}

#[test]
fn plain_text_round_trips_and_groups_printings_by_name() {
    let reg = registry();
    // Import the community format (count + name).
    let deck = Deck::from_text("4 Ruby 0\n2 Dragon Fire\n# a comment\n\n", &reg).expect("parse");
    assert_eq!(deck.total(), 6);

    // Two printings of the same name export as one grouped line.
    let mut grouped = Deck::default();
    grouped.add(CardDefId::from_raw(104), 3);
    grouped.add(CardDefId::from_raw(105), 3);
    assert_eq!(grouped.to_text(&reg), "6 Dragon Fire\n");
}

#[test]
fn an_unknown_name_in_text_is_reported() {
    let reg = registry();
    let err = Deck::from_text("4 Nonexistent Card", &reg).unwrap_err();
    assert!(matches!(err, DeckError::UnknownCardName(n) if n == "Nonexistent Card"));
}

#[test]
fn expand_produces_the_flat_card_list() {
    let mut deck = Deck::default();
    deck.add(CardDefId::from_raw(0), 2);
    deck.add(CardDefId::from_raw(1), 1);
    assert_eq!(
        deck.expand(),
        vec![
            CardDefId::from_raw(0),
            CardDefId::from_raw(0),
            CardDefId::from_raw(1),
        ]
    );
    // DeckCard is the storage unit.
    assert_eq!(
        deck.cards[0],
        DeckCard {
            card: CardDefId::from_raw(0),
            count: 2
        }
    );
}
