//! The TOML card-data loader: our committed `cards/*.toml` define real cards in
//! the engine's own format, validated on load (Slice 9).

use lorcana_engine::{
    AbilityCost, ActivatedAbility, Amount, CardDefId, CardKind, CharacterFilter, Classification,
    Condition, Effect, Keyword, NumericFilter, PlayerScope, ShiftAbility, Stat, StaticAbility,
    StaticTarget, Target, TargetSide, TriggerCondition, load_toml,
};

/// The committed example deck, compiled in so the test needs no runtime files.
const EXAMPLES: &str = include_str!("../cards/examples.toml");

#[test]
fn committed_example_cards_load_and_validate() {
    let defs = load_toml(EXAMPLES).expect("examples.toml loads");
    assert_eq!(defs.len(), 12, "all example cards loaded");

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

#[test]
fn the_effect_dsl_maps_abilities_onto_the_ast() {
    let defs = load_toml(EXAMPLES).expect("loads");
    let by_name = |name: &str| defs.iter().find(|d| d.has_name(name)).unwrap();

    // "When you play this, draw a card and gain 1 lore." -> All([Draw, Lore]).
    let genie = by_name("Genie");
    assert_eq!(genie.abilities().len(), 1);
    let play = &genie.abilities()[0];
    assert_eq!(play.condition, TriggerCondition::WhenYouPlayThis);
    assert_eq!(
        play.effect,
        Effect::All(vec![
            Effect::Draw {
                who: PlayerScope::You,
                amount: Amount::fixed(1),
            },
            Effect::Lore {
                who: PlayerScope::You,
                amount: Amount::fixed(1),
            },
        ])
    );

    // "Whenever this quests, chosen opposing character gets -2 {S} this turn."
    let ferdinand = by_name("Ferdinand");
    let quest = &ferdinand.abilities()[0];
    assert_eq!(quest.condition, TriggerCondition::WhenThisQuests);
    assert_eq!(
        quest.effect,
        Effect::GiveStrengthThisTurn {
            target: Target::ChosenCharacter {
                filter: CharacterFilter::any(TargetSide::Opposing),
            },
            amount: Amount::fixed(-2),
        }
    );

    // "{E}, 1 {I} — Draw a card."  (activated)
    let mirror = by_name("Beast's Mirror");
    assert_eq!(
        mirror.activated_abilities(),
        &[ActivatedAbility::new(
            AbilityCost::new(true, 1),
            Effect::Draw {
                who: PlayerScope::You,
                amount: Amount::fixed(1),
            },
        )]
    );

    // "Your other Hero characters get +1 {S}."  (static)
    let hercules = by_name("Hercules");
    assert_eq!(
        hercules.static_abilities(),
        &[StaticAbility {
            target: StaticTarget::OwnedCharacters {
                classifications: vec![Classification::new("Hero")],
                include_self: false,
            },
            stat: Stat::Strength,
            delta: 1,
            condition: None,
            per: None,
        }]
    );
}

#[test]
fn an_unparseable_ability_is_rejected() {
    let err = load_toml(
        r#"
        [[card]]
        name = "Bad"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 1
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { teleport = 1 }
        "#,
    )
    .expect_err("unknown effect verb");
    assert!(format!("{err}").contains("no known effect verb"), "{err}");
}

#[test]
fn a_dsl_authored_card_plays_through_the_engine() {
    use lorcana_engine::{
        CardInstance, CardRegistry, Conditions, GameState, GameStatus, Input, apply, start,
    };

    // Build the registry from the committed TOML, then play one of its cards.
    let mut reg = CardRegistry::new();
    for def in load_toml(EXAMPLES).expect("load") {
        reg.insert(def);
    }
    // examples.toml order: Abu=0 (vanilla cost-1, inkable), Genie=1.
    let abu = CardDefId::from_raw(0);
    let genie_def = CardDefId::from_raw(1);

    let decks = vec![
        (0..30).map(|_| abu).collect::<Vec<_>>(),
        (0..30).map(|_| abu).collect(),
    ];
    let mut state = GameState::new(decks, 7);
    let _ = start(&mut state).expect("start");
    while let GameStatus::AwaitingMulligan(p) = *state.status() {
        let _ = apply(
            &mut state,
            &reg,
            Input::Mulligan {
                player: p,
                put_back: Vec::new(),
            },
        )
        .expect("mulligan");
    }

    let me = state.active_player();
    // Put Genie in hand + 5 ready ink to pay its cost.
    let genie = state.allocate_card_id();
    state
        .player_mut(me)
        .unwrap()
        .hand_mut()
        .push(CardInstance::new(
            genie,
            genie_def,
            Conditions::faceup_idle(),
        ));
    for _ in 0..5 {
        let ink = state.allocate_card_id();
        state
            .player_mut(me)
            .unwrap()
            .inkwell_mut()
            .push(CardInstance::new(ink, abu, Conditions::faceup_idle()));
    }
    let lore_before = state.player(me).unwrap().lore();
    let deck_before = state.player(me).unwrap().deck().iter().count();

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: genie,
            shift_onto: None,
        },
    )
    .expect("play Genie");

    // Genie's TOML ability — "when played, draw a card and gain 1 lore" — fired:
    // a card was drawn and lore went up (and Genie is in play).
    assert!(
        state
            .player(me)
            .unwrap()
            .play()
            .iter()
            .any(|c| c.id() == genie),
        "Genie entered play"
    );
    assert_eq!(
        state.player(me).unwrap().lore(),
        lore_before + 1,
        "gained 1 lore"
    );
    assert_eq!(
        state.player(me).unwrap().deck().iter().count(),
        deck_before - 1,
        "drew a card"
    );
}

