//! Integration tests for Slice 6a: the challenge-cluster keywords (Rush,
//! Evasive, Alert, Bodyguard, Resist, Challenger) plugged into the Slice 3
//! challenge legality/damage seam.

use lorcana_engine::{
    CardDefId, CardDefinition, CardId, CardInstance, CardRegistry, CharacterStats, Conditions,
    GameState, GameStatus, Input, Keyword, LocationStats, PlayerId, apply, start,
};

fn started(registry: &CardRegistry) -> GameState {
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

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != player)
        .unwrap()
}

/// Place a character in play referencing `def`, with the given stats/state.
#[allow(clippy::too_many_arguments)]
fn place(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    def: u32,
    strength: u32,
    willpower: u32,
    ready: bool,
    drying: bool,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut instance = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready,
            damage: 0,
            drying,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(strength, willpower, 1)));
    state.player_mut(owner).unwrap().play_mut().push(instance);
    id
}

fn damage(state: &GameState, owner: PlayerId, card: CardId) -> Option<u32> {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == card)
        .map(|c| c.conditions().damage)
}

const fn char_def(id: u32) -> CardDefinition {
    CardDefinition::character(CardDefId::from_raw(id), 1, true, 3, 3, 1)
}

#[test]
fn evasive_target_only_challengeable_by_evasive_or_alert() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(10).with_keywords(vec![Keyword::Evasive])); // target
    registry.insert(char_def(11)); // plain challenger
    registry.insert(char_def(12).with_keywords(vec![Keyword::Evasive])); // evasive challenger
    registry.insert(char_def(13).with_keywords(vec![Keyword::Alert])); // alert challenger
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let target = place(&mut state, foe, 200, 10, 1, 9, false, false);
    let plain = place(&mut state, active, 100, 11, 3, 9, true, false);
    let evasive = place(&mut state, active, 101, 12, 3, 9, true, false);
    let alert = place(&mut state, active, 102, 13, 3, 9, true, false);

    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: plain,
                target
            }
        )
        .is_err(),
        "a non-Evasive character can't challenge an Evasive target"
    );
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: evasive,
                target
            }
        )
        .is_ok()
    );
    // Fresh target (the previous one took damage / may be exerted-as-is): use alert.
    let target2 = place(&mut state, foe, 201, 10, 1, 9, false, false);
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: alert,
                target: target2
            }
        )
        .is_ok(),
        "Alert ignores Evasive's restriction"
    );
}

#[test]
fn rush_lets_a_drying_character_challenge() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(50).with_keywords(vec![Keyword::Rush]));
    registry.insert(char_def(51));
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let target = place(&mut state, foe, 200, 51, 1, 9, false, false);
    // Challenger is drying (just played) but has Rush.
    let rusher = place(&mut state, active, 100, 50, 3, 9, true, true);

    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: rusher,
                target
            }
        )
        .is_ok()
    );
}

#[test]
fn resist_reduces_challenge_damage() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(20)); // challenger, strength 3
    registry.insert(char_def(21).with_keywords(vec![Keyword::Resist(2)])); // target
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place(&mut state, active, 100, 20, 3, 9, true, false);
    let target = place(&mut state, foe, 200, 21, 1, 9, false, false);

    let _ = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    )
    .expect("challenge");
    // 3 strength minus Resist 2 = 1 damage.
    assert_eq!(damage(&state, foe, target), Some(1));
}

#[test]
fn challenger_adds_strength_while_challenging() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(30).with_keywords(vec![Keyword::Challenger(2)])); // challenger
    registry.insert(char_def(31)); // target
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place(&mut state, active, 100, 30, 3, 9, true, false);
    let target = place(&mut state, foe, 200, 31, 1, 9, false, false);

    let _ = apply(
        &mut state,
        &registry,
        Input::Challenge { challenger, target },
    )
    .expect("challenge");
    // 3 base + Challenger 2 = 5 damage dealt to the target.
    assert_eq!(damage(&state, foe, target), Some(5));
}

#[test]
fn bodyguard_must_be_challenged_if_able() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(40).with_keywords(vec![Keyword::Bodyguard]));
    registry.insert(char_def(41)); // plain
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let challenger = place(&mut state, active, 100, 41, 3, 9, true, false);
    let guard = place(&mut state, foe, 200, 40, 1, 9, false, false); // exerted Bodyguard
    let plain = place(&mut state, foe, 201, 41, 1, 9, false, false); // exerted non-guard

    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger,
                target: plain
            }
        )
        .is_err(),
        "must choose the Bodyguard while it's a legal target"
    );
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger,
                target: guard
            }
        )
        .is_ok(),
        "challenging the Bodyguard itself is allowed"
    );
}

#[test]
fn evasive_bodyguard_does_not_trap_a_non_evasive_challenger() {
    let mut registry = CardRegistry::new();
    // A Bodyguard that is also Evasive.
    registry.insert(char_def(60).with_keywords(vec![Keyword::Bodyguard, Keyword::Evasive]));
    registry.insert(char_def(41)); // plain
    registry.insert(char_def(12).with_keywords(vec![Keyword::Evasive])); // evasive challenger
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let plain_challenger = place(&mut state, active, 100, 41, 3, 9, true, false);
    let evasive_challenger = place(&mut state, active, 101, 12, 3, 9, true, false);
    let guard = place(&mut state, foe, 200, 60, 1, 9, false, false); // Bodyguard + Evasive
    let plain_target = place(&mut state, foe, 201, 41, 1, 9, false, false);

    // Non-Evasive challenger: can't challenge the Evasive Bodyguard, and is NOT
    // forced to (it isn't a legal target for them), so it may challenge the plain.
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: plain_challenger,
                target: guard
            }
        )
        .is_err()
    );
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: plain_challenger,
                target: plain_target
            }
        )
        .is_ok(),
        "an Evasive Bodyguard must not trap a non-Evasive challenger"
    );

    // An Evasive challenger, by contrast, IS forced to the Bodyguard.
    let plain_target2 = place(&mut state, foe, 202, 41, 1, 9, false, false);
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: evasive_challenger,
                target: plain_target2
            }
        )
        .is_err(),
        "an Evasive challenger can reach the Bodyguard, so must choose it"
    );
}

