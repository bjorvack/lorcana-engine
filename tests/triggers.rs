//! Integration tests for Slice 4: the bag and triggered abilities — ETB and
//! quest triggers, optional ("may") triggers, and player-chosen ordering of
//! simultaneous triggers.

use lorcana_engine::{
    Amount, CardCategory, CardDefId, CardDefinition, CardId, CardInstance, CardRegistry,
    CharacterStats, Classification, Conditions, Decision, Effect, GameEvent, GameState, GameStatus,
    Input, PendingDecision, TriggerCondition, TriggeredAbility, apply, start,
};

fn two_decks(size: u32) -> Vec<Vec<CardDefId>> {
    vec![
        (0..size).map(CardDefId::from_raw).collect(),
        (0..size).map(CardDefId::from_raw).collect(),
    ]
}

/// A registry whose cards are all inkable cost-1 characters carrying the given
/// triggered abilities.
fn registry_with(abilities: &[TriggeredAbility]) -> CardRegistry {
    (0..30)
        .map(|n| {
            CardDefinition::character(CardDefId::from_raw(n), 1, true, 1, 1, 1)
                .with_abilities(abilities.to_vec())
        })
        .collect()
}

fn started(registry: &CardRegistry) -> GameState {
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

fn active_hand_card(state: &GameState, nth: usize) -> CardId {
    state
        .player(state.active_player())
        .unwrap()
        .hand()
        .iter()
        .nth(nth)
        .unwrap()
        .id()
}

/// Ink one card then play another; returns the events from the play.
fn ink_then_play(state: &mut GameState, registry: &CardRegistry) -> Vec<GameEvent> {
    let ink = active_hand_card(state, 0);
    let subject = active_hand_card(state, 1);
    let _ = apply(state, registry, Input::PutCardInInkwell { card: ink }).expect("ink");
    apply(
        state,
        registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    )
    .expect("play")
}

#[test]
fn enters_play_trigger_draws_a_card() {
    let registry = registry_with(&[TriggeredAbility::new(
        TriggerCondition::WhenYouPlayThis,
        Effect::DrawCards(Amount::fixed(1)),
    )]);
    let mut state = started(&registry);
    let active = state.active_player();

    let events = ink_then_play(&mut state, &registry);

    assert!(!state.is_awaiting_decision());
    // 7 - 1 (inked) - 1 (played) + 1 (ETB draw) = 6.
    assert_eq!(state.player(active).unwrap().hand().len(), 6);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, GameEvent::CardDrawn { player, .. } if *player == active))
    );
}

#[test]
fn optional_trigger_waits_for_a_may_decision() {
    let registry = registry_with(&[TriggeredAbility::optional(
        TriggerCondition::WhenYouPlayThis,
        Effect::DrawCards(Amount::fixed(1)),
    )]);
    let mut state = started(&registry);
    let active = state.active_player();

    let _ = ink_then_play(&mut state, &registry);
    // Suspended on the "may" decision.
    assert!(matches!(
        state.pending(),
        Some(PendingDecision::MayResolve { .. })
    ));
    let hand_before = state.player(active).unwrap().hand().len();

    // Declining draws nothing.
    let mut declined = state.clone();
    let _ = apply(
        &mut declined,
        &registry,
        Input::Decide(Decision::May(false)),
    )
    .expect("decline");
    assert!(!declined.is_awaiting_decision());
    assert_eq!(declined.player(active).unwrap().hand().len(), hand_before);

    // Accepting draws a card.
    let _ = apply(&mut state, &registry, Input::Decide(Decision::May(true))).expect("accept");
    assert!(!state.is_awaiting_decision());
    assert_eq!(state.player(active).unwrap().hand().len(), hand_before + 1);
}

#[test]
fn turn_actions_are_rejected_while_a_decision_is_pending() {
    let registry = registry_with(&[TriggeredAbility::optional(
        TriggerCondition::WhenYouPlayThis,
        Effect::DrawCards(Amount::fixed(1)),
    )]);
    let mut state = started(&registry);
    let _ = ink_then_play(&mut state, &registry);
    assert!(state.is_awaiting_decision());

    // Cannot end the turn until the decision is answered.
    assert!(apply(&mut state, &registry, Input::EndTurn).is_err());
}

