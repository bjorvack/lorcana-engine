//! The reducer: `start` sets a game up, `apply` advances it by one input.

use super::input::{Decision, Input, Rejected};
use crate::domain::cards::{
    CardDefinition, CardKind, CardRegistry, GameRuleStatic, Keyword, ShiftAbility, ShiftCost,
    ShiftKind, StaticAbility, StaticTarget,
};
use crate::domain::effects::{
    CardCategory, CharacterFilter, Effect, Target, TargetSide, TriggerCondition,
};
use crate::domain::game::{
    CardInstance, CharacterStats, Conditions, GameEvent, GameState, GameStatus, LocationStats,
    ModifierDuration, ModifierTarget, PendingDecision, RuleModifier, Stat, StatModifier, TriggerId,
};
use crate::domain::rules::game_state_check;
use crate::domain::types::card::Classification;
use crate::domain::types::ids::{CardDefId, CardId, PlayerId};
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
    // A pending decision must be answered before any other input (§8.7).
    if let Input::Decide(decision) = input {
        return apply_decision(state, registry, decision);
    }
    if state.is_awaiting_decision() {
        return Err(Rejected::AwaitingDecision);
    }

    match input {
        Input::Mulligan { player, put_back } => apply_mulligan(state, player, &put_back),
        Input::PutCardInInkwell { card } => apply_put_in_inkwell(state, registry, card),
        Input::PlayCard { card, shift_onto } => apply_play_card(state, registry, card, shift_onto),
        Input::Quest { character } => apply_quest(state, registry, character),
        Input::Boost { card } => apply_boost(state, registry, card),
        Input::MoveCharacter {
            character,
            location,
        } => apply_move(state, character, location),
        Input::Sing { song, singers } => apply_sing(state, registry, song, &singers),
        Input::Challenge { challenger, target } => {
            apply_challenge(state, registry, challenger, target)
        }
        Input::UseAbility { card, ability } => apply_use_ability(state, registry, card, ability),
        Input::EndTurn => apply_end_turn(state, registry),
        Input::Decide(_) => unreachable!("handled above"),
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

/// Play a character from hand, either normally (paying ink) or via **Shift**
/// (`shift_onto = Some(target)`), which is an alternate cost that puts the card
/// on top of a valid in-play character, forming a stack (§10.10).
fn apply_play_card(
    state: &mut GameState,
    registry: &CardRegistry,
    card: CardId,
    shift_onto: Option<CardId>,
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
    // Actions resolve their effect and go to discard (never in play, §6.3).
    if matches!(definition.kind(), CardKind::Action) {
        if shift_onto.is_some() {
            return Err(Rejected::CannotShift(card)); // actions can't Shift
        }
        let ink_cost = definition.cost();
        let effects = definition.action_effects().to_vec();
        if state
            .player(active)
            .expect("active player exists")
            .ready_ink()
            < ink_cost
        {
            return Err(Rejected::InsufficientInk(card));
        }
        state
            .player_mut(active)
            .expect("active player exists")
            .exert_ink(ink_cost);
        return Ok(resolve_action_play(
            state,
            registry,
            active,
            card,
            definition_id,
            effects,
        ));
    }
    let statics = definition.static_abilities().to_vec();
    let rule_statics = definition.rule_statics().to_vec();

    // --- pay the cost and place the card (a permanent: character or location) ---
    place_permanent(state, registry, active, card, shift_onto, definition)?;
    // Static abilities apply as the card enters play (§7.6.2).
    apply_enter_statics(state, active, card, &statics);
    apply_enter_rule_statics(state, active, card, &rule_statics);

    let mut events = vec![GameEvent::CardPlayed {
        player: active,
        card,
    }];
    events.extend(game_state_check(state));
    if state.is_finished() {
        return Ok(events);
    }
    // Bodyguard may enter play exerted (§10.3.2): ask the controller before its
    // enters-play triggers resolve. The choice is answered with `Decide`, which
    // then runs the enters-play triggers (`enqueue_enter_play_triggers`).
    if character_has_keyword(state, registry, card, &Keyword::Bodyguard) {
        state.set_pending(PendingDecision::EnterPlayExerted {
            player: active,
            card,
        });
        return Ok(events);
    }
    enqueue_enter_play_triggers(state, registry, active, card, definition_id);
    events.extend(resolve_bag(state, registry));
    Ok(events)
}

/// Enqueue a just-entered card's "when you play this" and the cross-scope
/// "whenever you play a [category]" triggers (§4.3.4.8). Shift *is* playing the
/// card (§10.10.1), so these fire whether or not Shift was used.
///
/// TODO(shift-conditional triggers — Slice 8): 23 cards gate a play trigger on
/// "if you used Shift to play them" (Mulan, Pegasus, Mickey, Basil; watchers
/// Bucky, Honey Lemon, Chem Purse). Thread a was-shifted play-context flag so
/// conditional effects (Slice 8 DSL) can gate on it. See "Slice 6c"/"Slice 8" in
/// `docs/planning/IMPLEMENTATION_PLAN.md`.
fn enqueue_enter_play_triggers(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    card: CardId,
    definition_id: CardDefId,
) {
    enqueue_self_triggers(
        state,
        registry,
        controller,
        card,
        &TriggerCondition::WhenYouPlayThis,
    );
    enqueue_play_a_card_triggers(state, registry, controller, card, definition_id);
}

/// Place a permanent (character or location) into play, paying its cost. Routes
/// to the right placement by card kind; items are not playable yet, and actions
/// are handled before this is called.
fn place_permanent(
    state: &mut GameState,
    registry: &CardRegistry,
    active: PlayerId,
    card: CardId,
    shift_onto: Option<CardId>,
    definition: &CardDefinition,
) -> Result<(), Rejected> {
    let ink_cost = definition.cost();
    match definition.kind() {
        CardKind::Character {
            strength,
            willpower,
            lore,
        } => {
            let character_stats = CharacterStats::new(strength, willpower, lore);
            let classifications = definition.classifications().to_vec();
            if let Some(target) = shift_onto {
                let ability = definition
                    .shift()
                    .cloned()
                    .ok_or(Rejected::CannotShift(card))?;
                let shift_names = definition.names().to_vec();
                place_via_shift(
                    state,
                    registry,
                    active,
                    card,
                    target,
                    &ability,
                    &shift_names,
                    character_stats,
                    classifications,
                )
            } else {
                place_normally(
                    state,
                    active,
                    card,
                    ink_cost,
                    character_stats,
                    classifications,
                )
            }
        }
        CardKind::Location {
            move_cost,
            willpower,
            lore,
        } => {
            if shift_onto.is_some() {
                return Err(Rejected::CannotShift(card)); // locations can't Shift
            }
            place_location(
                state,
                active,
                card,
                ink_cost,
                LocationStats::new(willpower, lore, move_cost),
            )
        }
        CardKind::Item => place_item(state, active, card, ink_cost),
        CardKind::Action => unreachable!("actions are handled before placement"),
    }
}

/// Pay an item's ink cost and put it into play (§6.4): faceup and in play, with no
/// strength/willpower/drying. Its abilities can be used the turn it's played
/// (§6.4.3) — `apply_use_ability` works on any in-play card.
fn place_item(
    state: &mut GameState,
    active: PlayerId,
    card: CardId,
    ink_cost: u32,
) -> Result<(), Rejected> {
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < ink_cost
    {
        return Err(Rejected::InsufficientInk(card));
    }
    let p = state.player_mut(active).expect("active player exists");
    p.exert_ink(ink_cost);
    let mut instance = p.hand_mut().take(card).expect("validated present");
    *instance.conditions_mut() = Conditions::faceup_idle();
    p.play_mut().push(instance);
    Ok(())
}

/// Pay a character's ink cost and put it into play drying (the normal play path).
fn place_normally(
    state: &mut GameState,
    active: PlayerId,
    card: CardId,
    ink_cost: u32,
    character_stats: CharacterStats,
    classifications: Vec<Classification>,
) -> Result<(), Rejected> {
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < ink_cost
    {
        return Err(Rejected::InsufficientInk(card));
    }
    let p = state.player_mut(active).expect("active player exists");
    p.exert_ink(ink_cost);
    let mut instance = p.hand_mut().take(card).expect("validated present");
    *instance.conditions_mut() = Conditions::entering_play();
    instance.set_stats(Some(character_stats));
    instance.set_classifications(classifications);
    p.play_mut().push(instance);
    Ok(())
}

/// Pay a location's ink cost and put it into play (§6.5): faceup, undamaged, in
/// play — locations have no ready/exerted/drying state (§5.1.13.3).
fn place_location(
    state: &mut GameState,
    active: PlayerId,
    card: CardId,
    ink_cost: u32,
    location: LocationStats,
) -> Result<(), Rejected> {
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < ink_cost
    {
        return Err(Rejected::InsufficientInk(card));
    }
    let p = state.player_mut(active).expect("active player exists");
    p.exert_ink(ink_cost);
    let mut instance = p.hand_mut().take(card).expect("validated present");
    *instance.conditions_mut() = Conditions::faceup_idle();
    instance.set_location_stats(Some(location));
    p.play_mut().push(instance);
    Ok(())
}

/// Play `card` via Shift (§10.10): validate the target/cost, then put the card on
/// top of `target`, inheriting its exerted/dry/drying state (§10.10.3–5) and
/// damage (§10.10.7) and forming a stack.
#[allow(clippy::too_many_arguments)]
fn place_via_shift(
    state: &mut GameState,
    registry: &CardRegistry,
    active: PlayerId,
    card: CardId,
    target: CardId,
    ability: &ShiftAbility,
    shift_names: &[String],
    character_stats: CharacterStats,
    classifications: Vec<Classification>,
) -> Result<(), Rejected> {
    let target_instance =
        find_in_play(state, active, target).map_err(|_| Rejected::InvalidShiftTarget(target))?;
    if !target_instance.is_character() {
        return Err(Rejected::InvalidShiftTarget(target));
    }
    let onto_ok = match &ability.kind {
        ShiftKind::Any => true,
        ShiftKind::SameName => registry
            .get(target_instance.definition())
            .is_some_and(|td| td.names().iter().any(|n| shift_names.contains(n))),
        ShiftKind::Classification(class) => registry
            .get(target_instance.definition())
            .is_some_and(|td| td.has_classification(class)),
    };
    if !onto_ok {
        return Err(Rejected::InvalidShiftTarget(target));
    }
    let ShiftCost::Ink(ink) = ability.cost;
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < ink
    {
        return Err(Rejected::InsufficientInk(card));
    }
    let inherited = Conditions {
        ready: target_instance.conditions().ready,
        damage: target_instance.conditions().damage,
        drying: target_instance.conditions().drying,
        facedown: false,
    };
    {
        let p = state.player_mut(active).expect("active player exists");
        p.exert_ink(ink);
        let underlying = p.play_mut().take(target).expect("validated present");
        let mut top = p.hand_mut().take(card).expect("validated present");
        *top.conditions_mut() = inherited;
        top.set_stats(Some(character_stats));
        top.set_classifications(classifications);
        top.stack_onto(underlying);
        p.play_mut().push(top);
    }
    // The underlying character is now under the top and left play, so its
    // continuous modifiers end (§7.6.4).
    // TODO(§10.10.6 — Slice 8): the shifted character should *keep* effects that
    // applied to the underlying character when it entered; `Card`-scoped modifiers
    // don't transfer to the new top yet.
    state.remove_modifiers_from_source(target);
    Ok(())
}

/// Move one of the active player's characters to one of their locations (§4.3.7):
/// pay the location's move cost (read from the location's denormalized stats),
/// then record the character as being there.
fn apply_move(
    state: &mut GameState,
    character: CardId,
    location: CardId,
) -> Result<Vec<GameEvent>, Rejected> {
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();
    let mover = find_in_play(state, active, character)?;
    if !mover.is_character() {
        return Err(Rejected::NotACharacter(character));
    }
    // Only your characters may move, and only to your locations (§4.3.7.1).
    let destination =
        find_in_play(state, active, location).map_err(|_| Rejected::NotALocation(location))?;
    let move_cost = destination
        .location_stats()
        .ok_or(Rejected::NotALocation(location))?
        .move_cost;
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < move_cost
    {
        return Err(Rejected::InsufficientInk(character));
    }
    let p = state.player_mut(active).expect("active player exists");
    p.exert_ink(move_cost);
    if let Some(c) = p.play_mut().iter_mut().find(|c| c.id() == character) {
        c.set_at_location(Some(location));
    }
    // TODO(move triggers — Slice 8 / trigger taxonomy): effects that happen "as a
    // result of moving" (and "while here") go to the bag here (§4.3.7.5).
    Ok(vec![GameEvent::Moved {
        player: active,
        character,
        location,
    }])
}

/// Sing a song (§6.3.3): pay the alternate cost by exerting eligible singers, then
/// resolve it like any action. A single singer must have a (Singer-adjusted) cost
/// ≥ the song's cost; several singers use the song's Sing Together value (§10.12).
fn apply_sing(
    state: &mut GameState,
    registry: &CardRegistry,
    song: CardId,
    singers: &[CardId],
) -> Result<Vec<GameEvent>, Rejected> {
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();
    let definition_id = hand_card_definition(state, active, song)?;
    let definition = registry
        .get(definition_id)
        .ok_or(Rejected::UnknownCard(song))?;
    if !definition.is_song() {
        return Err(Rejected::NotASong(song));
    }
    let song_cost = definition.cost();
    let sing_together = definition.sing_together();
    let effects = definition.action_effects().to_vec();
    if singers.is_empty() {
        return Err(Rejected::CannotSing(song));
    }

    // Each singer must be the active player's dry, ready character (§6.3.3.3); its
    // contribution is its cost raised to its Singer value if it has Singer (§10.11).
    let mut total = 0u32;
    for &singer in singers {
        let instance =
            find_in_play(state, active, singer).map_err(|_| Rejected::InvalidSinger(singer))?;
        if !instance.is_character() || !instance.conditions().ready || instance.conditions().drying
        {
            return Err(Rejected::InvalidSinger(singer));
        }
        let def = registry
            .get(instance.definition())
            .ok_or(Rejected::InvalidSinger(singer))?;
        total += def.cost().max(def.singer().unwrap_or(0));
    }
    // Enough singing value? One singer pays a song of its cost or less; several
    // require the song's Sing Together value (§10.12).
    let enough = if singers.len() == 1 {
        total >= song_cost
    } else {
        sing_together.is_some_and(|n| total >= n)
    };
    if !enough {
        return Err(Rejected::CannotSing(song));
    }

    // Pay by exerting the singers, then resolve the song.
    for &singer in singers {
        if let Some(p) = state.player_mut(active)
            && let Some(c) = p.play_mut().iter_mut().find(|c| c.id() == singer)
        {
            c.conditions_mut().ready = false;
        }
    }
    Ok(resolve_action_play(
        state,
        registry,
        active,
        song,
        definition_id,
        effects,
    ))
}

/// Resolve a just-paid-for action/song: move it from hand to discard (it's never
/// in play, §6.3.1), resolve its effects **directly** (not via the bag, §6.3.1.2),
/// then place any effects triggered by the play into the bag (§6.3.4). The cost
/// (ink for a normal play, exerted singers for a song) is paid by the caller.
fn resolve_action_play(
    state: &mut GameState,
    registry: &CardRegistry,
    active: PlayerId,
    card: CardId,
    played_def: CardDefId,
    effects: Vec<Effect>,
) -> Vec<GameEvent> {
    if let Some(p) = state.player_mut(active)
        && let Some(instance) = p.hand_mut().take(card)
    {
        p.discard_mut().push(instance);
    }
    let mut events = vec![GameEvent::CardPlayed {
        player: active,
        card,
    }];
    // Play-a-card watchers go to the bag now (§4.3.4.8) so they aren't lost if the
    // action's own effect suspends to choose a target; they resolve after it.
    enqueue_play_a_card_triggers(state, registry, active, card, played_def);
    resolve_effects(state, registry, active, card, effects, &mut events);
    events.extend(game_state_check_with_triggers(state, registry));
    if !state.is_awaiting_decision() && !state.is_finished() {
        events.extend(resolve_bag(state, registry));
    }
    events
}

/// Use a character's Boost ability (§10.4): pay its ink cost to put the top deck
/// card facedown under it, once per turn. The under-pile is the same stack model
/// Shift uses, so the Boost card dissolves out with the stack on leave-play.
fn apply_boost(
    state: &mut GameState,
    registry: &CardRegistry,
    card: CardId,
) -> Result<Vec<GameEvent>, Rejected> {
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();
    let instance = find_in_play(state, active, card)?;
    if !instance.is_character() {
        return Err(Rejected::NotACharacter(card));
    }
    let cost = registry
        .get(instance.definition())
        .and_then(CardDefinition::boost)
        .ok_or(Rejected::CannotBoost(card))?;
    if state.has_boosted_this_turn(card) {
        return Err(Rejected::AlreadyBoosted(card));
    }
    let player = state.player(active).expect("active player exists");
    if player.ready_ink() < cost {
        return Err(Rejected::InsufficientInk(card));
    }
    if player.deck().iter().next().is_none() {
        return Err(Rejected::DeckEmpty);
    }

    {
        let p = state.player_mut(active).expect("active player exists");
        p.exert_ink(cost);
        // The top deck card is already facedown (deck conditions); put it under
        // the character without revealing it (§10.4.1, §10.4.3).
        let deck_card = p.deck_mut().pop_top().expect("deck checked non-empty");
        if let Some(target) = p.play_mut().iter_mut().find(|c| c.id() == card) {
            target.push_under(deck_card);
        }
    }
    state.mark_boosted_this_turn(card);
    let mut events = vec![GameEvent::Boosted {
        player: active,
        card,
    }];
    // "Whenever a card is put under this character" triggers go to the bag (§10.4).
    enqueue_self_triggers(
        state,
        registry,
        active,
        card,
        &TriggerCondition::WhenCardPutUnder,
    );
    events.extend(resolve_bag(state, registry));
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
    let instance = find_in_play(state, active, character)?;
    if !instance.is_character() {
        return Err(Rejected::NotACharacter(character));
    }
    // Reckless characters can't quest (§10.7.2).
    if character_has_keyword(state, registry, character, &Keyword::Reckless) {
        return Err(Rejected::RecklessCannotQuest(character));
    }
    // Questing requires a dry, ready character (§4.3.5.5).
    // TODO(effects — Slice 8): effects can also forbid a specific character from
    // questing (e.g. Cobra Bubbles "...must quest during their next turn" forces
    // it; others may prevent it).
    if instance.conditions().drying {
        return Err(Rejected::CharacterStillDrying(character));
    }
    if !instance.conditions().ready {
        return Err(Rejected::CharacterExerted(character));
    }
    // Current Lore includes continuous modifiers, clamped at 0 (§7.8.3).
    let lore = state
        .current_character_stats(character)
        .map_or(0, |s| s.lore);

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
    if !state.is_finished() {
        // "Whenever this character quests" triggers go to the bag (§4.3.5.9).
        enqueue_self_triggers(
            state,
            registry,
            active,
            character,
            &TriggerCondition::WhenThisQuests,
        );
        enqueue_support_trigger(state, registry, active, character);
        events.extend(resolve_bag(state, registry));
    }
    Ok(events)
}

/// Support (§10.13): on quest, "you may add this character's `{S}` to another
/// chosen character's `{S}` this turn." Enqueued as an optional bag trigger
/// carrying the source's **current** `{S}` (so modifiers count), targeting
/// another chosen character.
fn enqueue_support_trigger(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    character: CardId,
) {
    if !character_has_keyword(state, registry, character, &Keyword::Support) {
        return;
    }
    let strength = state
        .current_character_stats(character)
        .map_or(0, |s| s.strength);
    if strength == 0 {
        return; // adding 0 {S} is a no-op
    }
    let _ = state.enqueue_trigger(
        controller,
        character,
        true, // "you may"
        Effect::GiveStrengthThisTurn {
            target: Target::ChosenCharacter {
                filter: CharacterFilter::any(TargetSide::Any),
                another: true,
            },
            amount: i32::try_from(strength).unwrap_or(i32::MAX),
        },
    );
}

/// Resolve a challenge (§4.3.6): the active player's dry, ready character
/// challenges an exerted opposing character; both deal damage equal to their
/// Strength simultaneously. The game-state check then banishes any character
/// whose damage has reached its Willpower (§1.9.1.3).
///
/// Slice 3 implements the **vanilla** rules. Challenge legality and resolution
/// are heavy hook points for card text; the base checks here are written so the
/// following can be layered in later (Golden Rules §1.2.1/§1.2.2), each linked
/// to the slice that delivers the machinery:
///
/// Challenger eligibility (now: dry + ready + character):
///   - Rush lets a drying character challenge (§10.9) — Slice 6.
///   - Effects can forbid a character from challenging, e.g. Cobra Bubbles –
///     Dedicated Official "...can't challenge..." — Slice 4 (triggers) + Slice 8
///     (durations).
///
/// Target eligibility (now: opposing in-play character + exerted):
///   - "can challenge ready characters" effects drop the exerted requirement,
///     e.g. Arthur – King Victorious / Cinderella – Stouthearted (and the §1.2.1
///     example) — Slice 5/8.
///   - Evasive: only Evasive may challenge it (§10.6); Alert ignores that
///     (§10.2); Bodyguard must be chosen if able (§10.3) — Slice 6.
///
/// Resolution hooks:
///   - "Whenever this character challenges / is challenged" triggers go to the
///     bag (done — enqueued below). "Banishes another in a challenge" and "when
///     this is banished" ride the banishment path (`game_state_check`).
///   - Resist damage reduction is applied (Slice 6a); "takes no damage from the
///     challenge" and other damage replacement is Slice 8.
fn apply_challenge(
    state: &mut GameState,
    registry: &CardRegistry,
    challenger: CardId,
    target: CardId,
) -> Result<Vec<GameEvent>, Rejected> {
    // --- validate (no mutation yet) ---
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();

    // All challenge legality lives in one authority (§4.3.6 + keyword and effect
    // interactions): see `can_challenge`.
    can_challenge(state, registry, active, challenger, target)?;

    // Legality passed; derive what's needed to resolve the challenge.
    let target_owner =
        opposing_owner_of(state, active, target).expect("legality validated the target owner");
    let challenger_def_id = find_in_play(state, active, challenger)
        .expect("legality validated the challenger")
        .definition();
    let target_def_id = find_in_play(state, target_owner, target)
        .expect("legality validated the target")
        .definition();

    // Current Strength includes continuous modifiers, clamped at 0 (§4.3.6.14,
    // §7.8.2); the challenger also gets Challenger +N while challenging (§10.5).
    let challenger_bonus = registry
        .get(challenger_def_id)
        .map_or(0, CardDefinition::challenger_bonus);
    let challenger_strength = state
        .current_character_stats(challenger)
        .map_or(0, |s| s.strength)
        + challenger_bonus;
    let target_strength = state
        .current_character_stats(target)
        .map_or(0, |s| s.strength);

    // Resist +N reduces the damage each takes (§10.8); applied inline (the general
    // damage-replacement framework is Slice 8).
    let challenger_resist = registry
        .get(challenger_def_id)
        .map_or(0, CardDefinition::resist);
    let target_resist = registry
        .get(target_def_id)
        .map_or(0, CardDefinition::resist);
    let damage_to_target = challenger_strength.saturating_sub(target_resist);
    let damage_to_challenger = target_strength.saturating_sub(challenger_resist);

    // --- mutate ---
    // Exert the challenger (§4.3.6.9), then both deal damage simultaneously
    // (§4.3.6.13).
    if let Some(c) = state
        .player_mut(active)
        .expect("active player exists")
        .play_mut()
        .iter_mut()
        .find(|c| c.id() == challenger)
    {
        c.conditions_mut().ready = false;
    }
    add_damage(state, target_owner, target, damage_to_target);
    add_damage(state, active, challenger, damage_to_challenger);

    let mut events = vec![GameEvent::Challenged {
        player: active,
        challenger,
        target,
    }];
    // "Whenever this character challenges / is challenged" triggers go to the bag
    // (§4.3.6); enqueued before the game-state check so a challenger/target that is
    // about to be banished still triggers (the bag captures the effect).
    enqueue_self_triggers(
        state,
        registry,
        active,
        challenger,
        &TriggerCondition::WhenThisChallenges,
    );
    enqueue_self_triggers(
        state,
        registry,
        target_owner,
        target,
        &TriggerCondition::WhenChallenged,
    );
    let check_events = game_state_check(state);
    let banished_in_check = |id: CardId| {
        check_events
            .iter()
            .any(|e| matches!(e, GameEvent::Banished { card, .. } if *card == id))
    };
    // "When this is banished" / "...in a challenge" for each card the challenge
    // banished (the cards are now in the discard).
    enqueue_banish_triggers(state, registry, &check_events, true);
    // "Whenever this character banishes another in a challenge" for each side that
    // banished the other (read from play or discard, since the banisher may itself
    // have been banished simultaneously).
    if banished_in_check(target)
        && let Some(def) = def_in_play_or_discard(state, active, challenger)
    {
        enqueue_triggers_for_def(
            state,
            registry,
            active,
            challenger,
            def,
            &TriggerCondition::WhenBanishesInChallenge,
        );
    }
    if banished_in_check(challenger)
        && let Some(def) = def_in_play_or_discard(state, target_owner, target)
    {
        enqueue_triggers_for_def(
            state,
            registry,
            target_owner,
            target,
            def,
            &TriggerCondition::WhenBanishesInChallenge,
        );
    }
    events.extend(check_events);
    if !state.is_finished() {
        events.extend(resolve_bag(state, registry));
    }
    Ok(events)
}

/// A card's definition id whether it is in play or in `owner`'s discard (e.g. a
/// card that may have just been banished).
fn def_in_play_or_discard(state: &GameState, owner: PlayerId, card: CardId) -> Option<CardDefId> {
    state
        .instance_in_play(card)
        .map(CardInstance::definition)
        .or_else(|| {
            state
                .player(owner)?
                .discard()
                .iter()
                .find(|c| c.id() == card)
                .map(CardInstance::definition)
        })
}

/// Add damage counters to an in-play card (§4.3.6.16).
fn add_damage(state: &mut GameState, owner: PlayerId, card: CardId, amount: u32) {
    if let Some(c) = state
        .player_mut(owner)
        .expect("owner exists")
        .play_mut()
        .iter_mut()
        .find(|c| c.id() == card)
    {
        c.conditions_mut().damage += amount;
    }
}

/// Find an instance in a specific player's play area, or reject.
fn find_in_play(
    state: &GameState,
    owner: PlayerId,
    card: CardId,
) -> Result<CardInstance, Rejected> {
    state
        .player(owner)
        .and_then(|p| p.play().iter().find(|c| c.id() == card).cloned())
        .ok_or(Rejected::CharacterNotInPlay(card))
}

/// Whether an in-play card currently has a keyword.
///
/// Reads the **printed** keywords. TODO(effect-granted keywords — Slice 8): cards
/// can *grant* keywords ("gains Alert and Challenger +2" — But I'm Much Faster /
/// Inkrunner; Cri-Kee's Alert), so once such effects exist they must be OR'd in
/// here, so every keyword check (and the whole `can_challenge` authority) sees
/// granted keywords too.
fn character_has_keyword(
    state: &GameState,
    registry: &CardRegistry,
    card: CardId,
    keyword: &Keyword,
) -> bool {
    state
        .instance_in_play(card)
        .and_then(|i| registry.get(i.definition()))
        .is_some_and(|d| d.has_keyword(keyword))
}

/// Target-side challenge legality **excluding** the Bodyguard must-choose rule:
/// the target must be an opposing in-play character, exerted, and not blocked by
/// Evasive (§4.3.6.7, §10.6/§10.2). Split out so the Bodyguard rule can test
/// candidate Bodyguards without recursing.
///
/// TODO(effect challenge-legality — Slice 8): plug in here —
///   - "can't be challenged" target restrictions (Tiana's Palace "while here",
///     The Wall, Panic) → reject;
///   - the challenger's "can challenge ready characters" permission (Pick a
///     Fight) → skip the exerted requirement.
fn target_legal_basic(
    state: &GameState,
    registry: &CardRegistry,
    active: PlayerId,
    challenger: CardId,
    target: CardId,
) -> Result<(), Rejected> {
    let owner =
        opposing_owner_of(state, active, target).ok_or(Rejected::TargetNotInPlay(target))?;
    let instance = find_in_play(state, owner, target)?;
    // A location can be challenged at any time — never ready/exerted, and Evasive
    // doesn't apply (§4.3.6.19–22).
    if instance.is_location() {
        return Ok(());
    }
    if !instance.is_character() {
        return Err(Rejected::TargetNotACharacter(target));
    }
    if instance.conditions().ready {
        return Err(Rejected::TargetNotExerted(target));
    }
    if character_has_keyword(state, registry, target, &Keyword::Evasive)
        && !character_has_keyword(state, registry, challenger, &Keyword::Evasive)
        && !character_has_keyword(state, registry, challenger, &Keyword::Alert)
    {
        return Err(Rejected::TargetEvasive(target));
    }
    Ok(())
}

/// The single authority for whether `challenger` may legally challenge `target`
/// (§4.3.6 plus keyword and effect interactions). Used by `apply_challenge`, the
/// Bodyguard "if able" rule, and Reckless's "must challenge if able".
///
/// TODO(effect challenge-legality — Slice 8): the challenger side must also honor
/// "can't challenge" effects (Frying Pan, Cobra Bubbles, Gantu's "characters with
/// cost ≤2 can't challenge your characters"); the target side is handled in
/// `target_legal_basic` (which already accepts locations, §4.3.6.19–22).
fn can_challenge(
    state: &GameState,
    registry: &CardRegistry,
    active: PlayerId,
    challenger: CardId,
    target: CardId,
) -> Result<(), Rejected> {
    // Challenger side: a ready character, dry unless it has Rush (§4.3.6.6, §10.9).
    let challenger_instance = find_in_play(state, active, challenger)?;
    if !challenger_instance.is_character() {
        return Err(Rejected::NotACharacter(challenger));
    }
    if challenger_instance.conditions().drying
        && !character_has_keyword(state, registry, challenger, &Keyword::Rush)
    {
        return Err(Rejected::CharacterStillDrying(challenger));
    }
    if !challenger_instance.conditions().ready {
        return Err(Rejected::CharacterExerted(challenger));
    }

    // Target side (basics).
    target_legal_basic(state, registry, active, challenger, target)?;

    // Bodyguard must-choose (§10.3.3): only applies when choosing a *character*
    // to challenge (not a location). If the target isn't a Bodyguard and the
    // defender has a Bodyguard this challenger could *legally* challenge (basics
    // pass), one of those must be chosen instead.
    let target_is_character = state
        .instance_in_play(target)
        .is_some_and(CardInstance::is_character);
    if target_is_character && !character_has_keyword(state, registry, target, &Keyword::Bodyguard) {
        let owner =
            opposing_owner_of(state, active, target).expect("validated by target_legal_basic");
        let forced = state.player(owner).is_some_and(|p| {
            p.play().iter().any(|c| {
                character_has_keyword(state, registry, c.id(), &Keyword::Bodyguard)
                    && target_legal_basic(state, registry, active, challenger, c.id()).is_ok()
            })
        });
        if forced {
            return Err(Rejected::MustChallengeBodyguard(target));
        }
    }
    Ok(())
}

/// Whether `challenger` could legally challenge **any** opposing card right now —
/// character or location (used by Reckless and Bodyguard "if able"). It scans all
/// opposing in-play cards, so locations are covered via `can_challenge`.
fn can_legally_challenge_anything(
    state: &GameState,
    registry: &CardRegistry,
    active: PlayerId,
    challenger: CardId,
) -> bool {
    state
        .players()
        .iter()
        .filter(|p| p.id() != active)
        .any(|p| {
            p.play()
                .iter()
                .any(|c| can_challenge(state, registry, active, challenger, c.id()).is_ok())
        })
}

/// A ready Reckless character of `active` that can still legally challenge,
/// which blocks ending the turn (§10.7.3), if any.
fn reckless_must_challenge(
    state: &GameState,
    registry: &CardRegistry,
    active: PlayerId,
) -> Option<CardId> {
    state.player(active)?.play().iter().find_map(|c| {
        let id = c.id();
        (c.conditions().ready
            && character_has_keyword(state, registry, id, &Keyword::Reckless)
            && can_legally_challenge_anything(state, registry, active, id))
        .then_some(id)
    })
}

/// The non-active player whose play area contains `card`, if any.
fn opposing_owner_of(state: &GameState, active: PlayerId, card: CardId) -> Option<PlayerId> {
    state
        .players()
        .iter()
        .filter(|p| p.id() != active)
        .find(|p| p.play().contains(card))
        .map(super::super::game::PlayerState::id)
}

/// Find the definition id of a card in the active player's hand, or reject.
fn hand_card_definition(
    state: &GameState,
    player: PlayerId,
    card: CardId,
) -> Result<CardDefId, Rejected> {
    state
        .player(player)
        .expect("active player exists")
        .hand()
        .iter()
        .find(|c| c.id() == card)
        .map(CardInstance::definition)
        .ok_or(Rejected::CardNotInHand(card))
}

/// Use an activated ability (§7.5): pay the cost, then resolve the effect
/// **immediately** — activated abilities do not go to the bag (§7.5.3.3).
fn apply_use_ability(
    state: &mut GameState,
    registry: &CardRegistry,
    card: CardId,
    ability_index: usize,
) -> Result<Vec<GameEvent>, Rejected> {
    // --- validate (no mutation yet) ---
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();
    let instance = find_in_play(state, active, card)?;
    let ability = registry
        .get(instance.definition())
        .ok_or(Rejected::UnknownCard(card))?
        .activated_abilities()
        .get(ability_index)
        .ok_or(Rejected::NoSuchAbility(card))?;
    let cost = ability.cost;
    let effect = ability.effect.clone();

    // Cost legality. Drying characters can't pay an exert cost (§4.2.2.1).
    if cost.exert_self {
        if instance.conditions().drying {
            return Err(Rejected::CharacterStillDrying(card));
        }
        if !instance.conditions().ready {
            return Err(Rejected::CharacterExerted(card));
        }
    }
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < cost.ink
    {
        return Err(Rejected::InsufficientInk(card));
    }

    // --- pay the cost ---
    {
        let p = state.player_mut(active).expect("active player exists");
        p.exert_ink(cost.ink);
        if cost.exert_self
            && let Some(c) = p.play_mut().iter_mut().find(|c| c.id() == card)
        {
            c.conditions_mut().ready = false;
        }
    }

    // --- resolve the effect immediately (§7.5.3.3) ---
    let mut events = vec![GameEvent::AbilityActivated {
        player: active,
        card,
    }];
    resolve_effects(state, registry, active, card, vec![effect], &mut events);
    events.extend(game_state_check_with_triggers(state, registry));
    // Resolve any triggers the effect produced (e.g. a banished card's "when
    // banished"), unless the effect itself is awaiting a target choice.
    if !state.is_awaiting_decision() && !state.is_finished() {
        events.extend(resolve_bag(state, registry));
    }
    Ok(events)
}

fn apply_end_turn(
    state: &mut GameState,
    registry: &CardRegistry,
) -> Result<Vec<GameEvent>, Rejected> {
    if !matches!(state.status(), GameStatus::Playing) {
        return Err(Rejected::NotPlaying);
    }
    if state.phase() != Phase::Main {
        return Err(Rejected::NotMainPhase);
    }
    let active = state.active_player();

    // Reckless (§10.7.3): can't end the turn while a ready Reckless character can
    // still legally challenge an opposing character *or location* (reuses the
    // `can_challenge` authority, so it respects Evasive/Bodyguard/Rush/locations).
    if let Some(reckless) = reckless_must_challenge(state, registry, active) {
        return Err(Rejected::RecklessMustChallenge(reckless));
    }

    let mut events = Vec::new();
    state.set_phase(Phase::End);
    state.set_step(Step::End);
    events.push(GameEvent::StepEntered { step: Step::End });
    // "Until end of turn" effects end here (§7.6.1).
    state.expire_end_of_turn_modifiers();
    // TODO(Slice 5h — start/end-of-turn triggers): "at the end of your turn"
    // triggers would be enqueued here, but resolving them can suspend on a
    // decision, and the engine can't yet resume the turn transition afterward.
    // Needs the turn-progression-with-suspension machinery — see "Slice 5h" in
    // docs/planning/IMPLEMENTATION_PLAN.md.
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
    state.clear_boosted_this_turn();

    let mut events = vec![GameEvent::TurnStarted {
        player: active,
        turn: state.turn_number(),
    }];
    // TODO(Slice 5h — start/end-of-turn triggers): "at the start of your turn"
    // triggers would be enqueued around here, pending the
    // turn-progression-with-suspension machinery (see "Slice 5h" in
    // docs/planning/IMPLEMENTATION_PLAN.md). begin_turn would also need the
    // registry threaded through start/apply_end_turn.

    // Ready step (§4.2.1).
    state.set_phase(Phase::Beginning);
    state.set_step(Step::Ready);
    events.push(GameEvent::StepEntered { step: Step::Ready });
    ready_all(state, active);
    events.extend(game_state_check(state));
    if state.is_finished() {
        return events;
    }

    // Set step (§4.2.2): dry characters, gain lore from locations (§6.5.6). (Resolve
    // start-of-turn triggers — none yet; see the Slice 5h TODO above.)
    state.set_step(Step::Set);
    events.push(GameEvent::StepEntered { step: Step::Set });
    dry_characters(state, active);
    let location_lore: u32 = state.player(active).map_or(0, |p| {
        p.play()
            .iter()
            .filter_map(|c| c.location_stats().map(|l| l.lore))
            .sum()
    });
    if location_lore > 0 {
        if let Some(p) = state.player_mut(active) {
            p.add_lore(location_lore);
        }
        events.push(GameEvent::LoreGained {
            player: active,
            amount: location_lore,
        });
    }
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

// ---------------------------------------------------------------------------
// The bag: enqueueing and resolving triggered abilities (§8.7).
// ---------------------------------------------------------------------------

/// Enqueue the source card's own triggers whose condition matches (e.g. a
/// character's "when you play this character" or "whenever this character
/// quests"). Only self-scoped triggers are detected so far (see the
/// `TriggerCondition` TODO for the broader scope/event space).
fn enqueue_self_triggers(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    condition: &TriggerCondition,
) {
    let Ok(instance) = find_in_play(state, controller, source) else {
        return;
    };
    enqueue_triggers_for_def(
        state,
        registry,
        controller,
        source,
        instance.definition(),
        condition,
    );
}

/// Enqueue triggers matching `condition` from `source`'s definition. Works for a
/// `source` that is no longer in play (e.g. a just-banished card now in the
/// discard) since it reads abilities from the definition, not the instance.
fn enqueue_triggers_for_def(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    definition_id: CardDefId,
    condition: &TriggerCondition,
) {
    let Some(definition) = registry.get(definition_id) else {
        return;
    };
    let matches: Vec<(bool, Effect)> = definition
        .abilities()
        .iter()
        .filter(|a| a.condition == *condition)
        .map(|a| (a.optional, a.effect.clone()))
        .collect();
    for (optional, effect) in matches {
        let _ = state.enqueue_trigger(controller, source, optional, effect);
    }
}

/// Enqueue "when this is banished" / "...in a challenge" triggers for each card
/// banished by the just-run game-state check (the `Banished` events). The card is
/// already in the discard (dissolved), so its triggers are read from its def.
///
/// Effect-driven (non-challenge) banishment routes through
/// `game_state_check_with_triggers`, which calls this with `in_challenge = false`,
/// so `WhenBanished` is centralized (the move-zone effects — Marshmallow / Gramma
/// Tala — also work now, Slice 8a-1/8b).
///
/// TODO(Slice 8+): §1.9.1.3 "banished by that character" attribution for
/// who-banished-whom effects still needs effect context.
fn enqueue_banish_triggers(
    state: &mut GameState,
    registry: &CardRegistry,
    check_events: &[GameEvent],
    in_challenge: bool,
) {
    let banished: Vec<(PlayerId, CardId)> = check_events
        .iter()
        .filter_map(|e| match e {
            GameEvent::Banished { player, card } => Some((*player, *card)),
            _ => None,
        })
        .collect();
    for (owner, card) in banished {
        let Some(def_id) = state
            .player(owner)
            .and_then(|p| p.discard().iter().find(|c| c.id() == card))
            .map(CardInstance::definition)
        else {
            continue;
        };
        enqueue_triggers_for_def(
            state,
            registry,
            owner,
            card,
            def_id,
            &TriggerCondition::WhenBanished,
        );
        if in_challenge {
            enqueue_triggers_for_def(
                state,
                registry,
                owner,
                card,
                def_id,
                &TriggerCondition::WhenBanishedInChallenge,
            );
        }
    }
}

/// Run the game-state check and enqueue "when banished" triggers for anything it
/// banished (the centralized banishment path for **effect-driven** banishment —
/// `apply_challenge` handles the in-challenge variants itself). The caller
/// resolves the bag.
fn game_state_check_with_triggers(
    state: &mut GameState,
    registry: &CardRegistry,
) -> Vec<GameEvent> {
    let events = game_state_check(state);
    enqueue_banish_triggers(state, registry, &events, false);
    events
}

/// Enqueue "whenever you play a [category]" triggers on the controller's other
/// in-play cards when `played` (a card the controller just played) matches the
/// category. This is the cross-scope event→trigger matcher (vs. the self-only
/// `enqueue_self_triggers`); only `WhenYouPlay` is matched here so far.
fn enqueue_play_a_card_triggers(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    played: CardId,
    played_def: CardDefId,
) {
    // The played card's category comes from its definition (it may not be in play
    // — actions are discarded), so this works for characters and actions/songs.
    let Some(played_definition) = registry.get(played_def) else {
        return;
    };
    let Some(owner) = state.player(controller) else {
        return;
    };
    let mut to_enqueue: Vec<(CardId, bool, Effect)> = Vec::new();
    for watcher in owner.play().iter() {
        if watcher.id() == played {
            continue; // a card's own play doesn't trigger its own "whenever you play"
        }
        let Some(definition) = registry.get(watcher.definition()) else {
            continue;
        };
        for ability in definition.abilities() {
            if let TriggerCondition::WhenYouPlay(category) = &ability.condition
                && category_matches(category, played_definition)
            {
                to_enqueue.push((watcher.id(), ability.optional, ability.effect.clone()));
            }
        }
    }
    for (source, optional, effect) in to_enqueue {
        let _ = state.enqueue_trigger(controller, source, optional, effect);
    }
}

/// Whether a played card (by its definition) matches a "whenever you play a …"
/// category. A song is an action, so it matches both `Action` and `Song`.
fn category_matches(category: &CardCategory, played: &CardDefinition) -> bool {
    match category {
        CardCategory::Character(filter) => {
            matches!(played.kind(), CardKind::Character { .. })
                && filter.as_ref().is_none_or(|c| played.has_classification(c))
        }
        CardCategory::Action => matches!(played.kind(), CardKind::Action),
        CardCategory::Song => played.is_song(),
        CardCategory::Item => matches!(played.kind(), CardKind::Item),
        CardCategory::Location => matches!(played.kind(), CardKind::Location { .. }),
    }
}

/// Apply a card's static abilities as it enters play (§7.6.2): each becomes a
/// continuous modifier lasting while the source is in play. The modifiers are
/// removed when the source leaves play (see `game_state_check`).
fn apply_enter_statics(
    state: &mut GameState,
    controller: PlayerId,
    card: CardId,
    statics: &[StaticAbility],
) {
    for ability in statics {
        let target = match &ability.target {
            StaticTarget::SelfCard => ModifierTarget::Card(card),
            StaticTarget::OwnedCharacters {
                classifications,
                include_self,
            } => ModifierTarget::OwnedCharacters {
                owner: controller,
                classifications: classifications.clone(),
                except: if *include_self { None } else { Some(card) },
            },
        };
        state.add_modifier(StatModifier::new(
            card,
            target,
            ability.stat,
            ability.delta,
            ModifierDuration::WhileSourceInPlay,
        ));
    }
}

/// Apply a card's game-rule static abilities as it enters play (the win/loss
/// modification layer, §1.2.1): each becomes a [`RuleModifier`] lasting while the
/// source is in play, removed on leave.
fn apply_enter_rule_statics(
    state: &mut GameState,
    controller: PlayerId,
    card: CardId,
    rule_statics: &[GameRuleStatic],
) {
    let opponents: Vec<PlayerId> = state
        .players()
        .iter()
        .map(super::super::game::PlayerState::id)
        .filter(|id| *id != controller)
        .collect();
    for rule in rule_statics {
        match rule {
            GameRuleStatic::OpponentsLoreToWin(threshold) => {
                for opponent in &opponents {
                    state.add_rule_modifier(RuleModifier::LoreToWin {
                        source: card,
                        player: *opponent,
                        threshold: *threshold,
                    });
                }
            }
        }
    }
}

/// Resolve the bag until it is empty or a player decision is required (§8.7).
/// The active player resolves all of their triggers first (choosing the order
/// when they have more than one), then each player around the table.
fn resolve_bag(state: &mut GameState, registry: &CardRegistry) -> Vec<GameEvent> {
    let mut events = Vec::new();
    while !state.is_awaiting_decision() && !state.is_finished() {
        let Some(player) = next_resolving_player(state) else {
            break; // bag empty
        };
        let theirs = state.triggers_for(player);
        if theirs.len() >= 2 {
            // §8.7.4: the player chooses which of their triggers resolves next.
            state.set_pending(PendingDecision::OrderTriggers {
                player,
                options: theirs,
            });
            break;
        }
        resolve_or_ask(state, registry, &mut events, theirs[0]);
    }
    events
}

/// The next player who should resolve a trigger: the active player if they have
/// any, otherwise each player around the table in turn order (§8.7.5–§8.7.6).
fn next_resolving_player(state: &GameState) -> Option<PlayerId> {
    let player_count = state.player_count();
    let start = usize::from(state.active_player().index());
    (0..player_count)
        .map(|offset| seat((start + offset) % player_count))
        .find(|p| !state.triggers_for(*p).is_empty())
}

/// Resolve a single trigger, or suspend on a "may" decision if it is optional.
fn resolve_or_ask(
    state: &mut GameState,
    registry: &CardRegistry,
    events: &mut Vec<GameEvent>,
    trigger: TriggerId,
) {
    let Some(entry) = state.bag().iter().find(|e| e.id() == trigger) else {
        return;
    };
    if entry.optional() {
        let player = entry.controller();
        state.set_pending(PendingDecision::MayResolve { player, trigger });
        return;
    }
    execute_trigger(state, registry, events, trigger);
}

/// Remove a trigger from the bag and apply its effect, then run a game-state
/// check (§8.7: a check follows each bag entry's resolution).
fn execute_trigger(
    state: &mut GameState,
    registry: &CardRegistry,
    events: &mut Vec<GameEvent>,
    trigger: TriggerId,
) {
    let Some(entry) = state.remove_trigger(trigger) else {
        return;
    };
    resolve_effects(
        state,
        registry,
        entry.controller(),
        entry.source(),
        vec![entry.effect()],
        events,
    );
    events.extend(game_state_check_with_triggers(state, registry));
}

/// Resolve a sequence of effects in order ("[A] then [B]", §7.1.2). If an effect
/// needs a target choice, stash the remaining effects in a `ChooseTarget` pending
/// and stop; `Decide` then applies the choice and resumes the `rest`. Effects with
/// no eligible target fizzle and the sequence continues ("as much as possible").
fn resolve_effects(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    effects: Vec<Effect>,
    events: &mut Vec<GameEvent>,
) {
    let mut iter = effects.into_iter();
    while let Some(effect) = iter.next() {
        if let Some((options, effect)) =
            execute_effect(state, registry, controller, source, &effect, events)
        {
            state.set_pending(PendingDecision::ChooseTarget {
                player: controller,
                source,
                options,
                effect,
                rest: iter.collect(),
            });
            return;
        }
    }
}

/// Apply a built-in effect for `controller`. Returns `Some((options, effect))`
/// when the effect needs the controller to choose a target (nothing applied yet);
/// otherwise applies the effect and returns `None`.
fn execute_effect(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) -> Option<(Vec<CardId>, Effect)> {
    match effect {
        Effect::DrawCards(n) => {
            for _ in 0..*n {
                events.push(draw(state, controller));
            }
        }
        Effect::GainLore(n) => {
            if let Some(p) = state.player_mut(controller) {
                p.add_lore(*n);
            }
            events.push(GameEvent::LoreGained {
                player: controller,
                amount: *n,
            });
        }
        Effect::EachOpponentLosesLore(n) => {
            let opponents: Vec<PlayerId> = state
                .players()
                .iter()
                .map(super::super::game::PlayerState::id)
                .filter(|id| *id != controller)
                .collect();
            for opponent in opponents {
                if let Some(p) = state.player_mut(opponent) {
                    p.lose_lore(*n);
                }
                events.push(GameEvent::LoreLost {
                    player: opponent,
                    amount: *n,
                });
            }
        }
        // Targeted effects: resolve the target now (self / all) or report that a
        // choice is needed (§7.1).
        Effect::ReturnToHand(target)
        | Effect::IntoInkwell(target)
        | Effect::GiveStrengthThisTurn { target, .. }
        | Effect::DealDamage { target, .. }
        | Effect::RemoveDamage { target, .. }
        | Effect::Banish(target) => match target {
            Target::SelfCard => apply_effect_to(state, registry, source, source, effect, events),
            Target::ChosenCharacter { filter, another } => {
                let options =
                    chosen_character_options(state, registry, controller, source, filter, *another);
                if !options.is_empty() {
                    return Some((options, effect.clone()));
                }
            }
            Target::AllCharacters(filter) => {
                let targets =
                    chosen_character_options(state, registry, controller, source, filter, false);
                for card in targets {
                    apply_effect_to(state, registry, source, card, effect, events);
                }
            }
            Target::ChosenItem { side } => {
                let options =
                    chosen_permanent_options(state, controller, *side, PermanentKind::Item);
                if !options.is_empty() {
                    return Some((options, effect.clone()));
                }
            }
            Target::ChosenLocation { side } => {
                let options =
                    chosen_permanent_options(state, controller, *side, PermanentKind::Location);
                if !options.is_empty() {
                    return Some((options, effect.clone()));
                }
            }
        },
    }
    None
}

/// A non-character permanent kind that can be a target.
#[derive(Clone, Copy)]
enum PermanentKind {
    Item,
    Location,
}

/// The in-play items / locations on the given `side` (relative to `controller`).
fn chosen_permanent_options(
    state: &GameState,
    controller: PlayerId,
    side: TargetSide,
    kind: PermanentKind,
) -> Vec<CardId> {
    let mut out = Vec::new();
    for player in state.players() {
        let is_yours = player.id() == controller;
        let side_ok = match side {
            TargetSide::Any => true,
            TargetSide::Yours => is_yours,
            TargetSide::Opposing => !is_yours,
        };
        if !side_ok {
            continue;
        }
        for card in player.play().iter() {
            let matches = match kind {
                // An item is an in-play card that is neither a character nor a location.
                PermanentKind::Item => !card.is_character() && !card.is_location(),
                PermanentKind::Location => card.is_location(),
            };
            if matches {
                out.push(card.id());
            }
        }
    }
    out
}

/// A zone an effect can move the source card to.
#[derive(Clone, Copy)]
enum SelfDestination {
    Hand,
    Inkwell,
}

/// Move `card` (the effect's source) from play or the discard to `owner`'s hand
/// or inkwell, dissolving any stack into the destination (§5.1.7). If it was in
/// play, its continuous modifiers end (§7.6.4).
fn move_self_card(state: &mut GameState, owner: PlayerId, card: CardId, dest: SelfDestination) {
    let was_in_play;
    {
        let Some(p) = state.player_mut(owner) else {
            return;
        };
        was_in_play = p.play().contains(card);
        let taken = if was_in_play {
            p.play_mut().take(card)
        } else {
            p.discard_mut().take(card)
        };
        let Some(instance) = taken else {
            return;
        };
        let conditions = match dest {
            SelfDestination::Hand => Conditions::faceup_idle(),
            // Facedown and exerted (Gramma Tala "facedown and exerted").
            SelfDestination::Inkwell => Conditions {
                ready: false,
                damage: 0,
                drying: false,
                facedown: true,
            },
        };
        for moved in instance.dissolve(conditions) {
            match dest {
                SelfDestination::Hand => p.hand_mut().push(moved),
                SelfDestination::Inkwell => p.inkwell_mut().push(moved),
            }
        }
    }
    if was_in_play {
        state.remove_modifiers_from_source(card);
    }
}

/// Apply a targeted effect to an already-resolved `target_card` (after a
/// `SelfCard`/`AllCharacters` resolution or a `ChooseTarget` decision). The
/// untargeted variants never reach here.
fn apply_effect_to(
    state: &mut GameState,
    registry: &CardRegistry,
    source: CardId,
    target_card: CardId,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) {
    match effect {
        Effect::ReturnToHand(_) => {
            if let Some(owner) = owner_holding(state, target_card) {
                move_self_card(state, owner, target_card, SelfDestination::Hand);
            }
        }
        Effect::IntoInkwell(_) => {
            if let Some(owner) = owner_holding(state, target_card) {
                move_self_card(state, owner, target_card, SelfDestination::Inkwell);
            }
        }
        Effect::GiveStrengthThisTurn { amount, .. } => {
            state.add_modifier(StatModifier::new(
                source,
                ModifierTarget::Card(target_card),
                Stat::Strength,
                *amount,
                ModifierDuration::UntilEndOfTurn,
            ));
        }
        Effect::DealDamage { amount, .. } => {
            if let Some(owner) = owner_holding(state, target_card) {
                add_damage(state, owner, target_card, *amount);
            }
        }
        Effect::RemoveDamage { amount, .. } => {
            if let Some(owner) = owner_holding(state, target_card)
                && let Some(c) = state
                    .player_mut(owner)
                    .and_then(|p| p.play_mut().iter_mut().find(|c| c.id() == target_card))
            {
                let conditions = c.conditions_mut();
                conditions.damage = conditions.damage.saturating_sub(*amount);
            }
        }
        Effect::Banish(_) => {
            banish_by_effect(state, registry, target_card, events);
        }
        Effect::DrawCards(_) | Effect::GainLore(_) | Effect::EachOpponentLosesLore(_) => {}
    }
}

/// Banish `card` directly by an effect (not via damage, not in a challenge):
/// dissolve its stack into the owner's discard (§5.1.7), end its continuous
/// modifiers (§7.6.4), emit `Banished`, and enqueue its "when banished" trigger.
/// Mirrors the game-state-check banishment but is registry-aware so the trigger
/// can be read (the move-zone variants — e.g. Marshmallow — then relocate it).
fn banish_by_effect(
    state: &mut GameState,
    registry: &CardRegistry,
    card: CardId,
    events: &mut Vec<GameEvent>,
) {
    let Some(owner) = owner_holding(state, card) else {
        return;
    };
    if let Some(p) = state.player_mut(owner)
        && let Some(instance) = p.play_mut().take(card)
    {
        for c in instance.dissolve(Conditions::faceup_idle()) {
            p.discard_mut().push(c);
        }
    } else {
        return; // not in play (e.g. already gone)
    }
    state.remove_modifiers_from_source(card);
    events.push(GameEvent::Banished {
        player: owner,
        card,
    });
    if let Some(def_id) = state
        .player(owner)
        .and_then(|p| p.discard().iter().find(|c| c.id() == card))
        .map(CardInstance::definition)
    {
        enqueue_triggers_for_def(
            state,
            registry,
            owner,
            card,
            def_id,
            &TriggerCondition::WhenBanished,
        );
    }
}

/// The in-play characters eligible for a [`CharacterFilter`] from `controller`'s
/// perspective (side, classifications, cost/`{S}`, damaged/exerted), optionally
/// excluding the `source`.
fn chosen_character_options(
    state: &GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    filter: &CharacterFilter,
    exclude_source: bool,
) -> Vec<CardId> {
    let mut out = Vec::new();
    for player in state.players() {
        let is_yours = player.id() == controller;
        let side_ok = match filter.side {
            TargetSide::Any => true,
            TargetSide::Yours => is_yours,
            TargetSide::Opposing => !is_yours,
        };
        if !side_ok {
            continue;
        }
        for card in player.play().iter() {
            if card.is_character()
                && !(exclude_source && card.id() == source)
                && character_matches_filter(state, registry, card, filter)
            {
                out.push(card.id());
            }
        }
    }
    out
}

/// Whether an in-play character matches every set dimension of a filter.
fn character_matches_filter(
    state: &GameState,
    registry: &CardRegistry,
    card: &CardInstance,
    filter: &CharacterFilter,
) -> bool {
    if !filter
        .classifications
        .iter()
        .all(|c| card.has_classification(c))
    {
        return false;
    }
    if let Some(nf) = filter.cost {
        let cost = registry
            .get(card.definition())
            .map_or(0, CardDefinition::cost);
        if !nf.matches(cost) {
            return false;
        }
    }
    if let Some(nf) = filter.strength {
        let strength = state
            .current_character_stats(card.id())
            .map_or(0, |s| s.strength);
        if !nf.matches(strength) {
            return false;
        }
    }
    if let Some(want_damaged) = filter.damaged
        && (card.conditions().damage > 0) != want_damaged
    {
        return false;
    }
    // exerted == !ready, so "exerted != want" simplifies to "ready == want".
    if let Some(want_exerted) = filter.exerted
        && card.conditions().ready == want_exerted
    {
        return false;
    }
    true
}

/// The player whose play area or discard currently holds `card`.
fn owner_holding(state: &GameState, card: CardId) -> Option<PlayerId> {
    state
        .players()
        .iter()
        .find(|p| p.play().contains(card) || p.discard().contains(card))
        .map(super::super::game::PlayerState::id)
}

/// Answer the pending bag decision and continue resolving (§8.7).
fn apply_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    decision: Decision,
) -> Result<Vec<GameEvent>, Rejected> {
    let Some(pending) = state.pending().cloned() else {
        return Err(Rejected::NoPendingDecision);
    };
    let mut events = Vec::new();
    match (pending, decision) {
        (PendingDecision::OrderTriggers { options, .. }, Decision::ResolveNext(trigger)) => {
            if !options.contains(&trigger) {
                return Err(Rejected::InvalidDecision);
            }
            let _ = state.take_pending();
            resolve_or_ask(state, registry, &mut events, trigger);
        }
        (PendingDecision::MayResolve { trigger, .. }, Decision::May(apply_it)) => {
            let _ = state.take_pending();
            if apply_it {
                execute_trigger(state, registry, &mut events, trigger);
            } else {
                let _ = state.remove_trigger(trigger);
            }
        }
        (PendingDecision::EnterPlayExerted { player, card }, Decision::EnterExerted(exert)) => {
            let _ = state.take_pending();
            if exert
                && let Some(p) = state.player_mut(player)
                && let Some(c) = p.play_mut().iter_mut().find(|c| c.id() == card)
            {
                c.conditions_mut().ready = false;
            }
            // Now that it has entered (ready or exerted), run its enters-play
            // triggers (§10.3.2 resolves before the enters-play trigger window).
            if let Some(definition_id) = state.instance_in_play(card).map(CardInstance::definition)
            {
                enqueue_enter_play_triggers(state, registry, player, card, definition_id);
            }
        }
        (
            PendingDecision::ChooseTarget {
                player,
                source,
                options,
                effect,
                rest,
            },
            Decision::ChooseTarget(chosen),
        ) => {
            if !options.contains(&chosen) {
                return Err(Rejected::InvalidDecision);
            }
            let _ = state.take_pending();
            apply_effect_to(state, registry, source, chosen, &effect, &mut events);
            // Resume the remaining "[A] then [B]" effects (may suspend again).
            resolve_effects(state, registry, player, source, rest, &mut events);
            events.extend(game_state_check_with_triggers(state, registry));
        }
        _ => return Err(Rejected::InvalidDecision),
    }
    if !state.is_awaiting_decision() {
        events.extend(resolve_bag(state, registry));
    }
    Ok(events)
}
