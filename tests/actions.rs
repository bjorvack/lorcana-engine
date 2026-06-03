//! Integration tests for Slice 7a: actions and songs — playing an action
//! resolves its effect and discards it; songs can be sung by exerting eligible
//! characters (Singer / Sing Together).

use lorcana_engine::{
    Amount, CardCategory, CardDefId, CardDefinition, CardId, CardInstance, CardKind, CardRegistry,
    CharacterFilter, CharacterStats, Classification, Conditions, Decision, Effect, GameState,
    GameStatus, Input, Keyword, PlayerId, PlayerScope, Target, TargetSide, TriggerCondition,
    TriggeredAbility, apply, start,
};

fn action_card(id: u32, cost: u32, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition::new(CardDefId::from_raw(id), cost, true, CardKind::Action)
        .with_action_effects(effects)
}

fn song_card(id: u32, cost: u32, keywords: Vec<Keyword>, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition::new(CardDefId::from_raw(id), cost, true, CardKind::Action)
        .with_classifications(vec![Classification::new("Song")])
        .with_keywords(keywords)
        .with_action_effects(effects)
}

fn singer_char(id: u32, cost: u32, singer: Option<u32>) -> CardDefinition {
    let keywords = singer.map(Keyword::Singer).into_iter().collect();
    CardDefinition::character(CardDefId::from_raw(id), cost, true, 1, 1, 1).with_keywords(keywords)
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

fn place_character(
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
    inst.set_stats(Some(CharacterStats::new(1, 1, 1)));
    state.player_mut(owner).unwrap().play_mut().push(inst);
    id
}

fn hand_card(state: &GameState, nth: usize) -> CardId {
    state
        .player(state.active_player())
        .unwrap()
        .hand()
        .iter()
        .nth(nth)
        .unwrap()
        .id()
}

fn lore(state: &GameState, player: PlayerId) -> u32 {
    state.player(player).unwrap().lore()
}

fn is_ready(state: &GameState, player: PlayerId, card: CardId) -> bool {
    state
        .player(player)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .unwrap()
        .conditions()
        .ready
}

#[test]
fn playing_an_action_resolves_its_effect_and_discards_it() {
    let reg: CardRegistry = (0..30)
        .map(|n| {
            action_card(
                n,
                0,
                vec![Effect::Lore {
                    who: PlayerScope::You,
                    amount: Amount::fixed(2),
                }],
            )
        })
        .collect();
    let mut state = started(&reg);
    let active = state.active_player();
    let card = hand_card(&state, 0);
    let hand_before = state.player(active).unwrap().hand().iter().count();

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card,
            shift_onto: None,
        },
    )
    .expect("play action");

    assert_eq!(lore(&state, active), 2, "the action's effect resolved");
    assert!(
        state.player(active).unwrap().discard().contains(card),
        "the action went to discard"
    );
    assert_eq!(
        state.player(active).unwrap().hand().iter().count(),
        hand_before - 1
    );
}

/// A registry whose deck cards are the song under test, plus the given extra defs.
fn song_reg(cost: u32, keywords: &[Keyword], extra: Vec<CardDefinition>) -> CardRegistry {
    let mut r: CardRegistry = (0..30)
        .map(|n| {
            song_card(
                n,
                cost,
                keywords.to_vec(),
                vec![Effect::Lore {
                    who: PlayerScope::You,
                    amount: Amount::fixed(2),
                }],
            )
        })
        .collect();
    for d in extra {
        r.insert(d);
    }
    r
}

#[test]
fn a_song_can_be_sung_by_exerting_an_eligible_character() {
    let reg = song_reg(3, &[], vec![singer_char(200, 4, None)]);
    let mut state = started(&reg);
    let active = state.active_player();
    let singer = place_character(&mut state, active, 100, 200, true, false);
    let song = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::Sing {
            song,
            singers: vec![singer],
        },
    )
    .expect("sing");

    assert_eq!(lore(&state, active), 2, "the song's effect resolved");
    assert!(!is_ready(&state, active, singer), "the singer is exerted");
    assert!(state.player(active).unwrap().discard().contains(song));
}

