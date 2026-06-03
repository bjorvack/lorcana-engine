//! Integration tests for start/end-of-turn triggers (§4.2.2.3, §4.4.1) and the
//! turn-progression-with-suspension machinery: a trigger that needs a decision
//! pauses the turn transition, and answering it resumes the remaining steps.

use lorcana_engine::{
    Amount, CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterFilter,
    CharacterStats, Conditions, Decision, DelayedWhen, Effect, GameState, GameStatus, Input, Phase,
    PlayerId, PlayerScope, Target, TargetSide, TriggerCondition, TriggeredAbility, apply, start,
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

fn place(state: &mut GameState, owner: PlayerId, raw: u32, def: u32) -> CardId {
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
    inst.set_stats(Some(CharacterStats::new(1, 3, 1)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn lore(state: &GameState, player: PlayerId) -> u32 {
    state.player(player).unwrap().lore()
}

fn end_turn(state: &mut GameState, reg: &CardRegistry) {
    let _ = apply(state, reg, Input::EndTurn).expect("end turn");
}

#[test]
fn a_start_of_turn_trigger_fires_when_the_turn_comes_around() {
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 3, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::AtStartOfTurn,
                Effect::Lore {
                    who: PlayerScope::You,
                    amount: Amount::fixed(2),
                },
            ),
        ]),
    );
    let mut state = started(&reg);
    let a = state.active_player();
    let _ = place(&mut state, a, 1000, 100);

    end_turn(&mut state, &reg); // A -> B
    end_turn(&mut state, &reg); // B -> A: A's start-of-turn trigger fires in the Set step

    assert_eq!(state.active_player(), a);
    assert_eq!(lore(&state, a), 2, "start-of-turn trigger resolved");
    assert_eq!(state.phase(), Phase::Main, "turn reached the Main phase");
}

#[test]
fn an_end_of_turn_trigger_fires_before_the_turn_passes() {
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 3, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::AtEndOfTurn,
                Effect::Lore {
                    who: PlayerScope::You,
                    amount: Amount::fixed(2),
                },
            ),
        ]),
    );
    let mut state = started(&reg);
    let a = state.active_player();
    let _ = place(&mut state, a, 1000, 100);

    end_turn(&mut state, &reg);

    assert_eq!(lore(&state, a), 2, "end-of-turn trigger resolved");
    assert_ne!(
        state.active_player(),
        a,
        "the turn passed to the next player"
    );
    assert_eq!(state.phase(), Phase::Main);
}

#[test]
fn a_suspending_start_of_turn_trigger_pauses_then_resumes_the_turn() {
    let mut reg = CardRegistry::new();
    // Optional ("you may") start-of-turn trigger — it suspends on a MayResolve.
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 3, 1).with_abilities(vec![
            TriggeredAbility::optional(
                TriggerCondition::AtStartOfTurn,
                Effect::Lore {
                    who: PlayerScope::You,
                    amount: Amount::fixed(2),
                },
            ),
        ]),
    );
    let mut state = started(&reg);
    let a = state.active_player();
    let _ = place(&mut state, a, 1000, 100);

    end_turn(&mut state, &reg); // A -> B
    end_turn(&mut state, &reg); // B -> A: the "may" trigger suspends the turn in the Set step

    assert!(
        state.is_awaiting_decision(),
        "paused on the may-resolve decision"
    );
    assert_eq!(state.phase(), Phase::Beginning, "still mid-Beginning phase");
    assert_eq!(lore(&state, a), 0);

    let _ = apply(&mut state, &reg, Input::Decide(Decision::May(true))).expect("resolve may");

    assert!(!state.is_awaiting_decision());
    assert_eq!(
        state.phase(),
        Phase::Main,
        "the turn resumed to the Main phase"
    );
    assert_eq!(lore(&state, a), 2, "the trigger applied after the decision");
}

#[test]
fn a_delayed_end_of_turn_trigger_fires_at_the_end_of_the_turn() {
    let mut reg = CardRegistry::new();
    // "Whenever this quests, at the end of this turn, gain 2 lore."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 3, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::ScheduleDelayed {
                    when: DelayedWhen::EndOfTurn,
                    effect: Box::new(Effect::Lore {
                        who: PlayerScope::You,
                        amount: Amount::fixed(2),
                    }),
                },
            ),
        ]),
    );
    let mut state = started(&reg);
    let a = state.active_player();
    let q = place(&mut state, a, 1000, 100);

    let _ = apply(&mut state, &reg, Input::Quest { character: q }).expect("quest");
    assert_eq!(
        lore(&state, a),
        1,
        "only the quest lore so far; delayed not yet fired"
    );

    end_turn(&mut state, &reg);
    assert_eq!(
        lore(&state, a),
        3,
        "the delayed effect fired at end of turn (+2)"
    );
}

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != player)
        .unwrap()
}

fn ready(state: &GameState, owner: PlayerId, card: CardId) -> bool {
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

fn exert(state: &mut GameState, owner: PlayerId, card: CardId) {
    state
        .player_mut(owner)
        .unwrap()
        .play_mut()
        .iter_mut()
        .find(|c| c.id() == card)
        .unwrap()
        .conditions_mut()
        .ready = false;
}

#[test]
fn freeze_keeps_a_character_exerted_through_its_next_ready_step_then_readies() {
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 3, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::Freeze(Target::ChosenCharacter {
                    filter: CharacterFilter::any(TargetSide::Opposing),
                }),
            ),
        ]),
    );
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(200),
        1,
        true,
        1,
        3,
        1,
    ));
    let mut state = started(&reg);
    let a = state.active_player();
    let b = opponent_of(&state, a);
    let quester = place(&mut state, a, 1000, 100);
    let victim = place(&mut state, b, 2000, 200);
    exert(&mut state, b, victim); // it quested last turn, say

    // Freeze the victim.
    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(victim)),
    )
    .expect("freeze");

    // B's next turn: the ready step must NOT ready the frozen victim.
    end_turn(&mut state, &reg);
    assert_eq!(state.active_player(), b);
    assert!(
        !ready(&state, b, victim),
        "frozen: stays exerted through its ready step"
    );

    // The freeze is one-shot: on B's following turn it readies normally.
    end_turn(&mut state, &reg); // B -> A
    end_turn(&mut state, &reg); // A -> B again
    assert_eq!(state.active_player(), b);
    assert!(
        ready(&state, b, victim),
        "freeze was consumed; victim readies"
    );
}