#[test]
fn player_orders_two_simultaneous_triggers() {
    let registry = registry_with(&[
        TriggeredAbility::new(
            TriggerCondition::WhenYouPlayThis,
            Effect::GainLore(Amount::fixed(1)),
        ),
        TriggeredAbility::new(
            TriggerCondition::WhenYouPlayThis,
            Effect::DrawCards(Amount::fixed(1)),
        ),
    ]);
    let mut state = started(&registry);
    let active = state.active_player();

    let _ = ink_then_play(&mut state, &registry);

    // Two triggers from one card → the controller chooses the order.
    let Some(PendingDecision::OrderTriggers { player, options }) = state.pending() else {
        panic!("expected an ordering decision");
    };
    assert_eq!(*player, active);
    assert_eq!(options.len(), 2);
    let first = options[0];

    let _ = apply(
        &mut state,
        &registry,
        Input::Decide(Decision::ResolveNext(first)),
    )
    .expect("order");

    // Both triggers resolved regardless of order: +1 lore and +1 card.
    assert!(!state.is_awaiting_decision());
    assert_eq!(state.player(active).unwrap().lore(), 1);
    assert_eq!(state.player(active).unwrap().hand().len(), 6);
}

#[test]
fn whenever_you_play_a_classification_trigger_fires() {
    // Deck cards are cost-0 Villain characters; a watcher in play has "whenever
    // you play a Villain character, gain 1 lore".
    let mut registry: CardRegistry = (0..30)
        .map(|n| {
            CardDefinition::character(CardDefId::from_raw(n), 0, true, 2, 3, 1)
                .with_classifications(vec![Classification::new("Villain")])
        })
        .collect();
    registry.insert(
        CardDefinition::character(CardDefId::from_raw(1000), 0, true, 1, 1, 1).with_abilities(
            vec![TriggeredAbility::new(
                TriggerCondition::WhenYouPlay(CardCategory::Character(Some(Classification::new(
                    "Villain",
                )))),
                Effect::GainLore(Amount::fixed(1)),
            )],
        ),
    );
    let mut state = started(&registry);
    let active = state.active_player();

    // Put the watcher into play.
    let mut watcher = CardInstance::new(
        CardId::from_raw(5000),
        CardDefId::from_raw(1000),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    watcher.set_stats(Some(CharacterStats::new(1, 1, 1)));
    state.player_mut(active).unwrap().play_mut().push(watcher);

    // Playing a Villain character fires the watcher's trigger.
    let subject = active_hand_card(&state, 0);
    let _ = apply(
        &mut state,
        &registry,
        Input::PlayCard {
            card: subject,
            shift_onto: None,
        },
    )
    .expect("play villain");

    assert_eq!(state.player(active).unwrap().lore(), 1);
}

#[test]
fn quest_trigger_fires() {
    // A dry character whose definition has "whenever this character quests, gain
    // 2 lore", injected into play.
    let def = CardDefId::from_raw(1000);
    let mut registry = CardRegistry::new();
    registry.insert(
        CardDefinition::character(def, 1, true, 1, 1, 1).with_abilities(vec![
            TriggeredAbility::new(
                TriggerCondition::WhenThisQuests,
                Effect::GainLore(Amount::fixed(2)),
            ),
        ]),
    );
    let mut state = started(&registry);
    let active = state.active_player();

    let character = CardId::from_raw(2000);
    let mut instance = CardInstance::new(
        character,
        def,
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(1, 1, 1)));
    state.player_mut(active).unwrap().play_mut().push(instance);

    let _ = apply(&mut state, &registry, Input::Quest { character }).expect("quest");

    // 1 lore from questing + 2 from the trigger.
    assert_eq!(state.player(active).unwrap().lore(), 3);
}

#[test]
fn same_seed_and_inputs_with_a_decision_are_deterministic() {
    let abilities = vec![TriggeredAbility::optional(
        TriggerCondition::WhenYouPlayThis,
        Effect::DrawCards(Amount::fixed(1)),
    )];
    let run = || {
        let registry = registry_with(&abilities);
        let mut state = started(&registry);
        let mut events = ink_then_play(&mut state, &registry);
        events
            .extend(apply(&mut state, &registry, Input::Decide(Decision::May(true))).expect("may"));
        (state, events)
    };
    let (state_a, events_a) = run();
    let (state_b, events_b) = run();
    assert_eq!(state_a, state_b);
    assert_eq!(events_a, events_b);
}