#[test]
fn reckless_cannot_quest() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(70).with_keywords(vec![Keyword::Reckless]));
    let mut state = started(&registry);
    let active = state.active_player();

    let reckless = place(&mut state, active, 100, 70, 3, 9, true, false);
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Quest {
                character: reckless
            }
        )
        .is_err(),
        "a Reckless character can't quest (§10.7.2)"
    );
}

#[test]
fn reckless_blocks_ending_the_turn_while_it_can_challenge() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(70).with_keywords(vec![Keyword::Reckless]));
    registry.insert(char_def(41)); // plain
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let _reckless = place(&mut state, active, 100, 70, 3, 9, true, false); // ready Reckless
    let _exerted_foe = place(&mut state, foe, 200, 41, 1, 9, false, false); // a legal target

    assert!(
        apply(&mut state, &registry, Input::EndTurn).is_err(),
        "can't end the turn while a ready Reckless character can challenge (§10.7.3)"
    );
}

#[test]
fn reckless_must_challenge_an_opposing_location() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(70).with_keywords(vec![Keyword::Reckless]));
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);
    let _reckless = place(&mut state, active, 100, 70, 3, 9, true, false); // ready Reckless
    // The opponent has only a location (no exerted characters); a location can be
    // challenged any time, so Reckless still can't end the turn (§10.7.3).
    let mut location = CardInstance::new(
        CardId::from_raw(200),
        CardDefId::from_raw(200),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    location.set_location_stats(Some(LocationStats::new(3, 0, 1)));
    state.player_mut(foe).unwrap().play_mut().push(location);

    assert!(
        apply(&mut state, &registry, Input::EndTurn).is_err(),
        "Reckless must challenge an opposing location too (§10.7.3)"
    );
}

#[test]
fn reckless_allows_ending_the_turn_with_no_legal_challenge() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(70).with_keywords(vec![Keyword::Reckless]));
    registry.insert(char_def(41)); // plain
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let _reckless = place(&mut state, active, 100, 70, 3, 9, true, false); // ready Reckless
    // The only opposing character is ready, so it can't be challenged.
    let _ready_foe = place(&mut state, foe, 200, 41, 1, 9, true, false);

    assert!(
        apply(&mut state, &registry, Input::EndTurn).is_ok(),
        "a Reckless character with no legal challenge doesn't block ending the turn"
    );
}

#[test]
fn with_multiple_bodyguards_either_one_may_be_challenged() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(40).with_keywords(vec![Keyword::Bodyguard]));
    registry.insert(char_def(41)); // plain
    let mut state = started(&registry);
    let active = state.active_player();
    let foe = opponent_of(&state, active);

    let c1 = place(&mut state, active, 100, 41, 3, 9, true, false);
    let c2 = place(&mut state, active, 101, 41, 3, 9, true, false);
    let guard_a = place(&mut state, foe, 200, 40, 1, 9, false, false);
    let guard_b = place(&mut state, foe, 201, 40, 1, 9, false, false);
    let plain = place(&mut state, foe, 202, 41, 1, 9, false, false);

    // A non-Bodyguard is off-limits while any exerted Bodyguard is present.
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: c1,
                target: plain
            }
        )
        .is_err()
    );
    // Either Bodyguard is a legal choice.
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: c1,
                target: guard_a
            }
        )
        .is_ok()
    );
    assert!(
        apply(
            &mut state,
            &registry,
            Input::Challenge {
                challenger: c2,
                target: guard_b
            }
        )
        .is_ok()
    );
}

#[test]
fn boost_puts_a_facedown_card_under_the_character() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(80).with_keywords(vec![Keyword::Boost(0)]));
    let mut state = started(&registry);
    let active = state.active_player();
    let booster = place(&mut state, active, 100, 80, 3, 9, true, false);
    let deck_before = state.player(active).unwrap().deck().iter().count();

    let _ = apply(&mut state, &registry, Input::Boost { card: booster }).expect("boost");

    let player = state.player(active).unwrap();
    let inst = player.play().iter().find(|c| c.id() == booster).unwrap();
    assert_eq!(inst.under().len(), 1, "a card was put under the character");
    assert!(
        inst.under()[0].conditions().facedown,
        "the Boost card stays facedown (§10.4.3)"
    );
    assert_eq!(
        player.deck().iter().count(),
        deck_before - 1,
        "the top deck card moved under the character"
    );
}

#[test]
fn boost_can_only_be_used_once_per_turn() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(80).with_keywords(vec![Keyword::Boost(0)]));
    let mut state = started(&registry);
    let active = state.active_player();
    let booster = place(&mut state, active, 100, 80, 3, 9, true, false);

    assert!(apply(&mut state, &registry, Input::Boost { card: booster }).is_ok());
    assert!(
        apply(&mut state, &registry, Input::Boost { card: booster }).is_err(),
        "Boost is once per turn (§10.4.1)"
    );
}

#[test]
fn a_character_without_boost_cannot_boost() {
    let mut registry = CardRegistry::new();
    registry.insert(char_def(81)); // no Boost
    let mut state = started(&registry);
    let active = state.active_player();
    let plain = place(&mut state, active, 100, 81, 3, 9, true, false);

    assert!(apply(&mut state, &registry, Input::Boost { card: plain }).is_err());
}
