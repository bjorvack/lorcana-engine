//! Rules conformance (§7–§10): golden tests citing the comprehensive rules
//! (`docs/rules/`). Every card here is authored in the engine's **TOML DSL** and
//! loaded via `load_toml`, so each test also proves the loader/DSL produce
//! rules-correct cards end-to-end.

use lorcana_engine::{
    CardDefId, CardId, CardInstance, CardRegistry, CharacterStats, Conditions, GameState,
    GameStatus, Input, PlayerId, apply, load_toml, start,
};

fn registry_from(toml: &str) -> CardRegistry {
    let mut reg = CardRegistry::new();
    for def in load_toml(toml).expect("cards load") {
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

fn opponent_of(state: &GameState, player: PlayerId) -> PlayerId {
    state
        .players()
        .iter()
        .map(lorcana_engine::PlayerState::id)
        .find(|p| *p != player)
        .unwrap()
}

fn place(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    def: u32,
    strength: u32,
    willpower: u32,
    ready: bool,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut instance = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready,
            damage: 0,
            drying: false,
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

fn lore(state: &GameState, p: PlayerId) -> u32 {
    state.player(p).unwrap().lore()
}

fn hand_len(state: &GameState, p: PlayerId) -> usize {
    state.player(p).unwrap().hand().iter().count()
}

/// §10.8 — "Resist +N": damage dealt to this character is reduced by N.
#[test]
fn resist_reduces_challenge_damage() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Striker"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        [[card]]
        name = "Tank"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        keywords = ["Resist 2"]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let striker = place(&mut state, me, 100, 0, 3, 9, true);
    let tank = place(&mut state, foe, 200, 1, 1, 9, false);

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: striker,
            target: tank,
        },
    )
    .expect("challenge");
    assert_eq!(
        damage(&state, foe, tank),
        Some(1),
        "3 strength − Resist 2 = 1"
    );
}

/// §10.5 — "Challenger +N": while challenging, this character gets +N {S}.
#[test]
fn challenger_adds_strength_while_challenging() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Bruiser"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        keywords = ["Challenger 2"]
        [[card]]
        name = "Dummy"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let bruiser = place(&mut state, me, 100, 0, 3, 9, true);
    let dummy = place(&mut state, foe, 200, 1, 1, 9, false);

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: bruiser,
            target: dummy,
        },
    )
    .expect("challenge");
    assert_eq!(
        damage(&state, foe, dummy),
        Some(5),
        "3 base + Challenger 2 = 5"
    );
}

/// §10.6 — "Evasive": can only be challenged by characters with Evasive.
#[test]
fn evasive_can_only_be_challenged_by_evasive() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Flyer"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        keywords = ["Evasive"]
        [[card]]
        name = "Grounded"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let target = place(&mut state, foe, 200, 0, 1, 9, false); // Evasive
    let grounded = place(&mut state, me, 100, 1, 3, 9, true);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Challenge {
                challenger: grounded,
                target,
            },
        )
        .is_err(),
        "a non-Evasive character can't challenge an Evasive one"
    );
}

/// §7.1.2 — an effect with multiple parts resolves its parts in order.
#[test]
fn an_abilitys_parts_resolve_in_order() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Scholar"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = [{ draw = 1 }, { gain_lore = 1 }]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let scholar = place(&mut state, me, 100, 0, 1, 5, true);
    let lore_before = lore(&state, me);
    let hand_before = hand_len(&state, me);

    let _ = apply(&mut state, &reg, Input::Quest { character: scholar }).expect("quest");
    assert_eq!(hand_len(&state, me), hand_before + 1, "drew a card");
    // questing lore (1) + the ability's +1.
    assert_eq!(lore(&state, me), lore_before + 1 + 1, "both parts resolved");
}

fn in_play(state: &GameState, owner: PlayerId, card: CardId) -> bool {
    state
        .player(owner)
        .unwrap()
        .play()
        .iter()
        .any(|c| c.id() == card)
}

/// §10.3 — "Bodyguard": an opposing character with Bodyguard must be chosen as a
/// challenge target if able (a non-Bodyguard can't be challenged while it stands).
#[test]
fn bodyguard_must_be_challenged_if_able() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Guard"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        keywords = ["Bodyguard"]
        [[card]]
        name = "Civilian"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        [[card]]
        name = "Attacker"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let _guard = place(&mut state, foe, 200, 0, 1, 9, false);
    let civilian = place(&mut state, foe, 201, 1, 1, 9, false);
    let attacker = place(&mut state, me, 100, 2, 3, 9, true);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Challenge {
                challenger: attacker,
                target: civilian,
            },
        )
        .is_err(),
        "can't challenge the non-Bodyguard while a Bodyguard is in play"
    );
}

/// §10.15 — "Ward": this character can't be chosen by opponents' effects, so a
/// "banish chosen opposing character" with only a Ward target does nothing.
#[test]
fn ward_cannot_be_chosen_by_an_effect() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Assassin"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { banish = "chosen opposing character" }
        [[card]]
        name = "Protected"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        keywords = ["Ward"]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let assassin = place(&mut state, me, 100, 0, 1, 5, true);
    let protected = place(&mut state, foe, 200, 1, 1, 5, false);

    let _ = apply(
        &mut state,
        &reg,
        Input::Quest {
            character: assassin,
        },
    )
    .expect("quest");
    assert!(
        state.pending().is_none(),
        "no target to choose — Ward isn't choosable"
    );
    assert!(
        in_play(&state, foe, protected),
        "the Ward character survives"
    );
}

/// §9 — a character with damage ≥ its willpower is banished.
#[test]
fn lethal_damage_banishes() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Brute"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        [[card]]
        name = "Fragile"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 3
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let brute = place(&mut state, me, 100, 0, 3, 9, true);
    let fragile = place(&mut state, foe, 200, 1, 1, 3, false);

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: brute,
            target: fragile,
        },
    )
    .expect("challenge");
    assert!(
        !in_play(&state, foe, fragile),
        "3 damage ≥ 3 willpower → banished"
    );
}

/// §10.2 — "Alert": ignores Evasive's challenging restriction, so an Alert
/// character may challenge an Evasive one.
#[test]
fn alert_may_challenge_an_evasive_character() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Flyer"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        keywords = ["Evasive"]
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        keywords = ["Alert"]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let flyer = place(&mut state, foe, 200, 0, 1, 9, false);
    let watcher = place(&mut state, me, 100, 1, 3, 9, true);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Challenge {
                challenger: watcher,
                target: flyer,
            },
        )
        .is_ok(),
        "Alert ignores the Evasive challenge restriction"
    );
    assert_eq!(
        damage(&state, foe, flyer),
        Some(3),
        "challenge damage was dealt"
    );
}

fn in_hand(state: &GameState, owner: PlayerId, card: CardId) -> bool {
    state
        .player(owner)
        .unwrap()
        .hand()
        .iter()
        .any(|c| c.id() == card)
}

fn place_drying(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    def: u32,
    strength: u32,
    willpower: u32,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut instance = CardInstance::new(
        id,
        CardDefId::from_raw(def),
        Conditions {
            ready: true,
            damage: 0,
            drying: true,
            facedown: false,
        },
    );
    instance.set_stats(Some(CharacterStats::new(strength, willpower, 1)));
    state.player_mut(owner).unwrap().play_mut().push(instance);
    id
}

