//! The reducer: `start` sets a game up, `apply` advances it by one input.

use super::input::{Input, Rejected};
use crate::domain::cards::{CardKind, CardRegistry};
use crate::domain::game::{Conditions, GameEvent, GameState, GameStatus};
use crate::domain::rules::game_state_check;
use crate::domain::types::ids::{CardId, PlayerId};
use crate::domain::types::turn::{Phase, Step};

/// The opening hand size (§3.1.5).
const OPENING_HAND_SIZE: usize = 7;

/// Start a not-yet-started game: pick the starting player from the seeded RNG,
/// deal opening hands, and enter the mulligan phase (§3.1).
///
/// # Errors
///
/// Returns [`Rejected::AlreadyStarted`] if the game is not in the `NotStarted`
/// state.
///
/// # Panics
///
/// Panics if the game has more than `u8::MAX` players.
pub fn start(state: &mut GameState) -> Result<Vec<GameEvent>, Rejected> {
    if !matches!(state.status(), GameStatus::NotStarted) {
        return Err(Rejected::AlreadyStarted);
    }

    let player_count = state.player_count();
    let starting_seat = state.rng_mut().below(player_count);
    let starting = seat(starting_seat);
    state.set_active_player(starting);

    for index in 0..player_count {
        let player = seat(index);
        for _ in 0..OPENING_HAND_SIZE {
            deal_one(state, player);
        }
    }

    state.set_status(GameStatus::AwaitingMulligan(starting));
    Ok(vec![GameEvent::HandsDealt])
}

/// Apply a single input, returning the events it produced. On `Err` the state is
/// left unchanged.
///
/// # Errors
///
/// Returns a [`Rejected`] describing why the input was illegal.
///
/// # Panics
///
/// Panics if the game has more than `u8::MAX` players.
pub fn apply(
    state: &mut GameState,
    registry: &CardRegistry,
    input: Input,
) -> Result<Vec<GameEvent>, Rejected> {
    match input {
        Input::Mulligan { player, put_back } => apply_mulligan(state, player, &put_back),
        Input::PutCardInInkwell { card } => apply_put_in_inkwell(state, registry, card),
        Input::PlayCard { card } => apply_play_card(state, registry, card),
        Input::Quest { character } => apply_quest(state, registry, character),
        Input::EndTurn => apply_end_turn(state),
    }
}

fn apply_mulligan(
    state: &mut GameState,
    player: PlayerId,
    put_back: &[CardId],
) -> Result<Vec<GameEvent>, Rejected> {
    // --- validate (no mutation yet) ---
    let GameStatus::AwaitingMulligan(expected) = *state.status() else {
        return Err(Rejected::NotAwaitingMulligan);
    };
    if player != expected {
        return Err(Rejected::WrongMulliganPlayer);
    }
    let hand = state.player(player).expect("awaited player exists").hand();
    for &card in put_back {
        if !hand.contains(card) {
            return Err(Rejected::MulliganCardNotInHand(card));
        }
    }

    // --- mutate ---
    let returned = u32::try_from(put_back.len()).expect("hand fits in u32");
    {
        let p = state.player_mut(player).expect("awaited player exists");
        for &card in put_back {
            if let Some(instance) = p.hand_mut().take(card) {
                p.deck_mut().insert_bottom(instance);
            }
        }
        while p.hand().len() < OPENING_HAND_SIZE {
            let Some(instance) = p.deck_mut().pop_top() else {
                break;
            };
            p.hand_mut().push(instance);
        }
    }
    if returned >= 1 {
        state.shuffle_deck(player);
    }

    let mut events = vec![GameEvent::MulliganResolved { player, returned }];
    events.extend(advance_after_mulligan(state, player));
    Ok(events)
}

/// Move mulligan to the next player in turn order, or start the first turn.
fn advance_after_mulligan(state: &mut GameState, just_resolved: PlayerId) -> Vec<GameEvent> {
    let player_count = state.player_count();
    let starting = usize::from(state.active_player().index());
    let offset = (usize::from(just_resolved.index()) + player_count - starting) % player_count;

    if offset + 1 >= player_count {
        state.set_status(GameStatus::Playing);
        begin_turn(state, true)
    } else {
        let next = (starting + offset + 1) % player_count;
        state.set_status(GameStatus::AwaitingMulligan(seat(next)));
        Vec::new()
    }
}

