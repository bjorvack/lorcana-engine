//! Integration tests for Slice 8b: targeted damage effects (deal / remove
//! damage to a chosen character) and the centralized "when banished" trigger for
//! effect-driven banishment.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterFilter, CharacterStats,
    Conditions, Decision, Effect, GameState, GameStatus, Input, NumericFilter, PlayerId, Target,
    TargetSide, TriggerCondition, TriggeredAbility, apply, start,
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

fn place(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    def: u32,
    willpower: u32,
    damage: u32,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut inst = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready: true,
            damage,
            drying: false,
            facedown: false,
        },
    );
    inst.set_stats(Some(CharacterStats::new(2, willpower, 1)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != player)
        .unwrap()
}

fn damage_on(state: &GameState, owner: PlayerId, card: CardId) -> Option<u32> {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .map(|c| c.conditions().damage)
}

/// A quester whose "whenever this quests" trigger deals `amount` damage to a
/// chosen opposing character.
fn quester_dealing(def: u32, amount: u32) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(def), 1, true, 2, 5, 1).with_abilities(vec![
        TriggeredAbility::new(
            TriggerCondition::WhenThisQuests,
            Effect::DealDamage {
                target: Target::ChosenCharacter {
                    filter: CharacterFilter::any(TargetSide::Opposing),
                    another: false,
                },
                amount,
            },
        ),
    ])
}

#[test]
fn a_trigger_deals_damage_to_a_chosen_character() {
    let mut reg = CardRegistry::new();
    reg.insert(quester_dealing(100, 2));
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(200),
        1,
        true,
        2,
        5,
        1,
    ));
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let victim = place(&mut state, foe, 2000, 200, 5, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    assert!(
        state.is_awaiting_decision(),
        "must choose the damage target"
    );
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(victim)),
    )
    .expect("choose target");

    assert_eq!(damage_on(&state, foe, victim), Some(2));
}

#[test]
fn lethal_effect_damage_banishes_and_fires_when_banished() {
    let mut reg = CardRegistry::new();
    reg.insert(quester_dealing(100, 3));
    // Victim (willpower 2) with "when banished, gain 4 lore".
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(200), 1, true, 2, 2, 1).with_abilities(vec![
            TriggeredAbility::new(TriggerCondition::WhenBanished, Effect::GainLore(4)),
        ]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let victim = place(&mut state, foe, 2000, 200, 2, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(victim)),
    )
    .expect("choose target");

    assert!(
        state.player(foe).unwrap().discard().contains(victim),
        "lethal damage banished it"
    );
    assert_eq!(
        state.player(foe).unwrap().lore(),
        4,
        "its when-banished trigger fired (centralized for effect-driven banishment)"
    );
}

#[test]
fn remove_damage_heals_a_chosen_character() {
    let mut reg = CardRegistry::new();
    // Quester heals 2 from a chosen character of yours.
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::RemoveDamage {
                    target: Target::ChosenCharacter {
                        filter: CharacterFilter::any(TargetSide::Yours),
                        another: true,
                    },
                    amount: 2,
                },
            ),
        ]),
    );
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(200),
        1,
        true,
        2,
        5,
        1,
    ));
    let mut state = started(&reg);
    let active = state.active_player();
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let ally = place(&mut state, active, 1001, 200, 5, 3); // 3 damage

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(ally)),
    )
    .expect("choose target");

    assert_eq!(
        damage_on(&state, active, ally),
        Some(1),
        "healed 2 of 3 damage"
    );
}

#[test]
fn a_trigger_banishes_a_chosen_character_and_fires_when_banished() {
    let mut reg = CardRegistry::new();
    // Quester: "whenever this quests, banish chosen opposing character."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::Banish(Target::ChosenCharacter {
                    filter: CharacterFilter::any(TargetSide::Opposing),
                    another: false,
                }),
            ),
        ]),
    );
    // Victim with high willpower (so it can't be banished by damage) + "when
    // banished, gain 3 lore".
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(200), 1, true, 2, 9, 1).with_abilities(vec![
            TriggeredAbility::new(TriggerCondition::WhenBanished, Effect::GainLore(3)),
        ]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let victim = place(&mut state, foe, 2000, 200, 9, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(victim)),
    )
    .expect("choose target");

    assert!(
        !state.player(foe).unwrap().play().contains(victim),
        "banished out of play"
    );
    assert!(
        state.player(foe).unwrap().discard().contains(victim),
        "went to the discard"
    );
    assert_eq!(
        state.player(foe).unwrap().lore(),
        3,
        "its when-banished trigger fired"
    );
}