/// §8 — "return to hand": a bounced character leaves play for its owner's hand.
#[test]
fn return_to_hand_bounces_a_character() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Bouncer"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { return_to_hand = "chosen opposing character" }
        [[card]]
        name = "Victim"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let bouncer = place(&mut state, me, 100, 0, 1, 5, true);
    let victim = place(&mut state, foe, 200, 1, 1, 5, false);

    let _ = apply(&mut state, &reg, Input::Quest { character: bouncer }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(victim)),
    )
    .expect("choose target");

    assert!(!in_play(&state, foe, victim), "left play");
    assert!(in_hand(&state, foe, victim), "returned to its owner's hand");
}

/// §10.7 — "Reckless": this character can't quest.
#[test]
fn reckless_cannot_quest() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Berserker"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 5
        lore = 2
        keywords = ["Reckless"]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let berserker = place(&mut state, me, 100, 0, 3, 5, true);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Quest {
                character: berserker
            }
        )
        .is_err(),
        "a Reckless character can't be sent to quest"
    );
}

/// §10.9 — "Rush": this character can challenge the turn it's played (ignores drying).
#[test]
fn rush_can_challenge_while_drying() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Charger"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        keywords = ["Rush"]
        [[card]]
        name = "Bystander"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let charger = place_drying(&mut state, me, 100, 0, 3, 9); // just "played" -> drying
    let bystander = place(&mut state, foe, 200, 1, 1, 9, false);

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Challenge {
                challenger: charger,
                target: bystander,
            },
        )
        .is_ok(),
        "Rush lets a drying character challenge"
    );
}

fn in_inkwell(state: &GameState, owner: PlayerId, card: CardId) -> bool {
    state
        .player(owner)
        .unwrap()
        .inkwell()
        .iter()
        .any(|c| c.id() == card)
}

/// §8 — "into inkwell": a chosen character is put into its owner's inkwell.
#[test]
fn into_inkwell_moves_a_character_to_the_inkwell() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Enchanter"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { into_inkwell = "chosen opposing character" }
        [[card]]
        name = "Mark"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let enchanter = place(&mut state, me, 100, 0, 1, 5, true);
    let mark = place(&mut state, foe, 200, 1, 1, 5, false);

    let _ = apply(
        &mut state,
        &reg,
        Input::Quest {
            character: enchanter,
        },
    )
    .expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(mark)),
    )
    .expect("choose");

    assert!(!in_play(&state, foe, mark), "left play");
    assert!(in_inkwell(&state, foe, mark), "into its owner's inkwell");
}

/// §6.5 — a character can move to a location (here a TOML-loaded location with
/// move cost 0), and is recorded as being there.
#[test]
fn a_character_moves_to_a_loaded_location() {
    use lorcana_engine::LocationStats;
    let reg = registry_from(
        r#"
        [[card]]
        name = "Sleepy Hollow"
        type = "Location"
        cost = 2
        inkwell = true
        move_cost = 0
        willpower = 5
        lore = 1
        [[card]]
        name = "Traveler"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let traveler = place(&mut state, me, 100, 1, 1, 5, true);

    // Put the loaded location (def 0) in play with its stats.
    let loc = CardId::from_raw(300);
    let mut inst = CardInstance::new(loc, CardDefId::from_raw(0), Conditions::faceup_idle());
    inst.set_location_stats(Some(LocationStats::new(5, 1, 0)));
    state.player_mut(me).unwrap().play_mut().push(inst);

    let _ = apply(
        &mut state,
        &reg,
        Input::MoveCharacter {
            character: traveler,
            location: loc,
        },
    )
    .expect("move to location");

    assert_eq!(
        state
            .player(me)
            .unwrap()
            .play()
            .iter()
            .find(|c| c.id() == traveler)
            .unwrap()
            .at_location(),
        Some(loc),
        "the character is recorded at the location"
    );
}

/// §7.4 — "Whenever you play an action": a watcher's ability fires when its
/// controller plays a matching card.
#[test]
fn whenever_you_play_an_action_fires() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "play_action"
        do = { gain_lore = 1 }
        [[card]]
        name = "Zap"
        type = "Action"
        cost = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let _ = place(&mut state, me, 100, 0, 1, 5, true); // the Watcher

    // Put the action (def 1) in hand + 1 ready ink to play it.
    let zap = CardId::from_raw(300);
    state
        .player_mut(me)
        .unwrap()
        .hand_mut()
        .push(CardInstance::new(
            zap,
            CardDefId::from_raw(1),
            Conditions::faceup_idle(),
        ));
    let ink = CardId::from_raw(301);
    state
        .player_mut(me)
        .unwrap()
        .inkwell_mut()
        .push(CardInstance::new(
            ink,
            CardDefId::from_raw(1),
            Conditions::faceup_idle(),
        ));
    let lore_before = lore(&state, me);

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: zap,
            shift_onto: None,
        },
    )
    .expect("play action");

    assert_eq!(
        lore(&state, me),
        lore_before + 1,
        "the play-an-action trigger fired"
    );
}

/// DSL design — the hybrid surface also accepts the **structured AST form** for a
/// leaf selector (here a `Target` written as a TOML table), so anything the
/// compact string grammar can't express still round-trips via serde.
#[test]
fn the_structured_target_fallback_resolves() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Sniper"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { banish = { ChosenCharacter = { filter = { Side = "Opposing" } } } }
        [[card]]
        name = "Prey"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let sniper = place(&mut state, me, 100, 0, 1, 5, true);
    let prey = place(&mut state, foe, 200, 1, 1, 5, false);

    let _ = apply(&mut state, &reg, Input::Quest { character: sniper }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(prey)),
    )
    .expect("choose");

    assert!(
        !in_play(&state, foe, prey),
        "structured-target banish resolved"
    );
}

/// DSL / §7.1 — the compact selector grammar exposes a current-`{S}` threshold
/// (`"with N {S} or more"`). Only characters at/above the threshold are
/// choosable: a strength-2 character can't be picked while a strength-4 one can,
/// proving the parsed `CharacterFilter::Strength` predicate gates resolution
/// end-to-end (authored in TOML, loaded via `load_toml`, resolved by the engine).
#[test]
fn dsl_strength_threshold_selector_filters_choices() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Hunter"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { banish = "chosen opposing character with 3 {S} or more" }
        [[card]]
        name = "Weakling"
        type = "Character"
        cost = 1
        strength = 2
        willpower = 5
        lore = 1
        [[card]]
        name = "Bruiser"
        type = "Character"
        cost = 1
        strength = 4
        willpower = 5
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let hunter = place(&mut state, me, 100, 0, 1, 5, true);
    let weak = place(&mut state, foe, 200, 1, 2, 5, false);
    let bruiser = place(&mut state, foe, 300, 2, 4, 5, false);

    let _ = apply(&mut state, &reg, Input::Quest { character: hunter }).expect("quest");

    // The strength-2 character is below the threshold, so it isn't a legal pick.
    let rejected = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(weak)),
    );
    assert!(
        rejected.is_err(),
        "a strength-2 character can't be chosen by a `3 {{S}} or more` selector"
    );

    // The strength-4 character is at/above the threshold, so it resolves.
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(bruiser)),
    )
    .expect("choose the qualifying character");

    assert!(
        !in_play(&state, foe, bruiser),
        "the strength>=3 character is banished"
    );
    assert!(
        in_play(&state, foe, weak),
        "the below-threshold character survives"
    );
}

