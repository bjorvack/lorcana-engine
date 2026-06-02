//! Integration tests for Slice 6c: Shift (§10.10) — same-name / Universal /
//! [Classification] variants, state inheritance (dry/exerted/damage), the card
//! stack, and the stack dissolving when the top leaves play.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, Classification,
    Conditions, GameState, GameStatus, Input, Keyword, PlayerId, ShiftAbility, ShiftCost,
    ShiftKind, apply, start,
};

fn shift_card(def: u32, kind: ShiftKind) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(def), 5, true, 3, 4, 2)
        .with_names(vec!["Elsa".to_string()])
        .with_keywords(vec![Keyword::Shift(ShiftAbility {
            cost: ShiftCost::Ink(0),
            kind,
        })])
}

fn named(def: u32, name: &str) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(def), 5, true, 3, 4, 2)
        .with_names(vec![name.to_string()])
}

/// A registry whose deck cards (0..30) are the shifting card, plus the given
/// target definitions.
fn registry(kind: &ShiftKind, targets: Vec<CardDefinition>) -> CardRegistry {
    let mut r: CardRegistry = (0..30).map(|n| shift_card(n, kind.clone())).collect();
    for t in targets {
        r.insert(t);
    }
    r
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

fn place(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    def: u32,
    ready: bool,
    drying: bool,
    damage: u32,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut inst = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready,
            damage,
            drying,
            facedown: false,
        },
    );
    inst.set_stats(Some(CharacterStats::new(3, 4, 2)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn hand_card(state: &GameState, nth: usize) -> CardId {
    state
        .player(state.active_player())
        .unwrap()
        .hand()
        .iter()
        .nth(nth)
        .unwrap()
        .id()
}

fn top(state: &GameState, owner: PlayerId, card: CardId) -> Option<CardInstance> {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .cloned()
}

#[test]
fn same_name_shift_onto_dry_character_forms_a_stack_and_enters_dry() {
    let reg = registry(&ShiftKind::SameName, vec![named(200, "Elsa")]);
    let mut state = started(&reg);
    let active = state.active_player();
    let target = place(&mut state, active, 5000, 200, true, false, 0); // dry, ready
    let shifter = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: shifter,
            shift_onto: Some(target),
        },
    )
    .expect("shift");

    // The shifter is the in-play top; the target is under it (not a separate top).
    let inst = top(&state, active, shifter).expect("shifter is in play");
    assert!(
        top(&state, active, target).is_none(),
        "target is now under the top"
    );
    assert_eq!(inst.under().len(), 1);
    assert_eq!(inst.under()[0].id(), target);
    // It inherited the underlying dry character's state, so it can quest at once.
    assert!(!inst.conditions().drying);
    assert!(
        apply(&mut state, &reg, Input::Quest { character: shifter }).is_ok(),
        "a character shifted onto a dry one can quest immediately (§10.10.5)"
    );
}

#[test]
fn shift_onto_a_drying_character_enters_drying() {
    let reg = registry(&ShiftKind::SameName, vec![named(200, "Elsa")]);
    let mut state = started(&reg);
    let active = state.active_player();
    let target = place(&mut state, active, 5000, 200, true, true, 0); // ready but drying
    let shifter = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: shifter,
            shift_onto: Some(target),
        },
    )
    .expect("shift");

    let inst = top(&state, active, shifter).unwrap();
    assert!(
        inst.conditions().drying,
        "inherits the drying state (§10.10.5)"
    );
    assert!(
        apply(&mut state, &reg, Input::Quest { character: shifter }).is_err(),
        "a character shifted onto a drying one is still drying and can't quest"
    );
}

#[test]
fn shift_inherits_exerted_and_damage() {
    let reg = registry(&ShiftKind::SameName, vec![named(200, "Elsa")]);
    let mut state = started(&reg);
    let active = state.active_player();
    let target = place(&mut state, active, 5000, 200, false, false, 2); // exerted, 2 damage
    let shifter = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: shifter,
            shift_onto: Some(target),
        },
    )
    .expect("shift");

    let inst = top(&state, active, shifter).unwrap();
    assert!(!inst.conditions().ready, "inherits exerted (§10.10.3)");
    assert_eq!(inst.conditions().damage, 2, "inherits damage (§10.10.7)");
}