fn apply_put_in_inkwell(
    state: &mut GameState,
    registry: &CardRegistry,
    card: CardId,
) -> Result<Vec<GameEvent>, Rejected> {
    // --- validate (no mutation yet) ---
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();
    let definition_id = hand_card_definition(state, active, card)?;
    let definition = registry
        .get(definition_id)
        .ok_or(Rejected::UnknownCard(card))?;
    if !definition.has_inkwell_symbol() {
        return Err(Rejected::NoInkwellSymbol(card));
    }
    if state.inked_this_turn() {
        return Err(Rejected::AlreadyInkedThisTurn);
    }

    // --- mutate ---
    {
        let p = state.player_mut(active).expect("active player exists");
        let mut instance = p.hand_mut().take(card).expect("validated present");
        *instance.conditions_mut() = Conditions::in_inkwell();
        p.inkwell_mut().push(instance);
    }
    state.set_inked_this_turn(true);

    let mut events = vec![GameEvent::CardPutInInkwell {
        player: active,
        card,
    }];
    events.extend(game_state_check(state));
    Ok(events)
}

fn apply_play_card(
    state: &mut GameState,
    registry: &CardRegistry,
    card: CardId,
) -> Result<Vec<GameEvent>, Rejected> {
    // --- validate (no mutation yet) ---
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();
    let definition_id = hand_card_definition(state, active, card)?;
    let definition = registry
        .get(definition_id)
        .ok_or(Rejected::UnknownCard(card))?;
    // Only characters can be played so far (items/locations/actions: later slices).
    if !matches!(definition.kind(), CardKind::Character { .. }) {
        return Err(Rejected::CardTypeNotPlayableYet(card));
    }
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < definition.cost()
    {
        return Err(Rejected::InsufficientInk(card));
    }

    // --- mutate ---
    {
        let p = state.player_mut(active).expect("active player exists");
        p.exert_ink(definition.cost());
        let mut instance = p.hand_mut().take(card).expect("validated present");
        *instance.conditions_mut() = Conditions::entering_play();
        p.play_mut().push(instance);
    }

    let mut events = vec![GameEvent::CardPlayed {
        player: active,
        card,
    }];
    events.extend(game_state_check(state));
    Ok(events)
}

fn apply_quest(
    state: &mut GameState,
    registry: &CardRegistry,
    character: CardId,
) -> Result<Vec<GameEvent>, Rejected> {
    // --- validate (no mutation yet) ---
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();
    let instance = state
        .player(active)
        .expect("active player exists")
        .play()
        .iter()
        .find(|c| c.id() == character)
        .copied()
        .ok_or(Rejected::CharacterNotInPlay(character))?;
    let CardKind::Character { lore, .. } = registry
        .get(instance.definition())
        .ok_or(Rejected::UnknownCard(character))?
        .kind()
    else {
        return Err(Rejected::NotACharacter(character));
    };
    if instance.conditions().drying {
        return Err(Rejected::CharacterStillDrying(character));
    }
    if !instance.conditions().ready {
        return Err(Rejected::CharacterExerted(character));
    }

    // --- mutate ---
    {
        let p = state.player_mut(active).expect("active player exists");
        if let Some(c) = p.play_mut().iter_mut().find(|c| c.id() == character) {
            c.conditions_mut().ready = false;
        }
        p.add_lore(lore);
    }

    let mut events = vec![
        GameEvent::Quested {
            player: active,
            character,
        },
        GameEvent::LoreGained {
            player: active,
            amount: lore,
        },
    ];
    events.extend(game_state_check(state));
    Ok(events)
}

/// Find the definition id of a card in the active player's hand, or reject.
fn hand_card_definition(
    state: &GameState,
    player: PlayerId,
    card: CardId,
) -> Result<crate::domain::types::ids::CardDefId, Rejected> {
    state
        .player(player)
        .expect("active player exists")
        .hand()
        .iter()
        .find(|c| c.id() == card)
        .map(|c| c.definition())
        .ok_or(Rejected::CardNotInHand(card))
}