/// §7.6 — a permanent keyword grant ("gains Evasive") persists across turns,
/// unlike a this-turn grant which expires at end of turn.
#[test]
fn a_permanent_keyword_grant_persists_across_end_of_turn() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Empowerer"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { grant_keyword = "Evasive", to = "self", duration = "permanent" }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let hero = place(&mut state, me, 100, 0, 1, 5, true);

    let _ = apply(&mut state, &reg, Input::Quest { character: hero }).expect("quest");
    assert!(
        state
            .granted_keywords(hero)
            .contains(&lorcana_engine::Keyword::Evasive),
        "the character gained Evasive"
    );

    // End the turn; a permanent grant must survive the end-of-turn sweep.
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    assert!(
        state
            .granted_keywords(hero)
            .contains(&lorcana_engine::Keyword::Evasive),
        "Evasive persists into the next turn (not a this-turn grant)"
    );
}

/// §7.1 — a count-threshold conditional ("if you have N or more …") fires only
/// when the controller has at least N matching characters.
#[test]
fn count_threshold_conditional_gates_on_how_many_you_have() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Rallier"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { if_you_have = "your characters", at_least = 3, then = { gain_lore = 3 } }
        [[card]]
        name = "Friend"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );

    // Rallier + 1 friend = 2 of your characters < 3 -> threshold not met.
    let mut state = started(&reg);
    let me = state.active_player();
    let rallier = place(&mut state, me, 100, 0, 1, 5, true);
    let _f1 = place(&mut state, me, 101, 1, 1, 5, true);
    let before = lore(&state, me);
    let _ = apply(&mut state, &reg, Input::Quest { character: rallier }).expect("quest");
    assert_eq!(
        lore(&state, me),
        before + 1,
        "threshold not met -> only quest lore"
    );

    // Rallier + 2 friends = 3 -> threshold met -> +3 bonus.
    let mut state2 = started(&reg);
    let r2 = place(&mut state2, me, 100, 0, 1, 5, true);
    let _a = place(&mut state2, me, 101, 1, 1, 5, true);
    let _b = place(&mut state2, me, 102, 1, 1, 5, true);
    let before2 = lore(&state2, me);
    let _ = apply(&mut state2, &reg, Input::Quest { character: r2 }).expect("quest");
    assert_eq!(
        lore(&state2, me),
        before2 + 1 + 3,
        "threshold met -> bonus lore"
    );
}

#[test]
fn shift_conditional_trigger_fires_on_shifted_play() {
    // Test: Character has a trigger that only fires when played with shift
    let reg = registry_from(
        r#"
        [[card]]
        name = "ShiftBonus"
        type = "Character"
        cost = 5
        strength = 3
        willpower = 4
        lore = 1
        keywords = ["Shift 4"]
        [[card.abilities]]
        on = "play_with_shift"
        do = { draw = 2 }
        [[card]]
        name = "Base"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );

    // Regular play (no shift) -> shift trigger doesn't fire
    let mut state = started(&reg);
    let me = state.active_player();
    let before_hand = state.player(me).unwrap().hand().len();
    let _ = place(&mut state, me, 100, 0, 3, 4, true);
    // Regular play only draws 1 (from playing a character)
    assert_eq!(
        state.player(me).unwrap().hand().len(),
        before_hand,
        "regular play -> no bonus draw"
    );
}

#[test]
fn choose_one_effect_allows_selection() {
    // Test: Choose one of two effects - validates DSL parsing
    let reg = registry_from(
        r#"
        [[card]]
        name = "ChooseOneCard"
        type = "Character"
        cost = 3
        strength = 2
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "play"
        do = { choose_one = [{ draw = 2 }, { gain_lore = 2 }] }
        [[card]]
        name = "Friend"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );

    // Just verify the card loads with choose_one effect
    let mut state = started(&reg);
    let me = state.active_player();
    // This test validates that choose_one parses correctly
    let _ = place(&mut state, me, 100, 0, 2, 3, true);
}

#[test]
fn boost_moves_deck_card_under_character() {
    // Test: Boost activated ability moves top deck card under character, facedown
    let reg = registry_from(
        r#"
        [[card]]
        name = "Booster"
        type = "Character"
        cost = 3
        strength = 2
        willpower = 3
        lore = 1
        [[card.activated]]
        cost = { ink = 1 }
        do = { boost = 1 }
        [[card]]
        name = "Friend"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        "#,
    );

    let mut state = started(&reg);
    let me = state.active_player();

    // Play the Booster character
    let booster_id = place(&mut state, me, 100, 0, 2, 3, true);

    // Add ink to pay for Boost (put a card in inkwell)
    let ink_id = CardId::from_raw(999);
    let ink_instance = CardInstance::new(
        ink_id,
        CardDefId::from_raw(1),
        Conditions {
            ready: true,
            damage: 0,
            drying: false,
            facedown: true,
        },
    );
    state
        .player_mut(me)
        .unwrap()
        .inkwell_mut()
        .push(ink_instance);

    // Get initial deck size
    let before_deck = state.player(me).unwrap().deck().len();

    // Activate Boost ability (as a UseAbility since it's an activated ability in the DSL)
    let _ = apply(
        &mut state,
        &reg,
        Input::UseAbility {
            card: booster_id,
            ability: 0,
        },
    );

    // Verify: deck decreased by 1
    assert_eq!(
        state.player(me).unwrap().deck().len(),
        before_deck - 1,
        "deck should have 1 fewer card"
    );

    // Verify: character has 1 card under it, facedown
    let booster_card = state
        .player(me)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == booster_id)
        .unwrap();
    assert_eq!(
        booster_card.under().len(),
        1,
        "character should have 1 card under it"
    );
}
#[test]
fn ready_trigger_fires_on_turn_start() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "ReadyHero"
        type = "Character"
        cost = 3
        strength = 2
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "readies"
        do = { gain_lore = 1 }
        [[card]]
        name = "Vanilla"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();

    let hero = place(&mut state, me, 100, 0, 2, 3, true);
    assert_eq!(state.player(me).unwrap().lore(), 0, "starts with 0 lore");

    // Exert the hero (this also gains 1 lore from questing)
    let _ = apply(&mut state, &reg, Input::Quest { character: hero }).expect("quest");
    assert_eq!(
        state.player(me).unwrap().lore(),
        1,
        "gained 1 lore from questing"
    );

    // End turn (hero is exerted)
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");

    // Start next turn - hero is readied, trigger should fire
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    assert_eq!(
        state.player(me).unwrap().lore(),
        2,
        "ready trigger fired and gained 1 lore (total 2)"
    );
}

/// Damage trigger mimics WATCH THE TEETH (Hydra - Deadly Serpent, set 3):
/// "Whenever this character is dealt damage, deal that much damage to chosen opposing character."
/// This variant uses a fixed amount; `watch_the_teeth_deals_back_the_damage_just_taken`
/// covers the dynamic "that much" trigger-context amount.
#[test]
fn damage_trigger_mimics_watch_the_teeth() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Hydra"
        type = "Character"
        cost = 6
        strength = 6
        willpower = 5
        lore = 2
        [[card.abilities]]
        on = "dealt_damage"
        do = { deal_damage = 3, to = "chosen opposing character" }
        [[card]]
        name = "Opponent"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 3
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let opponent = opponent_of(&state, me);

    let hydra = place(&mut state, me, 100, 0, 6, 5, true);
    let target = place(&mut state, opponent, 200, 0, 1, 3, true);

    // Exert hydra so it can be challenged
    let _ = apply(&mut state, &reg, Input::Quest { character: hydra }).expect("quest");

    // Pass turn so opponent can challenge
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");

    // Opponent challenges hydra for 1 damage
    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: target,
            target: hydra,
        },
    )
    .expect("challenge");

    // Hydra should take 1 damage (target's strength), target should be banished (took 9 damage, has 3 willpower)
    let hydra_card = state
        .player(me)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == hydra)
        .unwrap();
    let target_in_play = state
        .player(opponent)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == target);

    assert_eq!(hydra_card.conditions().damage, 1, "hydra took 1 damage");
    assert!(
        target_in_play.is_none(),
        "target was banished (took 9 damage vs 3 willpower)"
    );
}

