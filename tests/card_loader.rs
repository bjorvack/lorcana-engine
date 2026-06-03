//! The TOML card-data loader: our committed `cards/*.toml` define real cards in
//! the engine's own format, validated on load (Slice 9).

use lorcana_engine::{CardDefId, CardKind, Classification, Keyword, ShiftAbility, load_toml};

/// The committed example deck, compiled in so the test needs no runtime files.
const EXAMPLES: &str = include_str!("../cards/examples.toml");

#[test]
fn committed_example_cards_load_and_validate() {
    let defs = load_toml(EXAMPLES).expect("examples.toml loads");
    assert_eq!(defs.len(), 8, "all example cards loaded");

    // ids are assigned sequentially by position.
    assert_eq!(defs[0].id(), CardDefId::from_raw(0));

    let by_name = |name: &str| {
        defs.iter()
            .find(|d| d.has_name(name))
            .unwrap_or_else(|| panic!("{name} present"))
    };

    // A vanilla character: stats + classifications, no keywords.
    let abu = by_name("Abu");
    assert!(matches!(
        abu.kind(),
        CardKind::Character {
            strength: 1,
            willpower: 2,
            lore: 1
        }
    ));
    assert_eq!(abu.cost(), 1);
    assert!(abu.has_inkwell_symbol());
    assert!(abu.has_classification(&Classification::new("Ally")));
    assert!(abu.keywords().is_empty());

    // Valueless keyword.
    assert!(by_name("Genie").keywords().contains(&Keyword::Evasive));

    // Valued keywords parse their inline number.
    assert!(
        by_name("Captain Hook")
            .keywords()
            .contains(&Keyword::Challenger(2))
    );
    assert!(
        by_name("Donald Duck")
            .keywords()
            .contains(&Keyword::Resist(1))
    );
    assert!(
        by_name("Donald Duck")
            .keywords()
            .contains(&Keyword::Bodyguard)
    );
    assert!(
        by_name("Aladdin")
            .keywords()
            .contains(&Keyword::Shift(ShiftAbility::ink_same_name(5)))
    );

    // Non-character kinds.
    assert!(matches!(
        by_name("A Whole New World").kind(),
        CardKind::Action
    ));
    assert!(matches!(by_name("Beast's Mirror").kind(), CardKind::Item));
    assert!(matches!(
        by_name("The Great Illuminary").kind(),
        CardKind::Location {
            move_cost: 1,
            willpower: 3,
            lore: 1
        }
    ));
}

#[test]
fn a_character_missing_stats_is_rejected() {
    let err = load_toml(
        r#"
        [[card]]
        name = "Bad"
        type = "Character"
        cost = 3
        "#,
    )
    .expect_err("missing strength/willpower/lore");
    assert!(format!("{err}").contains("missing stat"), "{err}");
}

#[test]
fn an_unknown_keyword_is_rejected() {
    let err = load_toml(
        r#"
        [[card]]
        name = "Bad"
        type = "Item"
        cost = 2
        keywords = ["Teleport"]
        "#,
    )
    .expect_err("unknown keyword");
    assert!(format!("{err}").contains("could not be loaded"), "{err}");
}
