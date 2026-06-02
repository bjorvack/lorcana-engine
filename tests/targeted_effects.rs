//! Integration tests for Slice 8b: targeted damage effects (deal / remove
//! damage to a chosen character) and the centralized "when banished" trigger for
//! effect-driven banishment.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterFilter, CharacterStats,
    Conditions, Decision, Effect, GameState, GameStatus, Input, PlayerId, Target, TargetSide,
    TriggerCondition, TriggeredAbility, apply, start,
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