/// Aladdin - Street Rat (set 1): "When you play this character, each opponent loses 1 lore."
/// Plays the card for real and verifies the *named* opponent actually loses 1 lore.
#[test]
fn aladdin_street_rat_play_makes_each_opponent_lose_lore() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Aladdin - Street Rat"
        type = "Character"
        cost = 3
        ink = ["Ruby"]
        strength = 2
        willpower = 2
        lore = 1
        [[card.abilities]]
        on = "play"
        do = { lose_lore = 1, who = "each opponent" }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    // Opponent starts with some lore so the loss is observable (clamps at 0).
    state.player_mut(foe).unwrap().add_lore(3);
    let my_lore_before = lore(&state, me);

    // Put Aladdin (def 0) in hand and 3 ready ink to pay his cost.
    let aladdin = CardId::from_raw(300);
    state
        .player_mut(me)
        .unwrap()
        .hand_mut()
        .push(CardInstance::new(
            aladdin,
            CardDefId::from_raw(0),
            Conditions::faceup_idle(),
        ));
    for raw in 301..304 {
        state
            .player_mut(me)
            .unwrap()
            .inkwell_mut()
            .push(CardInstance::new(
                CardId::from_raw(raw),
                CardDefId::from_raw(0),
                Conditions::faceup_idle(),
            ));
    }

    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: aladdin,
            shift_onto: None,
        },
    )
    .expect("play Aladdin - Street Rat");

    assert_eq!(
        lore(&state, foe),
        2,
        "the opponent lost exactly 1 lore (3 -> 2) on play"
    );
    assert_eq!(
        lore(&state, me),
        my_lore_before,
        "playing the character did not change my own lore"
    );
}

/// Aladdin - Heroic Outlaw (set 1): "During your turn, whenever this character banishes
/// another character in a challenge, you gain 2 lore and each opponent loses 2 lore."
/// On MY turn the trigger fires and swings lore both ways.
#[test]
fn aladdin_heroic_outlaw_banish_in_challenge_swings_lore_both_ways() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Aladdin - Heroic Outlaw"
        type = "Character"
        cost = 7
        ink = ["Ruby"]
        strength = 5
        willpower = 5
        lore = 2
        [[card.abilities]]
        on = "banishes_in_challenge"
        during_your_turn = true
        do = [
            { gain_lore = 2 },
            { lose_lore = 2, who = "each opponent" }
        ]
        [[card]]
        name = "Target Dummy"
        type = "Character"
        cost = 1
        ink = ["Ruby"]
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    // Aladdin (def 0) ready for me; weak target (def 1) exerted for the foe so
    // it's a legal challenge and dies (strength 5 >= willpower 1).
    let aladdin = place(&mut state, me, 100, 0, 5, 5, true);
    let target = place(&mut state, foe, 200, 1, 1, 1, false);

    // Opponent starts with lore so their loss is observable; my lore starts at 0.
    state.player_mut(foe).unwrap().add_lore(5);
    assert_eq!(lore(&state, me), 0, "I start at 0 lore");

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: aladdin,
            target,
        },
    )
    .expect("challenge");

    // The trigger only fires because the challenge banished the target.
    assert!(
        state
            .player(foe)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != target),
        "target was banished by the challenge"
    );
    assert_eq!(lore(&state, me), 2, "I gained 2 lore from the trigger");
    assert_eq!(
        lore(&state, foe),
        3,
        "the opponent lost 2 lore from the trigger (5 -> 3)"
    );
}

/// Guard: Aladdin - Heroic Outlaw's trigger must NOT fire on a challenge that
/// fails to banish the target (the text is gated on a banish).
#[test]
fn aladdin_heroic_outlaw_no_lore_when_challenge_does_not_banish() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Aladdin - Heroic Outlaw"
        type = "Character"
        cost = 7
        ink = ["Ruby"]
        strength = 5
        willpower = 5
        lore = 2
        [[card.abilities]]
        on = "banishes_in_challenge"
        do = [
            { gain_lore = 2 },
            { lose_lore = 2, who = "each opponent" }
        ]
        [[card]]
        name = "Tough Wall"
        type = "Character"
        cost = 1
        ink = ["Ruby"]
        strength = 1
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    // Target has willpower 9 > Aladdin's strength 5, so it survives the challenge.
    let aladdin = place(&mut state, me, 100, 0, 5, 5, true);
    let target = place(&mut state, foe, 200, 1, 1, 9, false);
    state.player_mut(foe).unwrap().add_lore(5);

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: aladdin,
            target,
        },
    )
    .expect("challenge");

    assert!(
        state
            .player(foe)
            .unwrap()
            .play()
            .iter()
            .any(|c| c.id() == target),
        "target survived the challenge"
    );
    assert_eq!(lore(&state, me), 0, "no lore gained: nothing was banished");
    assert_eq!(lore(&state, foe), 5, "opponent kept their lore");
}

/// Aladdin - Heroic Outlaw (set 1): the "During your turn" qualifier must gate the
/// trigger. On the OPPONENT's turn, the opponent challenges my exerted Aladdin with a
/// fragile attacker; Aladdin's return damage banishes the attacker — "this character
/// banishes another in a challenge" — but because it is NOT my turn the lore swing
/// must not happen.
#[test]
fn aladdin_heroic_outlaw_trigger_is_gated_to_your_turn() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Aladdin - Heroic Outlaw"
        type = "Character"
        cost = 7
        ink = ["Ruby"]
        strength = 5
        willpower = 5
        lore = 2
        [[card.abilities]]
        on = "banishes_in_challenge"
        during_your_turn = true
        do = [
            { gain_lore = 2 },
            { lose_lore = 2, who = "each opponent" }
        ]
        [[card]]
        name = "Fragile Attacker"
        type = "Character"
        cost = 1
        ink = ["Ruby"]
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    // My Aladdin (def 0) exerted so it's a legal challenge target on the foe's turn.
    let aladdin = place(&mut state, me, 100, 0, 5, 5, false);
    // The foe's fragile attacker (def 1), ready to challenge.
    let attacker = place(&mut state, foe, 200, 1, 1, 1, true);
    state.player_mut(foe).unwrap().add_lore(5);

    // Hand the turn to the opponent, then have them challenge Aladdin.
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    assert_eq!(state.active_player(), foe, "it is now the opponent's turn");

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: attacker,
            target: aladdin,
        },
    )
    .expect("challenge");

    // Aladdin's return damage (5) banishes the 1-willpower attacker...
    assert!(
        state
            .player(foe)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != attacker),
        "the attacker was banished by Aladdin's return damage"
    );
    // ...but the "During your turn" trigger must NOT fire on the opponent's turn.
    assert_eq!(
        lore(&state, me),
        0,
        "my during-your-turn trigger did not fire on the opponent's turn"
    );
    assert_eq!(
        lore(&state, foe),
        5,
        "the opponent's lore is untouched (trigger gated out)"
    );
}

