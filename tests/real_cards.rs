//! A small sample of real Lorcana cards (`cards/set1_sample.toml`), authored in the
//! engine's TOML DSL — dogfooding the loader/DSL on real card text, and proving a
//! real card plays end-to-end (Slice 9 "a meaningful subset loads").

use lorcana_engine::{
    Amount, CardDefId, CardInstance, CardRegistry, Conditions, DiscardAmount, DiscardBy, Effect,
    GameState, GameStatus, Input, PlayerScope, TriggerCondition, apply, load_toml, start,
};

const SET: &str = r#"
# A small sample of real Lorcana cards expressed in the engine's own TOML format.
# Authored from public card facts (Lorcast used only as research); never loaded
# from an external dataset. These exercise the effect DSL on real card text.

[[card]]
name = "Jasmine"
type = "Character"
cost = 3
inkwell = true
strength = 2
willpower = 3
lore = 2
classifications = ["Storyborn", "Hero", "Princess"]
# "Whenever this character quests, each opponent loses 1 lore."
[[card.abilities]]
on = "quest"
do = { lose_lore = 1 }

[[card]]
name = "The White Rose"
type = "Character"
cost = 3
inkwell = true
strength = 3
willpower = 3
lore = 1
classifications = ["Storyborn"]
# "When you play this character, gain 1 lore."
[[card.abilities]]
on = "play"
do = { gain_lore = 1 }

[[card]]
name = "Daisy Duck"
type = "Character"
cost = 4
inkwell = true
strength = 2
willpower = 3
lore = 1
classifications = ["Dreamborn", "Hero", "Musketeer"]
# "Whenever this character quests, each opponent chooses and discards a card."
[[card.abilities]]
on = "quest"
do = { discard = 1, who = "each opponent" }
"#;

#[test]
fn the_real_card_sample_loads_and_maps_onto_the_ast() {
    let defs = load_toml(SET).expect("set1 loads");
    assert_eq!(defs.len(), 3);
    let by_name = |n: &str| defs.iter().find(|d| d.has_name(n)).unwrap();

    // Jasmine — "each opponent loses 1 lore" on quest.
    assert_eq!(
        by_name("Jasmine").abilities()[0].effect,
        Effect::Lore {
            who: PlayerScope::EachOpponent,
            amount: Amount::fixed(-1),
        }
    );
    // The White Rose — "gain 1 lore" on play.
    let rose = by_name("The White Rose");
    assert_eq!(
        rose.abilities()[0].condition,
        TriggerCondition::WhenYouPlayThis
    );
    assert_eq!(
        rose.abilities()[0].effect,
        Effect::Lore {
            who: PlayerScope::You,
            amount: Amount::fixed(1),
        }
    );
    // Daisy Duck — "each opponent chooses and discards a card" on quest.
    assert_eq!(
        by_name("Daisy Duck").abilities()[0].effect,
        Effect::Discard {
            who: PlayerScope::EachOpponent,
            amount: DiscardAmount::Count(1),
            by: DiscardBy::Owner,
        }
    );
}

#[test]
fn the_white_rose_gains_lore_when_played() {
    let mut reg = CardRegistry::new();
    for def in load_toml(SET).expect("load") {
        reg.insert(def);
    }
    // The White Rose is def index 1 (Jasmine=0); cost 3.
    let rose_def = CardDefId::from_raw(1);
    let filler = CardDefId::from_raw(0);

    let decks = vec![
        (0..30).map(|_| filler).collect::<Vec<_>>(),
        (0..30).map(|_| filler).collect(),
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
    let rose = state.allocate_card_id();
    state
        .player_mut(me)
        .unwrap()
        .hand_mut()
        .push(CardInstance::new(rose, rose_def, Conditions::faceup_idle()));
    for _ in 0..3 {
        let ink = state.allocate_card_id();
        state
            .player_mut(me)
            .unwrap()
            .inkwell_mut()
            .push(CardInstance::new(ink, filler, Conditions::faceup_idle()));
    }
    let lore_before = state.player(me).unwrap().lore();

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: rose,
            shift_onto: None,
        },
    )
    .expect("play The White Rose");

    assert_eq!(
        state.player(me).unwrap().lore(),
        lore_before + 1,
        "gained 1 lore on play"
    );
}