#[test]
fn the_dsl_supports_dynamic_amounts_conditionals_and_static_per_while() {
    let defs = load_toml(EXAMPLES).expect("loads");
    let by_name = |name: &str| defs.iter().find(|d| d.has_name(name)).unwrap();
    let villains = || {
        CharacterFilter::any(TargetSide::Yours)
            .and(CharacterFilter::Classification(Classification::new(
                "Villain",
            )))
            .and(CharacterFilter::negate(CharacterFilter::IsSource))
    };

    // "if you have another Villain, gain 1 lore for each other Villain."
    let mal = by_name("Maleficent");
    assert_eq!(
        mal.abilities()[0].effect,
        Effect::IfControl {
            filter: villains(),
            at_least: 1,
            then: Box::new(Effect::Lore {
                who: PlayerScope::You,
                amount: Amount::PerMatchingCharacter(villains()),
            }),
        }
    );

    // Static `per` (for-each) and `while` (condition).
    let cruella = by_name("Cruella De Vil");
    assert_eq!(
        cruella.static_abilities(),
        &[
            StaticAbility {
                target: StaticTarget::SelfCard,
                stat: Stat::Strength,
                delta: 1,
                condition: None,
                per: Some(Amount::PerMatchingCharacter(villains())),
            },
            StaticAbility {
                target: StaticTarget::SelfCard,
                stat: Stat::Lore,
                delta: 1,
                condition: Some(Condition::SourceExerted),
                per: None,
            },
        ]
    );
}

/// DSL — the compact selector grammar parses the richer leaf predicates (a name,
/// and numeric thresholds on cost / `{S}`) into the `CharacterFilter` algebra,
/// composing with side / classification / `another`.
#[test]
fn the_dsl_parses_name_and_threshold_selector_predicates() {
    let defs = load_toml(
        r#"
        [[card]]
        name = "Tester"
        type = "Character"
        cost = 3
        strength = 2
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "play"
        do = { banish = "chosen opposing character with cost 3 or less" }
        [[card.abilities]]
        on = "quest"
        do = { banish = "another Villain character with 3 {S} or more" }
        [[card.abilities]]
        on = "challenge"
        do = { banish = "chosen character named Stitch" }
        "#,
    )
    .expect("loads");
    let card = &defs[0];

    // "with cost 3 or less" -> Cost(at_most 3), composed with the opposing side.
    assert_eq!(
        card.abilities()[0].effect,
        Effect::Banish(Target::ChosenCharacter {
            filter: CharacterFilter::any(TargetSide::Opposing)
                .and(CharacterFilter::Cost(NumericFilter::at_most(3))),
        })
    );
    // "another Villain character with 3 {S} or more" -> Strength(at_least 3) +
    // classification + the `another` exclusion.
    assert_eq!(
        card.abilities()[1].effect,
        Effect::Banish(Target::ChosenCharacter {
            filter: CharacterFilter::any(TargetSide::Any)
                .and(CharacterFilter::Strength(NumericFilter::at_least(3)))
                .and(CharacterFilter::Classification(Classification::new(
                    "Villain"
                )))
                .and(CharacterFilter::negate(CharacterFilter::IsSource)),
        })
    );
    // "named Stitch" -> Named.
    assert_eq!(
        card.abilities()[2].effect,
        Effect::Banish(Target::ChosenCharacter {
            filter: CharacterFilter::any(TargetSide::Any)
                .and(CharacterFilter::Named("Stitch".into())),
        })
    );
}

#[test]
fn an_action_authored_in_the_dsl_resolves_its_effect_on_play() {
    use lorcana_engine::{Amount, Effect, Target};
    // An action's "when you play this" ability is loaded as its on-play action
    // effect (§6.3.2), not a triggered ability — so it actually resolves.
    let defs = load_toml(
        r#"
        [[card]]
        name = "Zap"
        type = "Action"
        cost = 1
        [[card.abilities]]
        on = "play"
        do = { deal_damage = 2, to = "chosen opposing character" }
        "#,
    )
    .expect("loads");
    let zap = &defs[0];
    assert!(
        zap.abilities().is_empty(),
        "the play ability became an action effect, not a trigger"
    );
    assert_eq!(
        zap.action_effects(),
        &[Effect::DealDamage {
            target: Target::ChosenCharacter {
                filter: CharacterFilter::any(TargetSide::Opposing),
            },
            amount: Amount::fixed(2),
        }]
    );
}