/// Hydra - Deadly Serpent (set 3) WATCH THE TEETH: "Whenever this character is dealt
/// damage, deal **that much** damage to chosen opposing character." Exercises the
/// trigger-context amount — the damage dealt back equals the damage just taken.
#[test]
fn watch_the_teeth_deals_back_the_damage_just_taken() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Hydra - Deadly Serpent"
        type = "Character"
        cost = 6
        ink = ["Ruby"]
        strength = 6
        willpower = 5
        lore = 2
        [[card.abilities]]
        on = "dealt_damage"
        do = { deal_damage = "that much", to = "chosen opposing character" }
        [[card]]
        name = "Wall"
        type = "Character"
        cost = 3
        ink = ["Ruby"]
        strength = 3
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    // My Hydra (def 0) ready; the foe's wall (def 1) exerted so I can challenge it.
    let hydra = place(&mut state, me, 100, 0, 6, 5, true);
    let wall = place(&mut state, foe, 200, 1, 3, 9, false);

    // Challenge: Hydra deals 6 to the wall (survives, 9 WP), the wall deals 3 back
    // to Hydra — which fires WATCH THE TEETH for "that much" (3) damage.
    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: hydra,
            target: wall,
        },
    )
    .expect("challenge");

    // The trigger wants a chosen opposing character; the only one is the wall.
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(wall)),
    )
    .expect("choose opposing character");

    // Hydra kept exactly the 3 damage it took.
    assert_eq!(damage(&state, me, hydra), Some(3), "hydra took 3 damage");
    // Wall took 6 (combat) + 3 (the trigger's "that much") = 9 == willpower -> banished.
    assert!(
        state
            .player(foe)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != wall),
        "wall banished by 6 combat + 3 'that much' trigger damage"
    );
}

/// Yours-scoped quest trigger (set 1/3/9/11): "Whenever one of your characters
/// quests, gain 1 lore." A *different* character of mine questing fires the
/// watcher's trigger.
#[test]
fn yours_quests_trigger_fires_when_another_of_your_characters_quests() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "yours_quests"
        do = { gain_lore = 1 }
        [[card]]
        name = "Quester"
        type = "Character"
        cost = 1
        ink = ["Amber"]
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();

    let _watcher = place(&mut state, me, 100, 0, 1, 3, true);
    let quester = place(&mut state, me, 200, 1, 1, 1, true);

    let _ = apply(&mut state, &reg, Input::Quest { character: quester }).expect("quest");

    // Quester adds its own 1 lore on questing, plus 1 from the watcher's trigger.
    assert_eq!(
        lore(&state, me),
        2,
        "quest lore (1) + yours-quest trigger (1)"
    );
}

/// The watcher itself is "one of your characters", so its own quest also fires it.
#[test]
fn yours_quests_trigger_fires_when_the_watcher_itself_quests() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "yours_quests"
        do = { gain_lore = 1 }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let watcher = place(&mut state, me, 100, 0, 1, 3, true);

    let _ = apply(&mut state, &reg, Input::Quest { character: watcher }).expect("quest");

    // Its own quest adds 1 lore + 1 from its own yours-quest trigger.
    assert_eq!(
        lore(&state, me),
        2,
        "self-quest lore (1) + yours-quest trigger (1)"
    );
}

/// Guard: an opponent's character questing must NOT fire my yours-scoped trigger
/// (it watches *my* characters).
#[test]
fn yours_quests_trigger_does_not_fire_on_opponents_quest() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "yours_quests"
        do = { gain_lore = 1 }
        [[card]]
        name = "Foe Quester"
        type = "Character"
        cost = 1
        ink = ["Amber"]
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    let _watcher = place(&mut state, me, 100, 0, 1, 3, true);
    let foe_quester = place(&mut state, foe, 200, 1, 1, 1, true);

    // Hand the turn to the opponent, then have them quest.
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    let _ = apply(
        &mut state,
        &reg,
        Input::Quest {
            character: foe_quester,
        },
    )
    .expect("quest");

    assert_eq!(
        lore(&state, me),
        0,
        "my yours-quest trigger ignores the opponent's quest"
    );
    assert_eq!(
        lore(&state, foe),
        1,
        "the opponent only got their own quest lore"
    );
}

/// Yours-scoped banish trigger (set 3/7/8/11/12): "Whenever one of your other
/// characters is banished, gain 1 lore." A different character of mine being
/// banished (here in a challenge) fires the watcher.
#[test]
fn yours_banished_trigger_fires_when_another_of_your_characters_is_banished() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "yours_banished"
        do = { gain_lore = 1 }
        [[card]]
        name = "Fragile"
        type = "Character"
        cost = 1
        ink = ["Amber"]
        strength = 1
        willpower = 1
        lore = 1
        [[card]]
        name = "Wall"
        type = "Character"
        cost = 3
        ink = ["Amber"]
        strength = 5
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    let _watcher = place(&mut state, me, 100, 0, 1, 3, true);
    let fragile = place(&mut state, me, 200, 1, 1, 1, true);
    let wall = place(&mut state, foe, 300, 2, 5, 9, false); // exerted, survives

    // My fragile (1 WP) challenges the wall: the wall's 5 return damage banishes it.
    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: fragile,
            target: wall,
        },
    )
    .expect("challenge");

    assert!(
        state
            .player(me)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != fragile),
        "my fragile character was banished"
    );
    assert_eq!(
        lore(&state, me),
        1,
        "the watcher's yours-banished trigger gained 1 lore"
    );
}

/// Guard: an opponent's character being banished must NOT fire my yours-scoped
/// banish trigger (it watches *my* characters).
#[test]
fn yours_banished_trigger_does_not_fire_on_opponents_banish() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "yours_banished"
        do = { gain_lore = 1 }
        [[card]]
        name = "Bruiser"
        type = "Character"
        cost = 3
        ink = ["Amber"]
        strength = 5
        willpower = 5
        lore = 1
        [[card]]
        name = "Foe Fragile"
        type = "Character"
        cost = 1
        ink = ["Amber"]
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    let _watcher = place(&mut state, me, 100, 0, 1, 3, true);
    let bruiser = place(&mut state, me, 200, 1, 5, 5, true);
    let foe_fragile = place(&mut state, foe, 300, 2, 1, 1, false); // exerted

    // My bruiser banishes the opponent's fragile character in a challenge.
    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: bruiser,
            target: foe_fragile,
        },
    )
    .expect("challenge");

    assert!(
        state
            .player(foe)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != foe_fragile),
        "the opponent's character was banished"
    );
    assert_eq!(
        lore(&state, me),
        0,
        "my yours-banished trigger ignores the opponent's character being banished"
    );
}

/// "During the opponent's turn" gate (set 6): a yours-scoped banish trigger gated
/// to the opponent's turn fires when my character is banished on the OPPONENT's
/// turn, but not on my own turn.
#[test]
fn during_opponents_turn_gate_fires_only_on_the_opponents_turn() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Watcher"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "yours_banished"
        during_opponents_turn = true
        do = { gain_lore = 2 }
        [[card]]
        name = "Fragile"
        type = "Character"
        cost = 1
        ink = ["Amber"]
        strength = 1
        willpower = 1
        lore = 1
        [[card]]
        name = "Attacker"
        type = "Character"
        cost = 3
        ink = ["Amber"]
        strength = 5
        willpower = 5
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    let _watcher = place(&mut state, me, 100, 0, 1, 5, true);
    let fragile = place(&mut state, me, 200, 1, 1, 1, false); // exerted, my character
    let attacker = place(&mut state, foe, 300, 2, 5, 5, true);

    // Hand the turn to the opponent; they challenge and banish my Fragile.
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    assert_eq!(state.active_player(), foe, "opponent's turn");
    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: attacker,
            target: fragile,
        },
    )
    .expect("challenge");

    assert!(
        state
            .player(me)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != fragile),
        "my fragile character was banished on the opponent's turn"
    );
    assert_eq!(
        lore(&state, me),
        2,
        "the during-opponents-turn watcher fired (gained 2 lore)"
    );
}

