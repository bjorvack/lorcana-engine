//! Integration tests for Slice 5a: activated abilities — paying the exert / ink
//! cost and resolving the effect immediately (§7.5).

use lorcana_engine::{
    AbilityCost, ActivatedAbility, Amount, CardDefId, CardDefinition, CardId, CardInstance,
    CardRegistry, CharacterStats, Conditions, Effect, GameEvent, GameState, GameStatus, Input,
    PlayerId, PlayerScope, apply, start,
};

fn two_decks(size: u32) -> Vec<Vec<CardDefId>> {
    vec![
        (0..size).map(CardDefId::from_raw).collect(),
        (0..size).map(CardDefId::from_raw).collect(),
    ]
}

fn started(registry: &CardRegistry) -> GameState {
    let mut state = GameState::new(two_decks(30), 7);
    let _ = start(&mut state).expect("start");
    while let GameStatus::AwaitingMulligan(player) = *state.status() {
        let _ = apply(
            &mut state,
            registry,
            Input::Mulligan {
                player,
                put_back: Vec::new(),
            },
        )
        .expect("mulligan");
    }
    state
}

/// A character in play (dry/faceup) with the given readiness, referencing `def`.
fn place_character(state: &mut GameState, owner: PlayerId, raw: u32, def: CardDefId, ready: bool) {
    let mut instance = CardInstance::new(
        CardId::from_raw(raw),
        def,
        Conditions {
            ready,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(2, 3, 1)));
    state.player_mut(owner).unwrap().play_mut().push(instance);
}

/// Put a ready ink card into a player's inkwell.
fn place_ink(state: &mut GameState, owner: PlayerId, raw: u32) {
    let instance = CardInstance::new(
        CardId::from_raw(raw),
        CardDefId::from_raw(raw),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    state
        .player_mut(owner)
        .unwrap()
        .inkwell_mut()
        .push(instance);
}

fn ready_of(state: &GameState, owner: PlayerId, card: CardId) -> bool {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .unwrap()
        .conditions()
        .ready
}

#[test]
fn exert_ability_draws_and_exerts_the_source() {
    let def = CardDefId::from_raw(1000);
    let mut registry = CardRegistry::new();
    registry.insert(
        CardDefinition::character(def, 1, true, 2, 3, 1).with_activated(vec![
            ActivatedAbility::new(
                AbilityCost::exert(),
                Effect::Draw {
                    who: PlayerScope::You,
                    amount: Amount::fixed(1),
                },
            ),
        ]),
    );
    let mut state = started(&registry);
    let active = state.active_player();
    let source = CardId::from_raw(2000);
    place_character(&mut state, active, 2000, def, true);

    let hand_before = state.player(active).unwrap().hand().len();
    let events = apply(
        &mut state,
        &registry,
        Input::UseAbility {
            card: source,
            ability: 0,
        },
    )
    .expect("use ability");

    assert_eq!(state.player(active).unwrap().hand().len(), hand_before + 1);
    assert!(
        !ready_of(&state, active, source),
        "exert cost exerts the source"
    );
    assert!(events.contains(&GameEvent::AbilityActivated {
        player: active,
        card: source,
    }));
}

#[test]
fn ink_ability_pays_ink() {
    let def = CardDefId::from_raw(1001);
    let mut registry = CardRegistry::new();
    registry.insert(
        CardDefinition::character(def, 1, true, 2, 3, 1).with_activated(vec![
            ActivatedAbility::new(
                AbilityCost::new(false, 1),
                Effect::Lore {
                    who: PlayerScope::You,
                    amount: Amount::fixed(1),
                },
            ),
        ]),
    );
    let mut state = started(&registry);
    let active = state.active_player();
    let source = CardId::from_raw(2001);
    place_character(&mut state, active, 2001, def, true);
    place_ink(&mut state, active, 3001);

    let _ = apply(
        &mut state,
        &registry,
        Input::UseAbility {
            card: source,
            ability: 0,
        },
    )
    .expect("use ability");

    assert_eq!(state.player(active).unwrap().lore(), 1);
    assert_eq!(
        state.player(active).unwrap().ready_ink(),
        0,
        "ink was spent"
    );
    assert!(
        ready_of(&state, active, source),
        "no exert cost, source stays ready"
    );
}

#[test]
fn insufficient_ink_is_rejected() {
    let def = CardDefId::from_raw(1001);
    let mut registry = CardRegistry::new();
    registry.insert(
        CardDefinition::character(def, 1, true, 2, 3, 1).with_activated(vec![
            ActivatedAbility::new(
                AbilityCost::new(false, 1),
                Effect::Lore {
                    who: PlayerScope::You,
                    amount: Amount::fixed(1),
                },
            ),
        ]),
    );
    let mut state = started(&registry);
    let active = state.active_player();
    place_character(&mut state, active, 2001, def, true); // no ink seeded

    let result = apply(
        &mut state,
        &registry,
        Input::UseAbility {
            card: CardId::from_raw(2001),
            ability: 0,
        },
    );
    assert!(result.is_err());
    assert_eq!(state.player(active).unwrap().lore(), 0);
}

#[test]
fn drying_or_exerted_source_cannot_pay_an_exert_cost() {
    let def = CardDefId::from_raw(1000);
    let mut registry = CardRegistry::new();
    registry.insert(
        CardDefinition::character(def, 1, true, 2, 3, 1).with_activated(vec![
            ActivatedAbility::new(
                AbilityCost::exert(),
                Effect::Draw {
                    who: PlayerScope::You,
                    amount: Amount::fixed(1),
                },
            ),
        ]),
    );
    let mut state = started(&registry);
    let active = state.active_player();

    // Exerted source.
    place_character(&mut state, active, 2000, def, false);
    assert!(
        apply(
            &mut state,
            &registry,
            Input::UseAbility {
                card: CardId::from_raw(2000),
                ability: 0,
            },
        )
        .is_err()
    );

    // Drying source.
    let mut drying = CardInstance::new(
        CardId::from_raw(2002),
        def,
        Conditions {
            ready: true,
            damage: 0,
            drying: true,
            facedown: false,
        },
    );
    drying.set_stats(Some(CharacterStats::new(2, 3, 1)));
    state.player_mut(active).unwrap().play_mut().push(drying);
    assert!(
        apply(
            &mut state,
            &registry,
            Input::UseAbility {
                card: CardId::from_raw(2002),
                ability: 0,
            },
        )
        .is_err()
    );
}

#[test]
fn unknown_ability_index_is_rejected() {
    let def = CardDefId::from_raw(1000);
    let mut registry = CardRegistry::new();
    registry.insert(CardDefinition::character(def, 1, true, 2, 3, 1));
    let mut state = started(&registry);
    let active = state.active_player();
    place_character(&mut state, active, 2000, def, true);

    let result = apply(
        &mut state,
        &registry,
        Input::UseAbility {
            card: CardId::from_raw(2000),
            ability: 0,
        },
    );
    assert!(result.is_err(), "no activated ability at index 0");
}
