//! Integration tests for Slice 5g: game-rule static modifiers (the win/loss
//! modification layer) — Donald Duck's "opponents need 25 lore to win", and the
//! threshold reverting the instant his static leaves play.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterFilter, CharacterStats,
    Conditions, Decision, Effect, GameState, GameStatus, Input, PlayerId, RequiredAction,
    RuleModifier, Target, TargetSide, TriggerCondition, TriggeredAbility, apply, check_win_loss,
    game_state_check, lore_to_win, start,
};

fn started_with(registry: &CardRegistry) -> GameState {
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

fn started() -> GameState {
    started_with(&CardRegistry::new())
}

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != player)
        .unwrap()
}

#[test]
fn donald_raises_opponents_threshold_to_25() {
    let mut state = started();
    let donald_controller = state.active_player();
    let foe = opponent_of(&state, donald_controller);

    // Donald's static: opponents need 25 lore to win.
    state.add_rule_modifier(RuleModifier::LoreToWin {
        source: CardId::from_raw(1000),
        player: foe,
        threshold: 25,
    });

    // Controller's own threshold is unchanged.
    assert_eq!(lore_to_win(&state, donald_controller), 20);
    assert_eq!(lore_to_win(&state, foe), 25);

    // Opponent at 24 does not win; at 25 wins.
    state.player_mut(foe).unwrap().add_lore(24);
    assert!(check_win_loss(&state).is_empty());
    state.player_mut(foe).unwrap().add_lore(1);
    assert_eq!(
        check_win_loss(&state),
        vec![RequiredAction::PlayerWins(foe)]
    );
}

#[test]
fn threshold_reverts_when_donald_leaves_play_and_a_pending_win_resolves() {
    let mut state = started();
    let donald_controller = state.active_player();
    let foe = opponent_of(&state, donald_controller);

    // Donald is in play with lethal damage already on him (willpower 1, 1 damage).
    let donald = CardId::from_raw(1000);
    let mut instance = CardInstance::new(
        donald,
        CardDefId::from_raw(1000),
        Conditions {
            ready: true,
            damage: 1,
            drying: false,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(1, 1, 1)));
    state
        .player_mut(donald_controller)
        .unwrap()
        .play_mut()
        .push(instance);
    state.add_rule_modifier(RuleModifier::LoreToWin {
        source: donald,
        player: foe,
        threshold: 25,
    });

    // The opponent sits at 20: not enough to win while Donald holds it at 25.
    state.player_mut(foe).unwrap().add_lore(20);
    assert!(
        check_win_loss(&state).is_empty(),
        "20 < 25 while Donald is in play"
    );

    // A game-state check banishes the lethal Donald; his override ends, so on the
    // next pass the opponent's 20 lore meets the reverted threshold and they win.
    let _ = game_state_check(&mut state);

    assert!(state.player(foe).unwrap().play().is_empty());
    assert!(
        state
            .player(donald_controller)
            .unwrap()
            .discard()
            .contains(donald)
    );
    let GameStatus::Finished { winners } = state.status() else {
        panic!("the opponent should win once Donald's override is gone");
    };
    assert_eq!(winners, &vec![foe]);
}

#[test]
fn an_effect_banishing_donald_resolves_the_opponents_pending_win() {
    // A quester that, on quest, banishes another chosen character of yours.
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::Banish(Target::ChosenCharacter {
                    filter: CharacterFilter::any(TargetSide::Yours)
                        .and(CharacterFilter::negate(CharacterFilter::IsSource)),
                }),
            ),
        ]),
    );
    let mut state = started_with(&reg);
    let a = state.active_player();
    let foe = opponent_of(&state, a);

    // A's quester + A's Donald (raising foe's threshold to 25).
    let quester = CardId::from_raw(1);
    let donald = CardId::from_raw(1000);
    for (id, def) in [(quester, 100), (donald, 1000)] {
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
        inst.set_stats(Some(CharacterStats::new(2, 5, 1)));
        state.player_mut(a).unwrap().play_mut().push(inst);
    }
    state.add_rule_modifier(RuleModifier::LoreToWin {
        source: donald,
        player: foe,
        threshold: 25,
    });

    // Foe sits at 20 — not enough while Donald holds the threshold at 25.
    state.player_mut(foe).unwrap().add_lore(20);
    assert!(check_win_loss(&state).is_empty());

    // A quests and banishes Donald by effect; his override ends and the
    // game-state check (run after the effect) resolves the foe's pending win.
    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(donald)),
    )
    .expect("banish");

    let GameStatus::Finished { winners } = state.status() else {
        panic!("the opponent should win once Donald's override is removed by the effect");
    };
    assert_eq!(winners, &vec![foe]);
}