#[test]
fn a_cost_filter_restricts_the_choosable_targets() {
    let mut reg = CardRegistry::new();
    // Quester: "whenever this quests, deal 2 damage to chosen opposing character
    // with cost 2 or less."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::DealDamage {
                    target: Target::ChosenCharacter {
                        filter: CharacterFilter {
                            cost: Some(NumericFilter::at_most(2)),
                            ..CharacterFilter::any(TargetSide::Opposing)
                        },
                        another: false,
                    },
                    amount: 2,
                },
            ),
        ]),
    );
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(300),
        2,
        true,
        2,
        5,
        1,
    )); // cheap
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(400),
        5,
        true,
        2,
        5,
        1,
    )); // pricey
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let cheap = place(&mut state, foe, 3000, 300, 5, 0);
    let pricey = place(&mut state, foe, 4000, 400, 5, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    // The cost-5 character is not an eligible target.
    assert!(
        apply(
            &mut state,
            &reg,
            Input::Decide(Decision::ChooseTarget(pricey))
        )
        .is_err(),
        "cost-5 character is filtered out"
    );
    // The cost-2 character is.
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(cheap)),
    )
    .expect("choose the eligible target");

    assert_eq!(damage_on(&state, foe, cheap), Some(2));
    assert_eq!(damage_on(&state, foe, pricey), Some(0));
}

#[test]
fn an_item_can_be_chosen_and_banished() {
    let mut reg = CardRegistry::new();
    // Quester: "whenever this quests, banish chosen item."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::Banish(Target::ChosenItem {
                    side: TargetSide::Any,
                }),
            ),
        ]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    // Inject an item (neither character nor location) for the opponent.
    let item = CardId::from_raw(2000);
    let item_inst = CardInstance::new(
        item,
        CardDefId::from_raw(500),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    state.player_mut(foe).unwrap().play_mut().push(item_inst);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    assert!(state.is_awaiting_decision(), "must choose the item");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(item)),
    )
    .expect("choose");

    assert!(
        !state.player(foe).unwrap().play().contains(item),
        "item banished"
    );
    assert!(state.player(foe).unwrap().discard().contains(item));
}

fn strength(state: &GameState, card: CardId) -> u32 {
    state.current_character_stats(card).unwrap().strength
}

fn up_to_two_debuffer(def: u32) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(def), 1, true, 2, 5, 1).with_abilities(vec![
        TriggeredAbility::new(
            TriggerCondition::WhenThisQuests,
            Effect::GiveStrengthThisTurn {
                target: Target::UpToCharacters {
                    filter: CharacterFilter::any(TargetSide::Opposing),
                    max: 2,
                },
                amount: -1,
            },
        ),
    ])
}

#[test]
fn up_to_n_applies_the_effect_to_each_chosen_target() {
    let mut reg = CardRegistry::new();
    reg.insert(up_to_two_debuffer(100));
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let a = place(&mut state, foe, 2000, 200, 5, 0); // {S} 2
    let b = place(&mut state, foe, 2001, 200, 5, 0); // {S} 2

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    assert!(state.is_awaiting_decision());
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTargets(vec![a, b])),
    )
    .expect("choose two targets");

    assert_eq!(strength(&state, a), 1, "first target debuffed");
    assert_eq!(strength(&state, b), 1, "second target debuffed");
    assert!(!state.is_awaiting_decision());
}

#[test]
fn up_to_n_allows_choosing_fewer_than_the_maximum() {
    let mut reg = CardRegistry::new();
    reg.insert(up_to_two_debuffer(100));
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let a = place(&mut state, foe, 2000, 200, 5, 0);
    let b = place(&mut state, foe, 2001, 200, 5, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    // Choosing just one of the two is allowed (0..max).
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTargets(vec![a])),
    )
    .expect("choose one target");

    assert_eq!(strength(&state, a), 1);
    assert_eq!(
        strength(&state, b),
        2,
        "the unchosen character is unaffected"
    );
}

