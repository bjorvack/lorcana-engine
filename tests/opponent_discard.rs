//! Reveal a chosen opponent's hand and discard a matching card of your choice
//! (Lenny / Timon / Goldie, §8.4).

use lorcana_engine::{
    CardCategory, CardDefId, CardDefinition, CardId, CardInstance, CardKind, CardRegistry,
    CharacterFilter, CharacterStats, Conditions, Decision, Effect, GameState, GameStatus, Input,
    PlayerId, PlayerScope, TriggerCondition, TriggeredAbility, apply, start,
};

const ACTION: u32 = 900;

fn registry() -> CardRegistry {
    let mut reg = CardRegistry::new();
    // Quester: "When this quests, chosen opponent reveals their hand and discards
    // an action card of your choice."
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::when_this_quests(),
                Effect::OpponentDiscardsChosen {
                    whose: PlayerScope::ChosenOpponent,
                    filter: CharacterFilter::Category(CardCategory::Action),
                },
            ),
        ]),
    );
    reg.insert(CardDefinition::new(
        CardDefId::from_raw(ACTION),
        1,
        true,
        CardKind::Action,
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
    reg
}

fn started(reg: &CardRegistry) -> GameState {
    let decks: Vec<Vec<CardDefId>> = (0..2)
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

fn give_hand_card(state: &mut GameState, owner: PlayerId, def: u32) -> CardId {
    let id = state.allocate_card_id();
    let inst = CardInstance::new(id, CardDefId::from_raw(def), Conditions::faceup_idle());
    state.player_mut(owner).unwrap().hand_mut().push(inst);
    id
}

fn in_hand(state: &GameState, player: PlayerId, card: CardId) -> bool {
    state
        .player(player)
        .unwrap()
        .hand()
        .iter()
        .any(|c| c.id() == card)
}

fn in_discard(state: &GameState, player: PlayerId, card: CardId) -> bool {
    state
        .player(player)
        .unwrap()
        .discard()
        .iter()
        .any(|c| c.id() == card)
}

#[test]
fn chosen_opponent_discards_an_action_of_your_choice() {
    let reg = registry();
    let mut state = started(&reg);
    let me = state.active_player();
    let opp = state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != me)
        .unwrap();

    let quester = place_quester(&mut state, me);
    let action = give_hand_card(&mut state, opp, ACTION);
    let character = give_hand_card(&mut state, opp, 0); // a non-action hand card

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    // Two players -> the lone opponent is auto-resolved; we go straight to the
    // card choice over their hand's action cards.
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(action)),
    )
    .expect("discard");

    assert!(
        in_discard(&state, opp, action),
        "the chosen action was discarded"
    );
    assert!(!in_hand(&state, opp, action));
    assert!(
        in_hand(&state, opp, character),
        "the non-action card stays in hand"
    );
}

#[test]
fn no_matching_card_means_no_discard() {
    // Timon: "discards a non-character card of your choice" — if the opponent's
    // hand is all characters, nothing is discarded (no choice is offered).
    let mut reg = CardRegistry::new();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::when_this_quests(),
                Effect::OpponentDiscardsChosen {
                    whose: PlayerScope::ChosenOpponent,
                    filter: CharacterFilter::negate(CharacterFilter::Category(
                        CardCategory::Character(None),
                    )),
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
    let mut state = started(&reg);
    let me = state.active_player();
    let quester = place_quester(&mut state, me);
    let opp_hand_before = state
        .players()
        .iter()
        .find(|p| p.id() != me)
        .unwrap()
        .hand()
        .iter()
        .count();

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    // The opponent's opening hand is all characters -> no non-character to discard,
    // so no decision is pending and nothing changed.
    assert!(state.pending().is_none(), "no choice when nothing matches");
    let opp_hand_after = state
        .players()
        .iter()
        .find(|p| p.id() != me)
        .unwrap()
        .hand()
        .iter()
        .count();
    assert_eq!(opp_hand_before, opp_hand_after);
}

fn quester_with(effect: Effect) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(100), 1, true, 1, 5, 1).with_abilities(vec![
        TriggeredAbility::new(TriggerCondition::when_this_quests(), effect),
    ])
}

fn reg_with(quester: CardDefinition) -> CardRegistry {
    let mut reg = CardRegistry::new();
    reg.insert(quester);
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

fn opponent_of(state: &GameState, me: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != me)
        .unwrap()
}