fn apply_end_turn(state: &mut GameState) -> Result<Vec<GameEvent>, Rejected> {
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();

    let mut events = Vec::new();
    state.set_phase(Phase::End);
    state.set_step(Step::End);
    events.push(GameEvent::StepEntered { step: Step::End });
    // "this turn" effects end here — none yet.
    events.push(GameEvent::TurnEnded { player: active });
    events.extend(game_state_check(state));
    if state.is_finished() {
        return Ok(events);
    }

    let next = next_active_player(state, active);
    state.set_active_player(next);
    state.increment_turn_number();
    events.extend(begin_turn(state, false));
    Ok(events)
}

/// Run a player's Beginning phase (Ready → Set → Draw) and stop in the Main
/// phase, the next point that needs input (§4.2). The very first turn of the
/// game skips the Draw step (§4.2.3.2).
fn begin_turn(state: &mut GameState, first_turn: bool) -> Vec<GameEvent> {
    let active = state.active_player();
    state.set_inked_this_turn(false);

    let mut events = vec![GameEvent::TurnStarted {
        player: active,
        turn: state.turn_number(),
    }];

    // Ready step (§4.2.1).
    state.set_phase(Phase::Beginning);
    state.set_step(Step::Ready);
    events.push(GameEvent::StepEntered { step: Step::Ready });
    ready_all(state, active);
    events.extend(game_state_check(state));
    if state.is_finished() {
        return events;
    }

    // Set step (§4.2.2): dry characters, gain location lore (none yet), resolve
    // start-of-turn triggers (none yet).
    state.set_step(Step::Set);
    events.push(GameEvent::StepEntered { step: Step::Set });
    dry_characters(state, active);
    events.extend(game_state_check(state));
    if state.is_finished() {
        return events;
    }

    // Draw step (§4.2.3).
    state.set_step(Step::Draw);
    events.push(GameEvent::StepEntered { step: Step::Draw });
    if !first_turn {
        events.push(draw(state, active));
    }
    events.extend(game_state_check(state));
    if state.is_finished() {
        return events;
    }

    // Main phase (§4.3).
    state.set_phase(Phase::Main);
    state.set_step(Step::Main);
    events.push(GameEvent::StepEntered { step: Step::Main });
    events.extend(game_state_check(state));
    events
}

/// Deal one card from a player's deck to their hand during setup (does not flag
/// a deck-out and emits no event).
fn deal_one(state: &mut GameState, player: PlayerId) {
    let p = state.player_mut(player).expect("player exists");
    if let Some(instance) = p.deck_mut().pop_top() {
        p.hand_mut().push(instance);
    }
}

/// Draw a card during play, flagging a deck-out if the deck is empty (§4.2.3,
/// §1.9.1.2).
fn draw(state: &mut GameState, player: PlayerId) -> GameEvent {
    let p = state.player_mut(player).expect("player exists");
    if let Some(instance) = p.deck_mut().pop_top() {
        let card = instance.id();
        p.hand_mut().push(instance);
        GameEvent::CardDrawn { player, card }
    } else {
        p.note_drew_from_empty_deck();
        GameEvent::DeckEmptyOnDraw { player }
    }
}

/// Ready all of a player's cards in play and in their inkwell (§4.2.1.1).
fn ready_all(state: &mut GameState, player: PlayerId) {
    let p = state.player_mut(player).expect("player exists");
    for card in p.play_mut().iter_mut() {
        card.conditions_mut().ready = true;
    }
    for card in p.inkwell_mut().iter_mut() {
        card.conditions_mut().ready = true;
    }
}

/// A player's characters in play stop drying (§4.2.2.1).
fn dry_characters(state: &mut GameState, player: PlayerId) {
    let p = state.player_mut(player).expect("player exists");
    for card in p.play_mut().iter_mut() {
        card.conditions_mut().drying = false;
    }
}

/// The next non-eliminated player after `from`, in seat order (§1.10.2.1).
fn next_active_player(state: &GameState, from: PlayerId) -> PlayerId {
    let player_count = state.player_count();
    let mut index = (usize::from(from.index()) + 1) % player_count;
    for _ in 0..player_count {
        let candidate = seat(index);
        if !state
            .player(candidate)
            .is_some_and(super::super::game::PlayerState::is_eliminated)
        {
            return candidate;
        }
        index = (index + 1) % player_count;
    }
    from
}

/// Build a [`PlayerId`] from a seat index.
fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(u8::try_from(index).expect("a game has at most 255 players"))
}