#[test]
fn up_to_n_rejects_too_many_or_duplicate_targets() {
    let mut reg = CardRegistry::new();
    reg.insert(up_to_two_debuffer(100));
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let a = place(&mut state, foe, 2000, 200, 5, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    // The same character can't be chosen twice (§7.1.8).
    assert!(
        apply(
            &mut state,
            &reg,
            Input::Decide(Decision::ChooseTargets(vec![a, a]))
        )
        .is_err()
    );
}

#[test]
fn a_name_filter_restricts_the_choosable_targets() {
    let mut reg = CardRegistry::new();
    // Quester: "whenever this quests, deal 2 damage to chosen character named Stitch."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::DealDamage {
                    target: Target::ChosenCharacter {
                        filter: CharacterFilter {
                            names: vec!["Stitch".to_string()],
                            ..CharacterFilter::any(TargetSide::Opposing)
                        },
                        another: false,
                    },
                    amount: 2,
                },
            ),
        ]),
    );
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(200), 1, true, 2, 5, 1)
            .with_names(vec!["Stitch".to_string()]),
    );
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(300), 1, true, 2, 5, 1)
            .with_names(vec!["Scar".to_string()]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let stitch = place(&mut state, foe, 2000, 200, 5, 0);
    let scar = place(&mut state, foe, 3000, 300, 5, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    assert!(
        apply(
            &mut state,
            &reg,
            Input::Decide(Decision::ChooseTarget(scar))
        )
        .is_err(),
        "the non-Stitch character is not a legal target"
    );
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(stitch)),
    )
    .expect("choose Stitch");
    assert_eq!(damage_on(&state, foe, stitch), Some(2));
    assert_eq!(damage_on(&state, foe, scar), Some(0));
}

#[test]
fn all_your_other_characters_excludes_the_source() {
    let mut reg = CardRegistry::new();
    // Quester: "whenever this quests, your other characters get +1 {S} this turn."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::GiveStrengthThisTurn {
                    target: Target::AllCharacters {
                        filter: CharacterFilter::any(TargetSide::Yours),
                        another: true,
                    },
                    amount: 1,
                },
            ),
        ]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let quester = place(&mut state, active, 1000, 100, 5, 0); // {S} 2
    let ally = place(&mut state, active, 1001, 200, 5, 0); // {S} 2

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    assert_eq!(strength(&state, ally), 3, "the other character is buffed");
    assert_eq!(
        strength(&state, quester),
        2,
        "the source itself is excluded"
    );
}

fn conditional_quester(def: u32) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(def), 1, true, 2, 5, 1).with_abilities(vec![
        TriggeredAbility::new(
            TriggerCondition::WhenThisQuests,
            Effect::IfControl {
                filter: CharacterFilter {
                    names: vec!["Elsa".to_string()],
                    ..CharacterFilter::any(TargetSide::Yours)
                },
                then: Box::new(Effect::GainLore(3)),
            },
        ),
    ])
}

#[test]
fn a_conditional_effect_resolves_when_the_board_condition_holds() {
    let mut reg = CardRegistry::new();
    reg.insert(conditional_quester(100));
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(200), 1, true, 1, 5, 1)
            .with_names(vec!["Elsa".to_string()]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let _elsa = place(&mut state, active, 1001, 200, 5, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    assert_eq!(
        state.player(active).unwrap().lore(),
        4,
        "condition held -> bonus applied"
    );
}

#[test]
fn a_conditional_effect_is_skipped_when_the_condition_fails() {
    let mut reg = CardRegistry::new();
    reg.insert(conditional_quester(100));
    let mut state = started(&reg);
    let active = state.active_player();
    let quester = place(&mut state, active, 1000, 100, 5, 0); // no Elsa in play

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    assert_eq!(
        state.player(active).unwrap().lore(),
        1,
        "condition failed -> no bonus"
    );
}

fn is_ready(state: &GameState, owner: PlayerId, card: CardId) -> bool {
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
fn exert_effect_exerts_a_chosen_character() {
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::Exert(Target::ChosenCharacter {
                    filter: CharacterFilter::any(TargetSide::Opposing),
                    another: false,
                }),
            ),
        ]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let quester = place(&mut state, active, 1000, 100, 5, 0);
    let victim = place(&mut state, foe, 2000, 200, 5, 0); // ready

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(victim)),
    )
    .expect("choose");

    assert!(
        !is_ready(&state, foe, victim),
        "the chosen character was exerted"
    );
}

#[test]
fn ready_effect_readies_the_source() {
    let mut reg = CardRegistry::new();
    // "Whenever this quests, ready this character." (quest exerts it, then re-ready)
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::Ready(Target::SelfCard),
            ),
        ]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let quester = place(&mut state, active, 1000, 100, 5, 0);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    assert!(
        is_ready(&state, active, quester),
        "questing exerted it, then it readied itself"
    );
}
