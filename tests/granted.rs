//! Integration test for Slice 8c: granting a triggered ability to a character
//! ("gains 'Whenever this character quests, …' this turn", §7.6).

use lorcana_engine::{
    Amount, CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterFilter,
    CharacterStats, Conditions, Decision, Effect, GameState, GameStatus, Input, PlayerId,
    PlayerScope, Target, TargetSide, TriggerCondition, TriggeredAbility, apply, start,
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

fn place(state: &mut GameState, owner: PlayerId, def: u32) -> CardId {
    let id = state.allocate_card_id();
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
    inst.set_stats(Some(CharacterStats::new(1, 5, 1)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn lore(state: &GameState, player: PlayerId) -> u32 {
    state.player(player).unwrap().lore()
}

#[test]
fn granted_quest_trigger_fires_when_the_target_quests() {
    let mut reg = CardRegistry::new();
    // Granter: "Whenever this quests, chosen character gains 'Whenever this
    // character quests, you gain 2 lore' this turn."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::GrantAbilityThisTurn {
                    target: Target::ChosenCharacter {
                        filter: CharacterFilter::any(TargetSide::Yours)
                            .and(CharacterFilter::negate(CharacterFilter::IsSource)),
                    },
                    condition: TriggerCondition::WhenThisQuests,
                    effect: Box::new(Effect::Lore {
                        who: PlayerScope::You,
                        amount: Amount::fixed(2),
                    }),
                    optional: false,
                },
            ),
        ]),
    );
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(200),
        1,
        true,
        1,
        5,
        1,
    ));
    for n in 0..30 {
        if reg.get(CardDefId::from_raw(n)).is_none() {
            reg.insert(CardDefinition::character(
                CardDefId::from_raw(n),
                1,
                true,
                1,
                1,
                1,
            ));
        }
    }
    let mut state = started(&reg);
    let me = state.active_player();
    let granter = place(&mut state, me, 100);
    let ally = place(&mut state, me, 200);

    // Granter quests and grants the ability to the ally.
    let _ = apply(&mut state, &reg, Input::Quest { character: granter }).expect("quest granter");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(ally)),
    )
    .expect("grant");

    // The ally quests: its own lore (1) plus the granted "+2 lore" trigger.
    let before = lore(&state, me);
    let _ = apply(&mut state, &reg, Input::Quest { character: ally }).expect("quest ally");
    assert_eq!(
        lore(&state, me),
        before + 1 + 2,
        "ally's quest lore plus the granted trigger"
    );
}

#[test]
fn granted_activated_ability_can_be_used_this_turn() {
    let mut reg = CardRegistry::new();
    // Granter: "Whenever this quests, chosen character gains '{E} — you gain 2
    // lore' this turn." (§7.5)
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::GrantActivatedThisTurn {
                    target: Target::ChosenCharacter {
                        filter: CharacterFilter::any(TargetSide::Yours)
                            .and(CharacterFilter::negate(CharacterFilter::IsSource)),
                    },
                    ink: 0,
                    exert_self: true,
                    effect: Box::new(Effect::Lore {
                        who: PlayerScope::You,
                        amount: Amount::fixed(2),
                    }),
                },
            ),
        ]),
    );
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(200),
        1,
        true,
        1,
        5,
        1,
    ));
    let mut state = started(&reg);
    let me = state.active_player();
    let granter = place(&mut state, me, 100);
    let ally = place(&mut state, me, 200); // ready, not drying

    let _ = apply(&mut state, &reg, Input::Quest { character: granter }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(ally)),
    )
    .expect("grant");

    let before = lore(&state, me);
    // The ally now has a single (granted) activated ability at index 0.
    let _ = apply(
        &mut state,
        &reg,
        Input::UseAbility {
            card: ally,
            ability: 0,
        },
    )
    .expect("activate");
    assert_eq!(
        lore(&state, me),
        before + 2,
        "granted exert ability resolved"
    );
    assert!(
        !state
            .player(me)
            .unwrap()
            .play()
            .iter()
            .find(|c| c.id() == ally)
            .unwrap()
            .conditions()
            .ready,
        "activating exerted the ally"
    );
}
