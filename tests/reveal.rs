//! Integration tests for Slice 8c: "look at the top N cards of your deck; you may
//! take one matching a filter into your hand; put the rest on the bottom" — the
//! scry/tutor pattern (Be Our Guest, Ariel, Develop Your Brain, §8.2).

use lorcana_engine::{
    CardCategory, CardDefId, CardDefinition, CardId, CardInstance, CardKind, CardRegistry,
    CharacterStats, ChoiceRef, Conditions, Decision, DeckPosition, Effect, GameState, GameStatus,
    Input, PendingDecision, PlayFilter, PlayerId, TriggerCondition, TriggeredAbility, apply, start,
};

const fn char_def(id: u32) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(id), 1, true, 2, 3, 1)
}

const fn item_def(id: u32) -> CardDefinition {
    CardDefinition::new(CardDefId::from_raw(id), 1, true, CardKind::Item)
}

/// Build a registry from the given defs plus character fillers for deck/hand ids.
fn registry(extra: Vec<CardDefinition>) -> CardRegistry {
    let mut reg = CardRegistry::new();
    for n in 0..30 {
        reg.insert(char_def(n));
    }
    for def in extra {
        reg.insert(def);
    }
    reg
}

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

/// Push a card onto the top of `owner`'s deck and return its (freshly allocated) id.
fn push_top(state: &mut GameState, owner: PlayerId, def: u32) -> CardId {
    let id = state.allocate_card_id();
    let inst = CardInstance::new(id, CardDefId::from_raw(def), Conditions::in_deck());
    state.player_mut(owner).unwrap().deck_mut().push(inst);
    id
}

fn place_quester(state: &mut GameState, owner: PlayerId, def: u32) -> CardId {
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

fn look_quester(count: u32, category: Option<CardCategory>, rest: DeckPosition) -> CardDefinition {
    char_def(100).with_abilities(vec![TriggeredAbility::new(
        TriggerCondition::WhenThisQuests,
        Effect::LookAtTopAndTake {
            whose: lorcana_engine::PlayerScope::You,
            count,
            filter: PlayFilter {
                max_cost: None,
                category,
            },
            rest,
        },
    )])
}

fn in_hand(state: &GameState, player: PlayerId, card: CardId) -> bool {
    state
        .player(player)
        .unwrap()
        .hand()
        .iter()
        .any(|c| c.id() == card)
}
fn in_deck(state: &GameState, player: PlayerId, card: CardId) -> bool {
    state.player(player).unwrap().deck().contains(card)
}

#[test]
fn look_at_top_and_take_a_matching_card_into_hand() {
    let reg = registry(vec![
        look_quester(3, Some(CardCategory::Character(None)), DeckPosition::Bottom),
        item_def(900),
        char_def(901),
        item_def(902),
    ]);
    let mut state = started(&reg);
    let me = state.active_player();
    let quester = place_quester(&mut state, me, 100);
    let bottom_item = push_top(&mut state, me, 900);
    let target = push_top(&mut state, me, 901); // the character among the top 3
    let top_item = push_top(&mut state, me, 902);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let Some(PendingDecision::Choose { options, .. }) = state.pending() else {
        panic!("expected a take-from-revealed choice");
    };
    let cards: Vec<_> = options
        .iter()
        .filter_map(|r| match r {
            ChoiceRef::Card(c) => Some(*c),
            ChoiceRef::Player(_) => None,
        })
        .collect();
    assert_eq!(cards, vec![target], "only the character is takeable");

    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::TakeRevealed(Some(target))),
    )
    .expect("take");

    assert!(in_hand(&state, me, target), "the character went to hand");
    assert!(!in_deck(&state, me, target));
    assert!(
        in_deck(&state, me, bottom_item) && in_deck(&state, me, top_item),
        "the rest stay in the deck"
    );
}

#[test]
fn look_at_top_with_no_match_puts_everything_back_with_no_choice() {
    let reg = registry(vec![
        look_quester(2, Some(CardCategory::Character(None)), DeckPosition::Bottom),
        item_def(900),
        item_def(901),
    ]);
    let mut state = started(&reg);
    let me = state.active_player();
    let quester = place_quester(&mut state, me, 100);
    let a = push_top(&mut state, me, 900);
    let b = push_top(&mut state, me, 901);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    assert!(
        state.pending().is_none(),
        "nothing matches; no choice is offered"
    );
    assert!(
        !in_hand(&state, me, a) && !in_hand(&state, me, b),
        "nothing taken"
    );
    assert!(
        in_deck(&state, me, a) && in_deck(&state, me, b),
        "rest stay in deck"
    );
}

#[test]
fn look_at_top_can_decline_to_take() {
    let reg = registry(vec![
        look_quester(2, Some(CardCategory::Character(None)), DeckPosition::Bottom),
        char_def(900),
        char_def(901),
    ]);
    let mut state = started(&reg);
    let me = state.active_player();
    let quester = place_quester(&mut state, me, 100);
    let a = push_top(&mut state, me, 900);
    let b = push_top(&mut state, me, 901);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    assert!(state.pending().is_some());
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::TakeRevealed(None)),
    )
    .expect("decline");

    assert!(
        !in_hand(&state, me, a) && !in_hand(&state, me, b),
        "declined: nothing taken"
    );
    assert!(in_deck(&state, me, a) && in_deck(&state, me, b));
}