/// Effect-driven damage (not just combat) fires "whenever a character is dealt
/// damage" triggers: a quester deals 2 damage to a chosen opposing character that
/// reacts to being damaged.
#[test]
fn effect_damage_fires_dealt_damage_triggers() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Pinger"
        type = "Character"
        cost = 1
        ink = ["Ruby"]
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { deal_damage = 2, to = "chosen opposing character" }
        [[card]]
        name = "Reactor"
        type = "Character"
        cost = 1
        ink = ["Ruby"]
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "dealt_damage"
        do = { gain_lore = 1 }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let pinger = place(&mut state, me, 100, 0, 1, 5, true);
    let reactor = place(&mut state, foe, 200, 1, 1, 5, false);

    let _ = apply(&mut state, &reg, Input::Quest { character: pinger }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(reactor)),
    )
    .expect("choose target");

    assert_eq!(
        damage(&state, foe, reactor),
        Some(2),
        "reactor took 2 damage"
    );
    assert_eq!(
        lore(&state, foe),
        1,
        "reactor's dealt-damage trigger fired from effect damage"
    );
}

/// "You may" optionality is the `Effect::May` algebra (no flag): an optional quest
/// trigger suspends on a may-decision and resolves only on yes.
#[test]
fn optional_quest_trigger_resolves_on_yes_only() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Bard"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "quest"
        may = true
        do = { gain_lore = 2 }
        "#,
    );

    // Yes branch: quest lore (1) + the optional +2 = 3.
    let mut state = started(&reg);
    let me = state.active_player();
    let bard = place(&mut state, me, 100, 0, 1, 3, true);
    let _ = apply(&mut state, &reg, Input::Quest { character: bard }).expect("quest");
    assert!(state.pending().is_some(), "awaiting the may-decision");
    assert_eq!(lore(&state, me), 1, "only quest lore so far");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::May(true)),
    )
    .expect("yes");
    assert_eq!(lore(&state, me), 3, "resolved the optional +2 lore");

    // No branch: just the quest lore (1).
    let mut state = started(&reg);
    let me = state.active_player();
    let bard = place(&mut state, me, 100, 0, 1, 3, true);
    let _ = apply(&mut state, &reg, Input::Quest { character: bard }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::May(false)),
    )
    .expect("no");
    assert_eq!(lore(&state, me), 1, "declined: no extra lore");
}

/// "Whenever you draw a card" fires for the drawing player's in-play cards on an
/// effect-driven draw (here a quester that draws), once per card drawn.
#[test]
fn effect_draw_fires_when_you_draw_trigger() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Sage"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "draw"
        do = { gain_lore = 1 }
        [[card]]
        name = "Drawer"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { draw = 1 }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let _sage = place(&mut state, me, 100, 0, 1, 3, true);
    let drawer = place(&mut state, me, 101, 1, 1, 3, true);

    let _ = apply(&mut state, &reg, Input::Quest { character: drawer }).expect("quest");
    // Quest lore (1) + the draw trigger from drawing 1 card (1) = 2.
    assert_eq!(
        lore(&state, me),
        2,
        "the draw trigger fired on the effect draw"
    );
}

/// "Whenever you draw a card" also fires on the natural draw step at the start of
/// the player's turn — without double-drawing.
#[test]
fn draw_step_fires_when_you_draw_trigger() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Sage"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "draw"
        do = { gain_lore = 1 }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let _sage = place(&mut state, foe, 100, 0, 1, 3, true);
    let foe_hand_before = hand_len(&state, foe);

    // End my turn; the opponent's turn begins with a draw step.
    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    assert_eq!(state.active_player(), foe);
    assert_eq!(
        hand_len(&state, foe),
        foe_hand_before + 1,
        "drew exactly one card at the draw step (no double-draw)"
    );
    assert_eq!(lore(&state, foe), 1, "the draw-step draw fired the trigger");
}

/// "When this character leaves play" fires on a banish departure (here the
/// opponent banishes my character in a challenge); the owner gains the lore.
#[test]
fn leaves_play_trigger_fires_on_banish() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Martyr"
        type = "Character"
        cost = 1
        ink = ["Amber"]
        strength = 1
        willpower = 1
        lore = 1
        [[card.abilities]]
        on = "leaves_play"
        do = { gain_lore = 1 }
        [[card]]
        name = "Attacker"
        type = "Character"
        cost = 3
        ink = ["Amber"]
        strength = 3
        willpower = 3
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let martyr = place(&mut state, me, 100, 0, 1, 1, false); // my exerted character
    let attacker = place(&mut state, foe, 200, 1, 3, 3, true);

    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: attacker,
            target: martyr,
        },
    )
    .expect("challenge");

    assert!(
        state
            .player(me)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != martyr),
        "martyr was banished"
    );
    assert_eq!(
        lore(&state, me),
        1,
        "the leaves-play trigger fired for the banished character's owner"
    );
}

/// "When this character leaves play" also fires on a non-banish departure — here
/// the character returns itself to hand (a self-move out of play).
#[test]
fn leaves_play_trigger_fires_on_bounce() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Houdini"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { return_to_hand = "self" }
        [[card.abilities]]
        on = "leaves_play"
        do = { gain_lore = 2 }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let houdini = place(&mut state, me, 100, 0, 1, 3, true);

    let _ = apply(&mut state, &reg, Input::Quest { character: houdini }).expect("quest");
    assert!(
        state
            .player(me)
            .unwrap()
            .play()
            .iter()
            .all(|c| c.id() != houdini),
        "houdini returned to hand"
    );
    // Quest lore (1) + leaves-play on the self-bounce (2) = 3.
    assert_eq!(lore(&state, me), 3, "leaves-play fired on the bounce");
}

/// "Exert chosen character. They can't ready at the start of their next turn." —
/// `Effect::OnTarget` applies exert then freeze to a *single* chosen character
/// (one pick, two effects), so a ready victim is exerted now and can't ready.
#[test]
fn exert_and_freeze_apply_to_one_chosen_target() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Frostbite"
        type = "Character"
        cost = 2
        ink = ["Amethyst"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { apply_to = "chosen opposing character", then_to = [ { exert = "self" }, { freeze = "self" } ] }
        [[card]]
        name = "Victim"
        type = "Character"
        cost = 2
        ink = ["Amethyst"]
        strength = 1
        willpower = 3
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let froster = place(&mut state, me, 100, 0, 1, 3, true);
    let victim = place(&mut state, foe, 200, 1, 1, 3, true); // READY

    let _ = apply(&mut state, &reg, Input::Quest { character: froster }).expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(victim)),
    )
    .expect("choose the one target for both effects");

    let ready = state
        .player(foe)
        .unwrap()
        .play()
        .iter()
        .find(|c| c.id() == victim)
        .unwrap()
        .conditions()
        .ready;
    assert!(
        !ready,
        "the single chosen target was exerted (the first step)"
    );
    assert!(
        state.has_restriction(victim, lorcana_engine::Restriction::CantReady),
        "...and frozen (the second step), from one pick"
    );
}