#[test]
fn random_discard_removes_a_card_without_a_choice() {
    use lorcana_engine::DiscardBy;
    let reg = reg_with(quester_with(Effect::Discard {
        who: PlayerScope::ChosenOpponent,
        amount: lorcana_engine::DiscardAmount::Count(1),
        by: DiscardBy::Random,
    }));
    let mut state = started(&reg);
    let me = state.active_player();
    let opp = opponent_of(&state, me);
    let quester = place_quester(&mut state, me);
    let before = state.player(opp).unwrap().hand().iter().count();

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    // Random discard resolves with no decision pending.
    assert!(state.pending().is_none(), "random discard needs no choice");
    let after = state.player(opp).unwrap().hand().iter().count();
    assert_eq!(
        after,
        before - 1,
        "exactly one card was discarded at random"
    );
    assert_eq!(state.player(opp).unwrap().discard().iter().count(), 1);
}

fn started_n(reg: &CardRegistry, n: usize) -> GameState {
    let decks: Vec<Vec<CardDefId>> = (0..n)
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

#[test]
fn each_opponent_chooses_and_discards_in_turn_order() {
    // "Each opponent chooses and discards a card" (DiscardBy::Owner): the discard
    // is driven down every opponent in turn, each picking their own card. With
    // three players the controller's two opponents are each prompted in sequence
    // via the multi-player discard continuation (§8.4).
    use lorcana_engine::{DiscardAmount, DiscardBy};
    let reg = reg_with(quester_with(Effect::Discard {
        who: PlayerScope::EachOpponent,
        amount: DiscardAmount::Count(1),
        by: DiscardBy::Owner,
    }));
    let mut state = started_n(&reg, 3);
    let me = state.active_player();
    let opponents: Vec<PlayerId> = state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .filter(|p| *p != me)
        .collect();
    assert_eq!(opponents.len(), 2, "three-player game has two opponents");

    let quester = place_quester(&mut state, me);
    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    // Each opponent is asked, in turn, to choose one of their own cards. The
    // controller is never asked to discard.
    let mut discarded: Vec<(PlayerId, CardId)> = Vec::new();
    while let Some(player) = state.pending().map(lorcana_engine::PendingDecision::player) {
        assert!(
            opponents.contains(&player),
            "only opponents choose what to discard"
        );
        let card = state
            .player(player)
            .unwrap()
            .hand()
            .iter()
            .next()
            .unwrap()
            .id();
        discarded.push((player, card));
        let _ = apply(
            &mut state,
            &reg,
            Input::Decide(Decision::DiscardCards(vec![card])),
        )
        .expect("discard");
    }

    assert_eq!(
        discarded.len(),
        opponents.len(),
        "every opponent discarded exactly once"
    );
    for (player, card) in discarded {
        assert!(
            in_discard(&state, player, card),
            "the chosen card was discarded"
        );
        assert!(!in_hand(&state, player, card));
        assert_eq!(
            state.player(player).unwrap().discard().iter().count(),
            1,
            "each opponent discarded exactly one card"
        );
    }
    assert_eq!(
        state.player(me).unwrap().discard().iter().count(),
        0,
        "the controller never discards"
    );
}

#[test]
fn chosen_opponent_discards_multiple_at_random() {
    // "Chosen opponent ... discards 2 at random": N random cards leave the hand
    // with no decision, using the seeded RNG (§8.4).
    use lorcana_engine::{DiscardAmount, DiscardBy};
    let reg = reg_with(quester_with(Effect::Discard {
        who: PlayerScope::ChosenOpponent,
        amount: DiscardAmount::Count(2),
        by: DiscardBy::Random,
    }));
    let mut state = started(&reg);
    let me = state.active_player();
    let opp = opponent_of(&state, me);
    let quester = place_quester(&mut state, me);
    let before = state.player(opp).unwrap().hand().iter().count();

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    assert!(state.pending().is_none(), "random discard needs no choice");
    let after = state.player(opp).unwrap().hand().iter().count();
    assert_eq!(
        after,
        before - 2,
        "exactly two cards were discarded at random"
    );
    assert_eq!(state.player(opp).unwrap().discard().iter().count(), 2);
}

#[test]
fn reveal_hand_emits_a_hand_revealed_event() {
    use lorcana_engine::GameEvent;
    let reg = reg_with(quester_with(Effect::RevealHand {
        whose: PlayerScope::ChosenOpponent,
    }));
    let mut state = started(&reg);
    let me = state.active_player();
    let opp = opponent_of(&state, me);
    let quester = place_quester(&mut state, me);

    let events = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    let revealed = events.iter().any(|e| {
        matches!(
            e,
            GameEvent::HandRevealed { player, cards } if *player == opp && !cards.is_empty()
        )
    });
    assert!(
        revealed,
        "a HandRevealed event names the opponent and their cards"
    );
}