#[test]
fn singing_rejects_a_character_whose_cost_is_too_low() {
    let reg = song_reg(3, &[], vec![singer_char(200, 2, None)]); // cost 2 < song 3
    let mut state = started(&reg);
    let active = state.active_player();
    let singer = place_character(&mut state, active, 100, 200, true, false);
    let song = hand_card(&state, 0);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Sing {
                song,
                singers: vec![singer]
            }
        )
        .is_err()
    );
}

#[test]
fn singer_keyword_lets_a_cheap_character_sing() {
    let reg = song_reg(3, &[], vec![singer_char(200, 1, Some(5))]); // Singer 5
    let mut state = started(&reg);
    let active = state.active_player();
    let singer = place_character(&mut state, active, 100, 200, true, false);
    let song = hand_card(&state, 0);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Sing {
                song,
                singers: vec![singer]
            }
        )
        .is_ok(),
        "Singer 5 can sing a cost-3 song (§10.11)"
    );
}

#[test]
fn sing_together_combines_singer_costs() {
    let reg = song_reg(
        5,
        &[Keyword::SingTogether(4)],
        vec![singer_char(200, 2, None)],
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let s1 = place_character(&mut state, active, 100, 200, true, false);
    let s2 = place_character(&mut state, active, 101, 200, true, false);
    let song = hand_card(&state, 0);

    // Neither alone could sing a cost-5 song, but together (2 + 2 ≥ 4) they can.
    assert!(
        apply(
            &mut state,
            &reg,
            Input::Sing {
                song,
                singers: vec![s1, s2]
            }
        )
        .is_ok(),
        "Sing Together combines costs (§10.12)"
    );
}

#[test]
fn a_drying_character_cannot_sing() {
    let reg = song_reg(3, &[], vec![singer_char(200, 4, None)]);
    let mut state = started(&reg);
    let active = state.active_player();
    let singer = place_character(&mut state, active, 100, 200, true, true); // drying
    let song = hand_card(&state, 0);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Sing {
                song,
                singers: vec![singer]
            }
        )
        .is_err(),
        "a freshly-played (drying) character can't exert to sing"
    );
}

#[test]
fn singing_a_song_fires_a_play_a_song_watcher() {
    let watcher = CardDefinition::character(CardDefId::from_raw(300), 1, true, 1, 1, 1)
        .with_abilities(vec![TriggeredAbility::new(
            TriggerCondition::WhenYouPlay(CardCategory::Song),
            Effect::Lore {
                who: PlayerScope::You,
                amount: Amount::fixed(1),
            },
        )]);
    let reg = song_reg(3, &[], vec![singer_char(200, 4, None), watcher]);
    let mut state = started(&reg);
    let active = state.active_player();
    let singer = place_character(&mut state, active, 100, 200, true, false);
    let _watch = place_character(&mut state, active, 102, 300, true, false);
    let song = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::Sing {
            song,
            singers: vec![singer],
        },
    )
    .expect("sing");

    // Song's GainLore(2) plus the watcher's GainLore(1).
    assert_eq!(lore(&state, active), 3, "the play-a-song watcher fired");
}

#[test]
fn a_targeted_action_suspends_to_choose_then_resolves() {
    // Every deck card is "Deal 2 damage to chosen opposing character".
    let mut reg: CardRegistry = (0..30)
        .map(|n| {
            action_card(
                n,
                0,
                vec![Effect::DealDamage {
                    target: Target::ChosenCharacter {
                        filter: CharacterFilter::any(TargetSide::Opposing),
                    },
                    amount: Amount::fixed(2),
                }],
            )
        })
        .collect();
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(100),
        1,
        true,
        1,
        1,
        1,
    ));
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != active)
        .unwrap();
    let victim = place_character(&mut state, foe, 5000, 100, true, false); // willpower 1
    let action = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: action,
            shift_onto: None,
        },
    )
    .expect("play action");
    assert!(
        state.is_awaiting_decision(),
        "the action waits for a target"
    );
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(victim)),
    )
    .expect("choose target");

    // 2 damage to a 1-willpower character banishes it.
    assert!(!state.player(foe).unwrap().play().contains(victim));
    assert!(state.player(foe).unwrap().discard().contains(victim));
    assert!(
        state.player(active).unwrap().discard().contains(action),
        "action discarded"
    );
}

