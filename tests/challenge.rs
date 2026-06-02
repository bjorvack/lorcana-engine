//! Integration tests for Slice 3: challenge resolution (mutual damage) and
//! banishment when damage reaches willpower.
//!
//! Characters are placed directly into play (via the public API) so a challenge
//! can be exercised without playing through several turns.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, Conditions,
    Effect, GameEvent, GameState, GameStatus, Input, PlayerId, TriggerCondition, TriggeredAbility,
    apply, start,
};

/// Start a game (skipping mulligans) using the given registry.
fn started_with(registry: &CardRegistry) -> GameState {
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

fn two_decks(size: u32) -> Vec<Vec<CardDefId>> {
    vec![
        (0..size).map(CardDefId::from_raw).collect(),
        (0..size).map(CardDefId::from_raw).collect(),
    ]
}

/// Start a game and skip both mulligans, leaving it in `Playing`/Main. The
/// registry is unused by challenges, so an empty one is fine.
fn started_game() -> (GameState, CardRegistry) {
    let registry = CardRegistry::new();
    let mut state = GameState::new(two_decks(30), 7);
    let _ = start(&mut state).expect("start");
    while let GameStatus::AwaitingMulligan(player) = *state.status() {
        let _ = apply(
            &mut state,
            &registry,
            Input::Mulligan {
                player,
                put_back: Vec::new(),
            },
        )
        .expect("mulligan");
    }
    (state, registry)
}

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|id| *id != player)
        .expect("a second player exists")
}

/// Place a character into `owner`'s play area with the given stats and
/// readiness (dry, faceup), returning its instance id.
fn place_character(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    strength: u32,
    willpower: u32,
    ready: bool,
) -> CardId {
    let id = CardId::from_raw(raw);
    let conditions = Conditions {
        ready,
        damage: 0,
        drying: false,
        facedown: false,
    };
    let mut instance = CardInstance::new(id, CardDefId::from_raw(raw), conditions);
    instance.set_stats(Some(CharacterStats::new(strength, willpower, 1)));
    state.player_mut(owner).unwrap().play_mut().push(instance);
    id
}

fn damage_of(state: &GameState, owner: PlayerId, card: CardId) -> Option<u32> {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .map(|c| c.conditions().damage)
}

#[test]
fn challenge_deals_mutual_damage() {
    let (mut state, registry) = started_game();
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place_character(&mut state, active, 1000, 2, 3, true);
    let target = place_character(&mut state, foe, 1001, 1, 3, false);

    let events = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    )
    .expect("challenge");

    // Challenger dealt 2 to target; target dealt 1 back. Neither is lethal.
    assert_eq!(damage_of(&state, foe, target), Some(2));
    assert_eq!(damage_of(&state, active, challenger), Some(1));
    // Challenger is now exerted.
    let c = state
        .player(active)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == challenger)
        .unwrap();
    assert!(!c.conditions().ready);
    assert!(events.contains(&GameEvent::Challenged {
        player: active,
        challenger,
        target,
    }));
}

#[test]
fn lethal_challenge_banishes_the_target() {
    let (mut state, registry) = started_game();
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place_character(&mut state, active, 1000, 3, 3, true);
    let target = place_character(&mut state, foe, 1001, 1, 3, false);

    let events = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    )
    .expect("challenge");

    // Target had 3 willpower and took 3 → banished to discard.
    assert!(damage_of(&state, foe, target).is_none());
    assert!(state.player(foe).unwrap().discard().contains(target));
    assert!(events.contains(&GameEvent::Banished {
        player: foe,
        card: target,
    }));
    // Challenger survives with 1 damage.
    assert_eq!(damage_of(&state, active, challenger), Some(1));
}

#[test]
fn a_trade_banishes_both_characters() {
    let (mut state, registry) = started_game();
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place_character(&mut state, active, 1000, 3, 2, true);
    let target = place_character(&mut state, foe, 1001, 2, 2, false);

    let _ = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    )
    .expect("challenge");

    assert!(state.player(active).unwrap().play().is_empty());
    assert!(state.player(foe).unwrap().play().is_empty());
    assert!(state.player(active).unwrap().discard().contains(challenger));
    assert!(state.player(foe).unwrap().discard().contains(target));
}

#[test]
fn zero_strength_target_deals_no_damage() {
    let (mut state, registry) = started_game();
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place_character(&mut state, active, 1000, 2, 3, true);
    let target = place_character(&mut state, foe, 1001, 0, 4, false);

    let _ = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    )
    .expect("challenge");

    assert_eq!(damage_of(&state, active, challenger), Some(0));
    assert_eq!(damage_of(&state, foe, target), Some(2));
}

#[test]
fn cannot_challenge_a_ready_character() {
    let (mut state, registry) = started_game();
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place_character(&mut state, active, 1000, 2, 3, true);
    let target = place_character(&mut state, foe, 1001, 1, 3, true); // ready

    let result = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    );
    assert!(result.is_err());
    // No mutation: challenger still ready, target undamaged.
    assert_eq!(damage_of(&state, foe, target), Some(0));
}

#[test]
fn cannot_challenge_with_a_drying_character() {
    let (mut state, registry) = started_game();
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    // Challenger placed drying.
    let id = CardId::from_raw(1000);
    let mut instance = CardInstance::new(
        id,
        CardDefId::from_raw(1000),
        Conditions {
            ready: true,
            damage: 0,
            drying: true,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(2, 3, 1)));
    state.player_mut(active).unwrap().play_mut().push(instance);
    let target = place_character(&mut state, foe, 1001, 1, 3, false);

    let result = apply(
        &mut state,
        &registry,
        Input::Challenge {
            challenger: id,
            target,
        },
    );
    assert!(result.is_err());
}

#[test]
fn cannot_challenge_your_own_character() {
    let (mut state, registry) = started_game();
    let active = state.active_player();

    let challenger = place_character(&mut state, active, 1000, 2, 3, true);
    let own = place_character(&mut state, active, 1001, 1, 3, false);

    let result = apply(
        &mut state,
        &registry,
        Input::Challenge {
            challenger,
            target: own,
        },
    );
    assert!(
        result.is_err(),
        "a player cannot challenge their own character"
    );
}

#[test]
fn challenge_triggers_fire_for_challenger_and_target() {
    let mut registry = CardRegistry::new();
    // Challenger: "whenever this character challenges, gain 1 lore."
    registry.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 3, 1).with_abilities(vec![
            TriggeredAbility::new(TriggerCondition::WhenThisChallenges, Effect::GainLore(1)),
        ]),
    );
    // Target: "whenever this character is challenged, its controller gains 2 lore."
    registry.insert(
        CardDefinition::character(CardDefId::from_raw(200), 1, true, 1, 9, 1).with_abilities(vec![
            TriggeredAbility::new(TriggerCondition::WhenChallenged, Effect::GainLore(2)),
        ]),
    );
    let mut state = started_with(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let challenger = place_character(&mut state, active, 100, 2, 3, true);
    let target = place_character(&mut state, foe, 200, 1, 9, false); // exerted, high willpower

    let _ = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    )
    .expect("challenge");

    assert_eq!(
        state.player(active).unwrap().lore(),
        1,
        "challenger's trigger fired"
    );
    assert_eq!(
        state.player(foe).unwrap().lore(),
        2,
        "challenged character's trigger fired"
    );
}