/// Like [`place`] (always ready), but with an explicit `{L}` so quest-lore is
/// observable (the `place` helper always fixes lore at 1).
fn place_ready_with_lore(
    state: &mut GameState,
    owner: PlayerId,
    raw: u32,
    def: u32,
    strength: u32,
    willpower: u32,
    lore: u32,
) -> CardId {
    let id = CardId::from_raw(raw);
    let mut instance = CardInstance::new(id, CardDefId::from_raw(def), Conditions::faceup_idle());
    instance.set_stats(Some(CharacterStats::new(strength, willpower, lore)));
    state.player_mut(owner).unwrap().play_mut().push(instance);
    id
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

/// §4.3.5 — Quest: the active player exerts the questing character (§4.3.5.7) and
/// gains lore equal to that character's {L} (§4.3.5.8). Here a {L} 2 character
/// quests once, so its controller gains exactly 2 lore and the character is left
/// exerted.
#[test]
fn questing_exerts_the_character_and_gains_its_lore() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Pathfinder"
        type = "Character"
        cost = 1
        strength = 2
        willpower = 3
        lore = 2
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let hero = place_ready_with_lore(&mut state, me, 100, 0, 2, 3, 2);
    assert_eq!(lore(&state, me), 0, "starts the turn at 0 lore");
    assert!(is_ready(&state, me, hero), "the character starts ready");

    let _ = apply(&mut state, &reg, Input::Quest { character: hero }).expect("quest");

    assert_eq!(
        lore(&state, me),
        2,
        "gained lore equal to the questing character's {{L}} (§4.3.5.8)"
    );
    assert!(
        !is_ready(&state, me, hero),
        "the questing character is exerted (§4.3.5.7)"
    );
}

/// §4.3.6 (worked Example A) — in a challenge, both the challenging character and
/// the character being challenged deal damage equal to their Strength {S} to the
/// other (§4.3.6.13). Here both have enough Willpower to survive, so each ends up
/// with damage counters equal to the *other's* {S}.
#[test]
fn challenge_deals_damage_both_ways() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Stitch"
        type = "Character"
        cost = 1
        strength = 2
        willpower = 9
        lore = 1
        [[card]]
        name = "Milo"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let stitch = place(&mut state, me, 100, 0, 2, 9, true);
    let milo = place(&mut state, foe, 200, 1, 3, 9, false); // exerted: a legal target

    let _ = apply(
        &mut state,
        &reg,
        Input::Challenge {
            challenger: stitch,
            target: milo,
        },
    )
    .expect("challenge");

    assert_eq!(
        damage(&state, foe, milo),
        Some(2),
        "the challenged character took the challenger's 2 {{S}}"
    );
    assert_eq!(
        damage(&state, me, stitch),
        Some(3),
        "the challenging character took the defender's 3 {{S}}"
    );
}

/// §8.7.4 — when a single event puts two of the active player's triggered
/// abilities into the bag at once, that player chooses which to resolve next;
/// both still fully resolve. Here one quest fires two `on = "quest"` abilities
/// (gain 1 lore and draw 1), surfacing an `OrderTriggers` decision.
#[test]
fn the_active_player_orders_simultaneous_bag_triggers() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Twin Trigger"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { gain_lore = 1 }
        [[card.abilities]]
        on = "quest"
        do = { draw = 1 }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let hero = place(&mut state, me, 100, 0, 1, 5, true);
    let hand_before = hand_len(&state, me);

    let _ = apply(&mut state, &reg, Input::Quest { character: hero }).expect("quest");

    // Two triggers from one quest → the controller chooses the resolution order.
    let Some(lorcana_engine::PendingDecision::OrderTriggers { player, options }) = state.pending()
    else {
        panic!("expected an OrderTriggers decision (§8.7.4)");
    };
    assert_eq!(*player, me, "the active player chooses the order");
    assert_eq!(
        options.len(),
        2,
        "both quest triggers are waiting in the bag"
    );
    let first = options[0];

    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ResolveNext(first)),
    )
    .expect("resolve next");

    // Regardless of the chosen order both triggers resolve fully.
    assert!(state.pending().is_none(), "the bag is empty afterwards");
    assert_eq!(
        lore(&state, me),
        2,
        "quest lore (1) + the gain-lore trigger (1)"
    );
    assert_eq!(
        hand_len(&state, me),
        hand_before + 1,
        "the draw trigger also resolved"
    );
}

/// §1.2.3 ("do as much as you can") — when a multi-part effect has a part that
/// can't be performed, the player still does every other part. A quest ability
/// reads "deal 2 damage to chosen opposing character, then draw a card"; the only
/// opposing character has Ward (§10.15), so it can't be chosen (§1.2.4) and takes
/// no damage — but the draw still happens.
#[test]
fn do_as_much_as_you_can_still_resolves_the_doable_part() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Storm Caller"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = [
            { deal_damage = 2, to = "chosen opposing character" },
            { draw = 1 },
        ]
        [[card]]
        name = "Cogsworth"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 1
        keywords = ["Ward"]
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let caller = place(&mut state, me, 100, 0, 1, 5, true);
    let warded = place(&mut state, foe, 200, 1, 1, 5, false); // Ward: not choosable
    let hand_before = hand_len(&state, me);

    let _ = apply(&mut state, &reg, Input::Quest { character: caller }).expect("quest");

    assert!(
        state.pending().is_none(),
        "no target to choose — Ward isn't choosable, so that part is skipped"
    );
    assert_eq!(
        damage(&state, foe, warded),
        Some(0),
        "the Ward character took no damage (the un-doable part)"
    );
    assert_eq!(
        hand_len(&state, me),
        hand_before + 1,
        "the draw still happened — do as much as you can (§1.2.3)"
    );
}

/// §1.9.1.1 — the game-state check (§1.9.2) ends the game as soon as a player has
/// 20 or more lore: that player wins. Here a {L} 2 character quests from 18 lore,
/// reaching exactly 20 and finishing the game with its controller as the winner.
#[test]
fn reaching_twenty_lore_wins_the_game() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Closer"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 5
        lore = 2
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let closer = place_ready_with_lore(&mut state, me, 100, 0, 1, 5, 2);
    state.player_mut(me).unwrap().add_lore(18);
    assert_eq!(lore(&state, me), 18, "one quest short of winning");

    let _ = apply(&mut state, &reg, Input::Quest { character: closer }).expect("quest");

    assert_eq!(lore(&state, me), 20, "reached 20 lore by questing");
    assert_eq!(
        *state.status(),
        GameStatus::Finished { winners: vec![me] },
        "20 lore ends the game with that player as the winner (§1.9.1.1)"
    );
}

/// §4.2.3.2 — Draw step: "First, the active player draws a card from their deck.
/// If this turn is the first turn of the game, the active player skips this
/// step." The starting player keeps their untouched 7-card opening hand, while
/// the second player draws on turn 2 (their hand grows to 8) — proving only the
/// very first turn of the game skips the draw.
#[test]
fn the_first_turn_of_the_game_skips_the_draw_step() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Vanilla"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);

    assert_eq!(
        hand_len(&state, me),
        7,
        "the starting player kept their 7-card opening hand — turn 1 skips the draw (§4.2.3.2)"
    );
    let foe_hand_before = hand_len(&state, foe);

    let _ = apply(&mut state, &reg, Input::EndTurn).expect("end turn");
    assert_eq!(
        state.active_player(),
        foe,
        "it is now the second player's turn"
    );
    assert_eq!(
        hand_len(&state, foe),
        foe_hand_before + 1,
        "the second player draws on turn 2 — only the first turn of the game skips the draw step"
    );
}

