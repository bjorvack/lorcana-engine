//! Rules conformance (§7–§10): golden tests citing the comprehensive rules
//! (`docs/rules/`). Every card here is authored in the engine's **TOML DSL** and
//! loaded via `load_toml`, so each test also proves the loader/DSL produce
//! rules-correct cards end-to-end.

use lorcana_engine::{
    CardDefId, CardId, CardInstance, CardRegistry, CharacterStats, Conditions, GameState,
    GameStatus, Input, PlayerId, apply, load_toml, start,
};

fn registry_from(toml: &str) -> CardRegistry {
    let mut reg = CardRegistry::new();
    for def in load_toml(toml).expect("cards load") {
        reg.insert(def);
    }
    reg
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

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != player)
        .unwrap()
}

fn place(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    def: u32,
    strength: u32,
    willpower: u32,
    ready: bool,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut instance = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(strength, willpower, 1)));
    state.player_mut(owner).unwrap().play_mut().push(instance);
    id
}

fn damage(state: &GameState, owner: PlayerId, card: CardId) -> Option<u32> {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .map(|c| c.conditions().damage)
}

fn lore(state: &GameState, p: PlayerId) -> u32 {
    state.player(p).unwrap().lore()
}

fn hand_len(state: &GameState, p: PlayerId) -> usize {
    state.player(p).unwrap().hand().iter().count()
}

/// §10.8 — "Resist +N": damage dealt to this character is reduced by N.
#[test]
fn resist_reduces_challenge_damage() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Striker"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        [[card]]
        name = "Tank"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        keywords = ["Resist 2"]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let striker = place(&mut state, me, 100, 0, 3, 9, true);
    let tank = place(&mut state, foe, 200, 1, 1, 9, false);

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: striker,
            target: tank,
        },
    )
    .expect("challenge");
    assert_eq!(
        damage(&state, foe, tank),
        Some(1),
        "3 strength − Resist 2 = 1"
    );
}

/// §10.5 — "Challenger +N": while challenging, this character gets +N {S}.
#[test]
fn challenger_adds_strength_while_challenging() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Bruiser"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        keywords = ["Challenger 2"]
        [[card]]
        name = "Dummy"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let bruiser = place(&mut state, me, 100, 0, 3, 9, true);
    let dummy = place(&mut state, foe, 200, 1, 1, 9, false);

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: bruiser,
            target: dummy,
        },
    )
    .expect("challenge");
    assert_eq!(
        damage(&state, foe, dummy),
        Some(5),
        "3 base + Challenger 2 = 5"
    );
}

/// §10.6 — "Evasive": can only be challenged by characters with Evasive.
#[test]
fn evasive_can_only_be_challenged_by_evasive() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Flyer"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        keywords = ["Evasive"]
        [[card]]
        name = "Grounded"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let target = place(&mut state, foe, 200, 0, 1, 9, false); // Evasive
    let grounded = place(&mut state, me, 100, 1, 3, 9, true);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Challenge {
                challenger: grounded,
                target,
            },
        )
        .is_err(),
        "a non-Evasive character can't challenge an Evasive one"
    );
}

/// §7.1.2 — an effect with multiple parts resolves its parts in order.
#[test]
fn an_abilitys_parts_resolve_in_order() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Scholar"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = [{ draw = 1 }, { gain_lore = 1 }]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let scholar = place(&mut state, me, 100, 0, 1, 5, true);
    let lore_before = lore(&state, me);
    let hand_before = hand_len(&state, me);

    let _ = apply(&mut state, &reg, Input::Quest { character: scholar }).expect("quest");
    assert_eq!(hand_len(&state, me), hand_before + 1, "drew a card");
    // questing lore (1) + the ability's +1.
    assert_eq!(lore(&state, me), lore_before + 1 + 1, "both parts resolved");
}
