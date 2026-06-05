//! Integration tests for player-scoped effects across player counts: a `Chosen*`
//! `PlayerScope` auto-resolves with a single candidate (2-player) but prompts a
//! choose-a-player decision with 2+ candidates (3–4 player games), §7.1.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, ChoiceRef,
    Conditions, Decision, DiscardAmount, DiscardBy, Effect, GameState, GameStatus, Input,
    PendingDecision, PlayerId, PlayerScope, TriggerCondition, TriggeredAbility, apply, start,
};

fn registry() -> CardRegistry {
    let mut reg = CardRegistry::new();
    // Quester: "whenever this quests, chosen opponent chooses and discards a card."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::when_this_quests(),
                Effect::Discard {
                    who: PlayerScope::ChosenOpponent,
                    amount: DiscardAmount::Count(1),
                    by: DiscardBy::Owner,
                },
            ),
        ]),
    );
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
    reg
}

fn started_n(reg: &CardRegistry, players: usize) -> GameState {
    let decks: Vec<Vec<CardDefId>> = (0..players)
        .map(|_| (0..30).map(CardDefId::from_raw).collect())
        .collect();
    let mut state = GameState::new(decks, 7);
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

fn place_quester(state: &mut GameState, owner: PlayerId) -> CardId {
    let id = state.allocate_card_id();
    let mut inst = CardInstance::new(
        id,
        CardDefId::from_raw(100),
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

fn opponents_of(state: &GameState, player: PlayerId) -> Vec<PlayerId> {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .filter(|p| *p != player)
        .collect()
}

fn hand_len(state: &GameState, player: PlayerId) -> usize {
    state.player(player).unwrap().hand().iter().count()
}

#[test]
fn chosen_opponent_prompts_a_choice_in_a_four_player_game() {
    let reg = registry();
    let mut state = started_n(&reg, 4);
    let active = state.active_player();
    let opps = opponents_of(&state, active);
    let quester = place_quester(&mut state, active);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    // 3 opponents -> the controller must choose which one discards.
    let Some(PendingDecision::Choose {
        player, options, ..
    }) = state.pending()
    else {
        panic!("expected a choose-a-player decision with 3 opponents");
    };
    assert_eq!(*player, active);
    let chosen: Vec<_> = options
        .iter()
        .map(|r| match r {
            ChoiceRef::Player(p) => *p,
            ChoiceRef::Card(_) => panic!("expected player options"),
        })
        .collect();
    assert_eq!(chosen, opps);

    let target = opps[1];
    let before = hand_len(&state, target);
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChoosePlayer(target)),
    )
    .expect("choose");

    // Now that opponent makes their own discard choice.
    let Some(PendingDecision::Choose {
        player: chooser, ..
    }) = state.pending()
    else {
        panic!("the chosen opponent now discards");
    };
    assert_eq!(*chooser, target);
    let card = state
        .player(target)
        .unwrap()
        .hand()
        .iter()
        .next()
        .unwrap()
        .id();
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::DiscardCards(vec![card])),
    )
    .expect("discard");
    assert_eq!(hand_len(&state, target), before - 1);
}

#[test]
fn chosen_opponent_auto_resolves_in_two_player() {
    let reg = registry();
    let mut state = started_n(&reg, 2);
    let active = state.active_player();
    let opp = opponents_of(&state, active)[0];
    let quester = place_quester(&mut state, active);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    // Only one opponent: no choose-a-player step — straight to that opponent's
    // discard choice.
    let Some(PendingDecision::Choose { player, .. }) = state.pending() else {
        panic!("expected the single opponent's discard choice, no player prompt");
    };
    assert_eq!(*player, opp);
}

#[test]
fn mill_is_a_card_move_from_deck_to_discard() {
    // "Put the top 2 cards of your deck into your discard" — milling expressed as
    // a zone move (MoveSource::DeckTop -> Destination::Discard).
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::when_this_quests(),
                Effect::Move {
                    what: lorcana_engine::MoveSource::DeckTop {
                        who: PlayerScope::You,
                        count: lorcana_engine::Amount::fixed(2),
                    },
                    to: lorcana_engine::Destination::Discard,
                },
            ),
        ]),
    );
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
    let mut state = started_n(&reg, 2);
    let active = state.active_player();
    let quester = place_quester(&mut state, active);
    let deck_before = state.player(active).unwrap().deck().len();
    let discard_before = state.player(active).unwrap().discard().len();

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    assert!(state.pending().is_none());
    assert_eq!(state.player(active).unwrap().deck().len(), deck_before - 2);
    assert_eq!(
        state.player(active).unwrap().discard().len(),
        discard_before + 2
    );
}

#[test]
fn each_player_draws_applies_to_everyone() {
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::when_this_quests(),
                Effect::Draw {
                    who: PlayerScope::EachPlayer,
                    amount: lorcana_engine::Amount::fixed(2),
                },
            ),
        ]),
    );
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
    let mut state = started_n(&reg, 4);
    let active = state.active_player();
    let quester = place_quester(&mut state, active);
    let before: Vec<usize> = state
        .players()
        .iter()
        .map(|p| p.hand().iter().count())
        .collect();

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    assert!(state.pending().is_none(), "no choice: every player draws");
    for (p, b) in state.players().iter().zip(before) {
        assert_eq!(p.hand().iter().count(), b + 2);
    }
}
