//! Integration tests for Slice 8b-13: effect-driven challenge/quest legality —
//! restrictions ("can't quest / challenge / be challenged", §1.2.2) and
//! permissions ("may challenge ready / Evasive / while drying"), routed through
//! the single legality authority. Preventions beat permissions (§1.2.2).

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterFilter, CharacterStats,
    Conditions, Decision, Effect, GameState, GameStatus, Input, Keyword, ModifierDuration,
    ModifierTarget, Permission, PlayerId, Property, PropertyModifier, Restriction, Target,
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
    ready: bool,
    drying: bool,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut inst = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready,
            damage: 0,
            drying,
            facedown: false,
        },
    );
    inst.set_stats(Some(CharacterStats::new(2, 5, 1)));
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

fn restrict(state: &mut GameState, card: CardId, restriction: Restriction) {
    state.add_property_modifier(PropertyModifier::new(
        CardId::from_raw(9999),
        ModifierTarget::Card(card),
        Property::Restriction(restriction),
        ModifierDuration::WhileSourceInPlay,
    ));
}

fn permit(state: &mut GameState, card: CardId, permission: Permission) {
    state.add_property_modifier(PropertyModifier::new(
        CardId::from_raw(9999),
        ModifierTarget::Card(card),
        Property::Permission(permission),
        ModifierDuration::WhileSourceInPlay,
    ));
}

fn challenge(
    state: &mut GameState,
    reg: &CardRegistry,
    challenger: CardId,
    target: CardId,
) -> bool {
    apply(state, reg, Input::Challenge { challenger, target }).is_ok()
}

#[test]
fn cant_be_challenged_blocks_a_challenge() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let c = place(&mut state, me, 1, 100, true, false);
    let t = place(&mut state, foe, 2, 200, false, false); // exerted (normally challengeable)
    restrict(&mut state, t, Restriction::CantBeChallenged);

    assert!(
        !challenge(&mut state, &reg, c, t),
        "can't be challenged (§1.2.2)"
    );
}

#[test]
fn cant_challenge_blocks_the_challenger() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let c = place(&mut state, me, 1, 100, true, false);
    let t = place(&mut state, foe, 2, 200, false, false);
    restrict(&mut state, c, Restriction::CantChallenge);

    assert!(
        !challenge(&mut state, &reg, c, t),
        "the challenger can't challenge"
    );
}

#[test]
fn cant_quest_blocks_questing() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let q = place(&mut state, me, 1, 100, true, false);
    restrict(&mut state, q, Restriction::CantQuest);

    assert!(apply(&mut state, &reg, Input::Quest { character: q }).is_err());
}

#[test]
fn challenge_ready_permission_allows_challenging_a_ready_target() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let c = place(&mut state, me, 1, 100, true, false);
    let t = place(&mut state, foe, 2, 200, true, false); // READY — normally illegal
    assert!(
        !challenge(&mut state, &reg, c, t),
        "a ready target is normally illegal"
    );

    permit(&mut state, c, Permission::ChallengeReady);
    assert!(
        challenge(&mut state, &reg, c, t),
        "the permission allows it"
    );
}

#[test]
fn quest_while_drying_permission_allows_questing_while_drying() {
    let reg = CardRegistry::new();
    let mut state = started(&reg);
    let me = state.active_player();
    let q = place(&mut state, me, 1, 100, true, true); // drying
    assert!(apply(&mut state, &reg, Input::Quest { character: q }).is_err());

    permit(&mut state, q, Permission::QuestWhileDrying);
    assert!(apply(&mut state, &reg, Input::Quest { character: q }).is_ok());
}

#[test]
fn challenge_evasive_permission_lets_a_non_alert_challenge_an_evasive_target() {
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(200), 1, true, 2, 5, 1)
            .with_keywords(vec![Keyword::Evasive]),
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let c = place(&mut state, me, 1, 100, true, false); // not Evasive / Alert
    let t = place(&mut state, foe, 2, 200, false, false); // exerted Evasive
    assert!(
        !challenge(&mut state, &reg, c, t),
        "Evasive blocks a non-Evasive challenger"
    );

    permit(&mut state, c, Permission::ChallengeEvasive);
    assert!(
        challenge(&mut state, &reg, c, t),
        "the permission lets it through"
    );
}

#[test]
fn an_effect_can_grant_a_permission_to_a_chosen_character() {
    // Quester: "whenever this quests, another chosen character of yours may
    // challenge ready characters this turn."
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::PermitThisTurn {
                    target: Target::ChosenCharacter {
                        filter: CharacterFilter::any(TargetSide::Yours),
                        another: true,
                    },
                    permission: Permission::ChallengeReady,
                },
            ),
        ]),
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let quester = place(&mut state, me, 1, 100, true, false);
    let ally = place(&mut state, me, 2, 101, true, false);
    let ready_target = place(&mut state, foe, 3, 200, true, false); // ready

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(ally)),
    )
    .expect("grant");

    assert!(
        challenge(&mut state, &reg, ally, ready_target),
        "the granted permission lets the ally challenge a ready character"
    );
}
