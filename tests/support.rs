//! Integration tests for Slice 8a-2: the Support keyword (§10.13) and the
//! choose-a-target machinery — a quest may add the Support character's current
//! `{S}` to another chosen character until end of turn.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, Conditions,
    Decision, GameState, GameStatus, Input, Keyword, ModifierDuration, ModifierTarget, PlayerId,
    Stat, StatModifier, apply, start,
};

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

fn place(state: &mut GameState, owner: PlayerId, raw: u32, def: u32, strength: u32) -> CardId {
    let id = CardId::from_raw(raw);
    let mut inst = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    inst.set_stats(Some(CharacterStats::new(strength, 5, 1)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn strength(state: &GameState, card: CardId) -> u32 {
    state.current_character_stats(card).unwrap().strength
}

/// Registry: a Support character (def 100) and a plain character (def 200).
fn registry() -> CardRegistry {
    let mut r = CardRegistry::new();
    r.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 3, 5, 1)
            .with_keywords(vec![Keyword::Support]),
    );
    r.insert(CardDefinition::character(
        CardDefId::from_raw(200),
        1,
        true,
        2,
        5,
        1,
    ));
    r
}

#[test]
fn support_adds_its_strength_to_a_chosen_character_until_end_of_turn() {
    let reg = registry();
    let mut state = started(&reg);
    let active = state.active_player();
    let supporter = place(&mut state, active, 1000, 100, 3); // Support, {S} 3
    let ally = place(&mut state, active, 1001, 200, 2);

    let _ = apply(
        &mut state,
        &reg,
        Input::Quest {
            character: supporter,
        },
    )
    .expect("quest");
    // "you may" — accept.
    assert!(state.is_awaiting_decision());
    let _ = apply(&mut state, &reg, Input::Decide(Decision::May(true))).expect("may");
    // Then choose the target.
    assert!(state.is_awaiting_decision());
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(ally)),
    )
    .expect("choose target");

    assert_eq!(
        strength(&state, ally),
        2 + 3,
        "ally gained the supporter's {{S}}"
    );
    assert!(!state.is_awaiting_decision());
}

#[test]
fn support_uses_the_supporters_modified_strength() {
    let reg = registry();
    let mut state = started(&reg);
    let active = state.active_player();
    let supporter = place(&mut state, active, 1000, 100, 3);
    let ally = place(&mut state, active, 1001, 200, 2);
    // Buff the supporter by +2 (now {S} 5); Support should add the modified value.
    state.add_modifier(StatModifier::new(
        CardId::from_raw(9999),
        ModifierTarget::Card(supporter),
        Stat::Strength,
        2,
        ModifierDuration::WhileSourceInPlay,
    ));
    assert_eq!(strength(&state, supporter), 5);

    let _ = apply(
        &mut state,
        &reg,
        Input::Quest {
            character: supporter,
        },
    )
    .expect("quest");
    let _ = apply(&mut state, &reg, Input::Decide(Decision::May(true))).expect("may");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(ally)),
    )
    .expect("choose target");

    assert_eq!(
        strength(&state, ally),
        2 + 5,
        "added the supporter's modified {{S}}"
    );
}

#[test]
fn declining_support_does_nothing() {
    let reg = registry();
    let mut state = started(&reg);
    let active = state.active_player();
    let supporter = place(&mut state, active, 1000, 100, 3);
    let ally = place(&mut state, active, 1001, 200, 2);

    let _ = apply(
        &mut state,
        &reg,
        Input::Quest {
            character: supporter,
        },
    )
    .expect("quest");
    let _ = apply(&mut state, &reg, Input::Decide(Decision::May(false))).expect("decline");

    assert_eq!(
        strength(&state, ally),
        2,
        "no buff when Support is declined"
    );
    assert!(!state.is_awaiting_decision());
}

#[test]
fn chained_support_adds_the_combined_strength() {
    // A (Support, {S} 1) buffs B (Support, {S} 2) -> B is now {S} 3. When B then
    // quests, B's Support adds B's *combined* {S} (3) to C (§10.13, §7.8).
    let reg = registry();
    let mut state = started(&reg);
    let active = state.active_player();
    let a = place(&mut state, active, 1000, 100, 1); // Support {S} 1
    let b = place(&mut state, active, 1001, 100, 2); // Support {S} 2
    let c = place(&mut state, active, 1002, 200, 0); // plain {S} 0

    // A quests and supports B.
    let _ = apply(&mut state, &reg, Input::Quest { character: a }).expect("quest a");
    let _ = apply(&mut state, &reg, Input::Decide(Decision::May(true))).expect("may a");
    let _ = apply(&mut state, &reg, Input::Decide(Decision::ChooseTarget(b))).expect("a -> b");
    assert_eq!(strength(&state, b), 3, "B now has its base 2 plus A's 1");

    // B quests and supports C with B's *current* (combined) {S} of 3.
    let _ = apply(&mut state, &reg, Input::Quest { character: b }).expect("quest b");
    let _ = apply(&mut state, &reg, Input::Decide(Decision::May(true))).expect("may b");
    let _ = apply(&mut state, &reg, Input::Decide(Decision::ChooseTarget(c))).expect("b -> c");
    assert_eq!(strength(&state, c), 3, "C gains B's combined {{S}} (2 + 1)");
}