#[test]
fn a_multi_effect_action_resolves_the_rest_after_the_choice() {
    // "Deal 2 damage to chosen opposing character. Gain 3 lore." (Energy-Blast-like)
    let mut reg: CardRegistry = (0..30)
        .map(|n| {
            action_card(
                n,
                0,
                vec![
                    Effect::DealDamage {
                        target: Target::ChosenCharacter {
                            filter: CharacterFilter::any(TargetSide::Opposing),
                        },
                        amount: Amount::fixed(2),
                    },
                    Effect::Lore {
                        who: PlayerScope::You,
                        amount: Amount::fixed(3),
                    },
                ],
            )
        })
        .collect();
    reg.insert(CardDefinition::character(
        CardDefId::from_raw(100),
        1,
        true,
        1,
        1,
        1,
    ));
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != active)
        .unwrap();
    let victim = place_character(&mut state, foe, 5000, 100, true, false);
    let action = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: action,
            shift_onto: None,
        },
    )
    .expect("play action");
    // Suspended on the first clause; the second clause ("gain 3 lore") must NOT
    // have resolved yet (§7.1.2 "[A] then [B]" resolves in order).
    assert!(state.is_awaiting_decision());
    assert_eq!(
        lore(&state, active),
        0,
        "the 'then' clause waits for the choice"
    );

    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(Decision::ChooseTarget(victim)),
    )
    .expect("choose target");

    assert!(
        state.player(foe).unwrap().discard().contains(victim),
        "first clause: banished"
    );
    assert_eq!(
        lore(&state, active),
        3,
        "second clause resolved after the choice"
    );
    assert!(!state.is_awaiting_decision());
}

#[test]
fn ward_target_does_as_much_as_you_can_still_draws() {
    // §1.2.3 worked example (Cogsworth has Ward): "Deal 2 damage to chosen
    // character. Draw a card." With only a Warded opposing character to choose,
    // you can't deal the damage, so you do as much as you can — draw the card.
    let mut reg: CardRegistry = (0..30)
        .map(|n| {
            action_card(
                n,
                0,
                vec![
                    Effect::DealDamage {
                        target: Target::ChosenCharacter {
                            filter: CharacterFilter::any(TargetSide::Opposing),
                        },
                        amount: Amount::fixed(2),
                    },
                    Effect::Draw {
                        who: PlayerScope::You,
                        amount: Amount::fixed(1),
                    },
                ],
            )
        })
        .collect();
    reg.insert(
        CardDefinition::character(CardDefId::from_raw(100), 1, true, 2, 5, 1)
            .with_keywords(vec![Keyword::Ward]),
    );
    let mut state = started(&reg);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let cogsworth = place_character(&mut state, foe, 1000, 100, true, false);
    let hand_before = state.player(active).unwrap().hand().iter().count();
    let card = hand_card(&state, 0);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card,
            shift_onto: None,
        },
    )
    .expect("play action");

    assert!(
        state.pending().is_none(),
        "no Warded target to choose; no decision"
    );
    let damage = state
        .player(foe)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == cogsworth)
        .unwrap()
        .conditions()
        .damage;
    assert_eq!(damage, 0, "Ward prevented the damage being dealt");
    // Played 1 (action -> discard) and drew 1: hand size is unchanged, proving
    // the draw still resolved (§1.2.3 "do as much as you can").
    assert_eq!(
        state.player(active).unwrap().hand().iter().count(),
        hand_before
    );
    assert!(state.player(active).unwrap().discard().contains(card));
}

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != player)
        .unwrap()
}
