//! Behaviour tests for cards authored from their printed text via the effect DSL
//! (the `card-author` skill). Each card is loaded through `load_toml` exactly as
//! authored in its `cards/<set>/<collector>.toml`, then exercised to pin the
//! rules-correct outcome. One test per authored card.

use lorcana_engine::{
    CardDefId, CardId, CardInstance, CardRegistry, Conditions, GameState, GameStatus, Input,
    PlayerId, apply, load_toml, start,
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

/// Push a ready ink card into `owner`'s inkwell (to fund a play).
fn place_ink(state: &mut GameState, owner: PlayerId, raw: u32) {
    state
        .player_mut(owner)
        .unwrap()
        .inkwell_mut()
        .push(CardInstance::new(
            CardId::from_raw(raw),
            CardDefId::from_raw(raw),
            Conditions {
                ready: true,
                damage: 0,
                drying: false,
                facedown: false,
            },
        ));
}

fn deck_len(state: &GameState, p: PlayerId) -> usize {
    state.player(p).unwrap().deck().len()
}

/// Genie - Wish Fulfilled (set 6 / 53): "WHAT HAPPENS NOW? When you play this
/// character, draw a card." Playing it draws one card (deck shrinks by one beyond
/// the play itself, which doesn't draw). Evasive is a keyword (no DSL).
#[test]
fn genie_wish_fulfilled_draws_on_play() {
    let reg = registry_from(
        r#"
        [[card]]
        name = "Genie - Wish Fulfilled"
        type = "Character"
        cost = 4
        ink = ["Amethyst"]
        image = "x"
        max_copies = 4
        strength = 2
        willpower = 4
        lore = 2
        classifications = ["Storyborn", "Ally"]
        keywords = ["Evasive"]
        text = "Evasive\nWHAT HAPPENS NOW? When you play this character, draw a card."
        [[card.abilities]]
        on = "play"
        do = { draw = 1 }
        "#,
    );
    let mut state = started(&reg);
    let me = state.active_player();
    for ink in 900..904 {
        place_ink(&mut state, me, ink);
    }
    let genie = CardId::from_raw(800);
    state
        .player_mut(me)
        .unwrap()
        .hand_mut()
        .push(CardInstance::new(
            genie,
            CardDefId::from_raw(0),
            Conditions::faceup_idle(),
        ));

    let before = deck_len(&state, me);
    let _ = apply(
        &mut state,
        &reg,
        Input::PlayCard {
            card: genie,
            shift_onto: None,
        },
    )
    .expect("play genie");

    assert!(
        state.player(me).unwrap().play().contains(genie),
        "Genie entered play"
    );
    assert_eq!(
        deck_len(&state, me),
        before - 1,
        "the on-play ability drew exactly one card",
    );
}