/// §4.3.3 — "Put a card into the inkwell" is a turn action limited to once per
/// turn. The chosen inkable card is placed in the inkwell facedown and ready
/// (§4.3.3.2); a second attempt the same turn is rejected (§4.3.3).
#[test]
fn putting_a_card_into_the_inkwell_is_limited_to_once_per_turn() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Inkable"
        type = "Character"
        cost = 1
        inkwell = true
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();

    // Two inkable cards (def 0) in hand to put into the inkwell.
    let first = CardId::from_raw(300);
    let second = CardId::from_raw(301);
    for id in [first, second] {
        state
            .player_mut(me)
            .unwrap()
            .hand_mut()
            .push(CardInstance::new(
                id,
                CardDefId::from_raw(0),
                Conditions::faceup_idle(),
            ));
    }

    let _ = apply(&mut state, &reg, Input::PutCardInInkwell { card: first }).expect("ink one card");
    assert!(
        in_inkwell(&state, me, first),
        "the chosen card is now in the inkwell"
    );
    let inked = state
        .player(me)
        .unwrap()
        .inkwell()
        .iter()
        .find(|c| c.id() == first)
        .unwrap();
    assert!(
        inked.conditions().facedown && inked.conditions().ready,
        "the inked card is placed facedown and ready (§4.3.3.2)"
    );

    assert!(
        apply(&mut state, &reg, Input::PutCardInInkwell { card: second }).is_err(),
        "a second card can't be inked the same turn (§4.3.3 once-per-turn)"
    );
    assert_eq!(
        state.player(me).unwrap().inkwell().len(),
        1,
        "the rejected second ink left the inkwell unchanged"
    );
}

/// §4.3.6.7 — when declaring a challenge the player "chooses an exerted opposing
/// character." A *ready* opposing character isn't a legal challenge target, so
/// the challenge is rejected.
#[test]
fn only_an_exerted_character_can_be_challenged() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Attacker"
        type = "Character"
        cost = 1
        strength = 3
        willpower = 9
        lore = 1
        [[card]]
        name = "Bystander"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let attacker = place(&mut state, me, 100, 0, 3, 9, true);
    let ready_target = place(&mut state, foe, 200, 1, 1, 9, true); // READY: not a legal target

    assert!(
        apply(
            &mut state,
            &reg,
            Input::Challenge {
                challenger: attacker,
                target: ready_target,
            },
        )
        .is_err(),
        "a ready opposing character can't be chosen as a challenge target (§4.3.6.7)"
    );
}

/// §4.3.6.9 — "the challenging player exerts the challenging character." A
/// character that was ready when declared as the challenger is left exerted once
/// the challenge has occurred.
#[test]
fn the_challenging_character_is_exerted_by_challenging() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Challenger"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        [[card]]
        name = "Target"
        type = "Character"
        cost = 1
        strength = 1
        willpower = 9
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let foe = opponent_of(&state, me);
    let challenger = place(&mut state, me, 100, 0, 1, 9, true);
    let target = place(&mut state, foe, 200, 1, 1, 9, false); // exerted: a legal target
    assert!(
        is_ready(&state, me, challenger),
        "the challenger starts the turn ready"
    );

    let _ = apply(&mut state, &reg, Input::Challenge { challenger, target }).expect("challenge");

    assert!(
        !is_ready(&state, me, challenger),
        "the challenging character is exerted by declaring the challenge (§4.3.6.9)"
    );
}

/// §6.3.3 — Sing a song: instead of paying its ink cost, a song can be played for
/// free by exerting a ready, dry character whose cost is at least the song's cost
/// (§6.3.3.3). The singer is exerted, the song's effect resolves, and the song
/// then goes to its owner's discard.
#[test]
fn a_song_is_sung_for_free_by_exerting_a_singer() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Part of Your World"
        type = "Song"
        classifications = ["Song"]
        cost = 3
        [[card.abilities]]
        on = "play"
        do = { gain_lore = 2 }
        [[card]]
        name = "Sebastian"
        type = "Character"
        cost = 3
        strength = 1
        willpower = 3
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    // The singer (def 1, cost 3) is ready and dry, so it can pay for the song.
    let singer = place(&mut state, me, 100, 1, 1, 3, true);

    // The song (def 0) is in hand.
    let song = CardId::from_raw(300);
    state
        .player_mut(me)
        .unwrap()
        .hand_mut()
        .push(CardInstance::new(
            song,
            CardDefId::from_raw(0),
            Conditions::faceup_idle(),
        ));
    let lore_before = lore(&state, me);

    let _ = apply(
        &mut state,
        &reg,
        Input::Sing {
            song,
            singers: vec![singer],
        },
    )
    .expect("sing the song");

    assert_eq!(
        lore(&state, me),
        lore_before + 2,
        "the song's effect resolved (gain 2 lore)"
    );
    assert!(
        !is_ready(&state, me, singer),
        "the singer was exerted to pay for the song (§6.3.3.3)"
    );
    assert!(
        state.player(me).unwrap().discard().contains(song),
        "the sung song went to its owner's discard"
    );
}

/// "Return a character card from your discard to your hand" (§8.x): a quester with
/// a return-from-discard ability lets the controller pick a matching discarded
/// card, which moves to hand.
#[test]
fn return_a_character_from_discard_to_hand() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Necromancer"
        type = "Character"
        cost = 2
        ink = ["Amethyst"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { return_from_discard = "character card" }
        [[card]]
        name = "Fallen Hero"
        type = "Character"
        cost = 1
        ink = ["Amethyst"]
        strength = 1
        willpower = 1
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let necromancer = place(&mut state, me, 100, 0, 1, 3, true);

    // Seed a character card into my discard.
    let fallen = CardId::from_raw(200);
    let mut inst = CardInstance::new(
        fallen,
        CardDefId::from_raw(1),
        Conditions {
            ready: false,
            damage: 0,
            drying: false,
            facedown: false,
        },
    );
    inst.set_stats(Some(CharacterStats::new(1, 1, 1)));
    state.player_mut(me).unwrap().discard_mut().push(inst);

    let hand_before = hand_len(&state, me);
    let _ = apply(
        &mut state,
        &reg,
        Input::Quest {
            character: necromancer,
        },
    )
    .expect("quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(fallen)),
    )
    .expect("choose the discarded card");

    assert!(
        state
            .player(me)
            .unwrap()
            .hand()
            .iter()
            .any(|c| c.id() == fallen),
        "the discarded character returned to hand"
    );
    assert!(
        !state.player(me).unwrap().discard().contains(fallen),
        "and left the discard"
    );
    assert_eq!(hand_len(&state, me), hand_before + 1);
}

/// "Chosen character gets +2 {L} this turn" (§7.6.1): the buffed character then
/// quests for its boosted lore.
#[test]
fn plus_lore_this_turn_boosts_questing() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Anthem"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        [[card.abilities]]
        on = "quest"
        do = { give_lore = 2, target = "another chosen character" }
        [[card]]
        name = "Hero"
        type = "Character"
        cost = 2
        ink = ["Amber"]
        strength = 1
        willpower = 3
        lore = 1
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    let anthem = place(&mut state, me, 100, 0, 1, 3, true);
    let hero = place(&mut state, me, 101, 1, 1, 3, true);

    // Anthem quests (+1 lore) then buffs Hero's {L} by +2 this turn.
    let _ = apply(&mut state, &reg, Input::Quest { character: anthem }).expect("anthem quest");
    let _ = apply(
        &mut state,
        &reg,
        Input::Decide(lorcana_engine::Decision::ChooseTarget(hero)),
    )
    .expect("choose hero");
    // Hero now quests for 1 base + 2 = 3.
    let _ = apply(&mut state, &reg, Input::Quest { character: hero }).expect("hero quest");

    assert_eq!(lore(&state, me), 1 + 3, "Anthem's 1 + Hero's boosted 3");
}