#[test]
fn look_at_a_chosen_players_deck_and_take_to_your_hand() {
    // "Look at the top card of chosen player's deck; take a character into your
    // hand." The looked-at deck is the chosen player's; the card enters the
    // looker's hand. With 2 candidates (self/opponent) the looker is prompted.
    let reg = registry(vec![
        char_def(901),
        char_def(100).with_abilities(vec![TriggeredAbility::new(
            TriggerCondition::WhenThisQuests,
            Effect::LookAtTopAndTake {
                whose: lorcana_engine::PlayerScope::ChosenPlayer,
                count: 1,
                filter: PlayFilter {
                    max_cost: None,
                    category: Some(CardCategory::Character(None)),
                },
                rest: DeckPosition::Bottom,
            },
        )]),
    ]);
    let mut state = started(&reg);
    let me = state.active_player();
    let opp = state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != me)
        .unwrap();
    let quester = place_quester(&mut state, me, 100);
    let target = push_top(&mut state, opp, 901); // a character atop the opponent's deck

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    // 2 candidates (me / opp): the looker chooses whose deck.
    let _ =
        apply(&mut state, &reg, Input::Decide(Decision::ChoosePlayer(opp))).expect("choose player");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::TakeRevealed(Some(target))),
    )
    .expect("take");

    assert!(
        in_hand(&state, me, target),
        "the card enters the looker's hand"
    );
    assert!(
        !in_deck(&state, opp, target),
        "it left the chosen player's deck"
    );
}

fn named_def(id: u32, name: &str) -> CardDefinition {
    char_def(id).with_names(vec![name.to_string()])
}

fn lore(state: &GameState, player: PlayerId) -> u32 {
    state.player(player).unwrap().lore()
}

fn name_reveal_quester() -> CardDefinition {
    char_def(100).with_abilities(vec![TriggeredAbility::new(
        TriggerCondition::WhenThisQuests,
        Effect::NameThenReveal {
            lore_on_match: lorcana_engine::Amount::fixed(3),
            match_to: lorcana_engine::Destination::Hand,
            otherwise_to: lorcana_engine::Destination::Deck(DeckPosition::Bottom),
        },
    )])
}

#[test]
fn name_a_card_match_takes_to_hand_and_gains_lore() {
    let reg = registry(vec![name_reveal_quester(), named_def(901, "Mickey Mouse")]);
    let mut state = started(&reg);
    let me = state.active_player();
    let quester = place_quester(&mut state, me, 100);
    let top = push_top(&mut state, me, 901); // named "Mickey Mouse" atop the deck

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let lore_after_quest = lore(&state, me); // questing itself gained lore
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::NameCard("Mickey Mouse".to_string())),
    )
    .expect("name");

    assert!(in_hand(&state, me, top), "matched card goes to hand");
    assert_eq!(lore(&state, me), lore_after_quest + 3, "and gains 3 lore");
}

#[test]
fn name_a_card_miss_goes_to_the_bottom() {
    let reg = registry(vec![name_reveal_quester(), named_def(901, "Mickey Mouse")]);
    let mut state = started(&reg);
    let me = state.active_player();
    let quester = place_quester(&mut state, me, 100);
    let top = push_top(&mut state, me, 901);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let lore_after_quest = lore(&state, me);
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::NameCard("Donald Duck".to_string())),
    )
    .expect("name");

    assert!(!in_hand(&state, me, top), "missed: not taken to hand");
    assert!(
        in_deck(&state, me, top),
        "stays in the deck (on the bottom)"
    );
    assert_eq!(lore(&state, me), lore_after_quest, "no lore on a miss");
}

fn push_discard(state: &mut GameState, owner: PlayerId, def: u32) -> CardId {
    let id = state.allocate_card_id();
    let inst = CardInstance::new(id, CardDefId::from_raw(def), Conditions::faceup_idle());
    state.player_mut(owner).unwrap().discard_mut().push(inst);
    id
}

fn in_discard(state: &GameState, player: PlayerId, card: CardId) -> bool {
    state
        .player(player)
        .unwrap()
        .discard()
        .iter()
        .any(|c| c.id() == card)
}

fn blast_quester() -> CardDefinition {
    char_def(100).with_abilities(vec![TriggeredAbility::new(
        TriggerCondition::WhenThisQuests,
        Effect::NameThenRecur,
    )])
}

#[test]
fn name_a_card_recurs_all_matching_characters_from_discard() {
    // Blast from Your Past: name a card, return ALL character cards with that name
    // from your discard to your hand (other names stay).
    let reg = registry(vec![
        blast_quester(),
        named_def(901, "Mickey Mouse"),
        named_def(902, "Donald Duck"),
    ]);
    let mut state = started(&reg);
    let me = state.active_player();
    let quester = place_quester(&mut state, me, 100);
    let mickey_a = push_discard(&mut state, me, 901);
    let mickey_b = push_discard(&mut state, me, 901);
    let donald = push_discard(&mut state, me, 902);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::NameCard("Mickey Mouse".to_string())),
    )
    .expect("name");

    assert!(
        in_hand(&state, me, mickey_a) && in_hand(&state, me, mickey_b),
        "both Mickeys recur"
    );
    assert!(
        in_discard(&state, me, donald),
        "the non-matching card stays in the discard"
    );
}
