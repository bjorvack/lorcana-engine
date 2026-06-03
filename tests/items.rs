//! Integration tests for Slice 7c: items — they enter play and their activated
//! abilities can be used the turn they're played (§6.4).

use lorcana_engine::{
    AbilityCost, ActivatedAbility, Amount, CardDefId, CardDefinition, CardKind, CardRegistry,
    Effect, GameState, GameStatus, Input, PlayerId, apply, start,
};

fn item_card(id: u32) -> CardDefinition {
    CardDefinition::new(CardDefId::from_raw(id), 0, true, CardKind::Item).with_activated(vec![
        ActivatedAbility::new(
            AbilityCost::new(false, 0),
            Effect::GainLore(Amount::fixed(1)),
        ),
    ])
}

fn started(reg: &CardRegistry) -> GameState {
    let mut state = GameState::new(
        vec![
            (0..30).map(CardDefId::from_raw).collect(),
            (0..30).map(CardDefId::from_raw).collect(),
        ],
        7,
    );
    let _ = start(&mut state).expect("start");
    while let GameStatus::AwaitingMulligan(player) = *state.status() {
        let _ = apply(
            &mut state,
            reg,
            Input::Mulligan {
                player,
                put_back: Vec::new(),
            },
        )
        .expect("mulligan");
    }
    state
}

fn hand_card(state: &GameState, nth: usize) -> lorcana_engine::CardId {
    state
        .player(state.active_player())
        .unwrap()
        .hand()
        .iter()
        .nth(nth)
        .unwrap()
        .id()
}

fn lore(state: &GameState, player: PlayerId) -> u32 {
    state.player(player).unwrap().lore()
}

#[test]
fn an_item_enters_play_and_is_neither_character_nor_location() {
    let reg: CardRegistry = (0..30).map(item_card).collect();
    let mut state = started(&reg);
    let active = state.active_player();
    let card = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card,
            shift_onto: None,
        },
    )
    .expect("play item");

    let inst = state
        .player(active)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .expect("item is in play");
    assert!(!inst.is_character());
    assert!(!inst.is_location());
}

#[test]
fn an_items_ability_can_be_used_the_turn_it_is_played() {
    let reg: CardRegistry = (0..30).map(item_card).collect();
    let mut state = started(&reg);
    let active = state.active_player();
    let card = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card,
            shift_onto: None,
        },
    )
    .expect("play item");
    let _ =
        apply(&mut state, &reg, Input::UseAbility { card, ability: 0 }).expect("use item ability");

    assert_eq!(
        lore(&state, active),
        1,
        "the item's ability resolved (§6.4.3)"
    );
}
