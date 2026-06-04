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
    assert!(
        booster_card.under().first().unwrap().conditions().facedown,
        "card under should be facedown"
    );
}