#[test]
fn shift_rejects_a_different_named_target() {
    let reg = registry(&ShiftKind::SameName, vec![named(201, "Anna")]);
    let mut state = started(&reg);
    let active = state.active_player();
    let target = place(&mut state, active, 5000, 201, true, false, 0);
    let shifter = hand_card(&state, 0);

    let result = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: shifter,
            shift_onto: Some(target),
        },
    );
    assert!(
        result.is_err(),
        "can't same-name Shift onto a differently-named character"
    );
}

#[test]
fn universal_shift_goes_onto_any_character() {
    let reg = registry(&ShiftKind::Any, vec![named(201, "Anna")]);
    let mut state = started(&reg);
    let active = state.active_player();
    let target = place(&mut state, active, 5000, 201, true, false, 0);
    let shifter = hand_card(&state, 0);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::PlayCard {
                card: shifter,
                shift_onto: Some(target),
            },
        )
        .is_ok(),
        "Universal Shift can go onto any of your characters (§10.10.9.2)"
    );
}

#[test]
fn classification_shift_requires_the_classification() {
    let kind = ShiftKind::Classification(Classification::new("Puppy"));
    let puppy = CardDefinition::character(CardDefId::from_raw(202), 5, true, 3, 4, 2)
        .with_classifications(vec![Classification::new("Puppy")]);
    let plain = named(203, "Rex");
    let reg = registry(&kind, vec![puppy, plain]);
    let mut state = started(&reg);
    let active = state.active_player();
    let puppy_target = place(&mut state, active, 5000, 202, true, false, 0);
    let plain_target = place(&mut state, active, 5001, 203, true, false, 0);

    let shifter = hand_card(&state, 0);
    // Onto a non-Puppy → rejected (no mutation, so the shifter stays in hand).
    assert!(
        apply(
            &mut state,
            &reg,
            Input::PlayCard {
                card: shifter,
                shift_onto: Some(plain_target),
            },
        )
        .is_err()
    );
    // Onto a Puppy → ok.
    assert!(
        apply(
            &mut state,
            &reg,
            Input::PlayCard {
                card: shifter,
                shift_onto: Some(puppy_target),
            },
        )
        .is_ok()
    );
}

#[test]
fn a_card_without_shift_cannot_be_shifted() {
    // Deck cards are plain (no Shift); a same-name target is in play.
    let mut reg: CardRegistry = (0..30).map(|n| named(n, "Elsa")).collect();
    reg.insert(named(200, "Elsa"));
    let mut state = started(&reg);
    let active = state.active_player();
    let target = place(&mut state, active, 5000, 200, true, false, 0);
    let card = hand_card(&state, 0);

    let result = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card,
            shift_onto: Some(target),
        },
    );
    assert!(
        result.is_err(),
        "a card with no Shift ability can't be shifted"
    );
}

#[test]
fn banishing_a_shifted_stack_dissolves_it_into_the_discard() {
    let reg = registry(&ShiftKind::SameName, vec![named(200, "Elsa")]);
    let mut state = started(&reg);
    let active = state.active_player();
    // Target carries lethal damage (>= the shifter's 4 willpower); after shift the
    // top inherits it and is banished by the game-state check.
    let target = place(&mut state, active, 5000, 200, false, false, 4);
    let shifter = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: shifter,
            shift_onto: Some(target),
        },
    )
    .expect("shift");

    let player = state.player(active).unwrap();
    assert!(player.play().is_empty(), "the lethal stack was banished");
    // The stack dissolved into TWO separate cards in the discard (§5.1.7).
    assert!(player.discard().contains(shifter));
    assert!(player.discard().contains(target));
}
