//! The reducer: `start` sets a game up, `apply` advances it by one input.

use super::input::{Decision, Input, Rejected};
use crate::domain::cards::{
    CardDefinition, CardKind, CardRegistry, GameRuleStatic, Keyword, ShiftAbility, ShiftCost,
    ShiftKind, StaticAbility, StaticTarget,
};
use crate::domain::effects::{
    Amount, CardCategory, CharacterFilter, CountCondition, DeckPosition, DelayedWhen, Destination,
    DiscardAmount, DiscardBy, Effect, MoveSource, PlayerScope, Target, TargetSide,
    TriggerCondition,
};
use crate::domain::game::{
    CardInstance, CharacterStats, ChoiceRef, ChoiceThen, Conditions, DelayedTrigger, GameEvent,
    GameState, GameStatus, GrantedActivated, GrantedTrigger, LocationStats, ModifierDuration,
    ModifierTarget, PendingDecision, Permission, PlayerState, Property, PropertyModifier,
    Restriction, RuleModifier, Stat, StatModifier, TriggerId,
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
        Input::Mulligan { player, put_back } => apply_mulligan(state, registry, player, &put_back),
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
    registry: &CardRegistry,
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
    events.extend(advance_after_mulligan(state, registry, player));
    Ok(events)
}

/// Move mulligan to the next player in turn order, or start the first turn.
fn advance_after_mulligan(
    state: &mut GameState,
    registry: &CardRegistry,
    just_resolved: PlayerId,
) -> Vec<GameEvent> {
    let player_count = state.player_count();
    let starting = usize::from(state.active_player().index());
    let offset = (usize::from(just_resolved.index()) + player_count - starting) % player_count;

    if offset + 1 >= player_count {
        state.set_status(GameStatus::Playing);
        begin_turn(state, registry, true)
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

    // Enqueue "whenever a card is put into your inkwell" triggers
    enqueue_turn_triggers(
        state,
        registry,
        active,
        &TriggerCondition::WhenCardPutInInkwell,
    );

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
    enqueue_enter_play_triggers(
        state,
        registry,
        active,
        card,
        definition_id,
        shift_onto.is_some(),
    );
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
    was_shifted: bool,
) {
    enqueue_self_triggers(
        state,
        registry,
        controller,
        card,
        &TriggerCondition::WhenYouPlayThis,
    );
    if was_shifted {
        enqueue_self_triggers(
            state,
            registry,
            controller,
            card,
            &TriggerCondition::WhenYouPlayThisWithShift,
        );
    }
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
            let names = definition.names().to_vec();
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
                    ink_cost,
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
                    names,
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
    names: Vec<String>,
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
    instance.set_printed_cost(ink_cost);
    instance.set_names(names);
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
    printed_cost: u32,
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
        top.set_printed_cost(printed_cost);
        top.set_names(shift_names.to_vec());
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
    // Can't-quest prevention — Reckless (§10.7.2) or an effect (§1.2.2).
    if has_restriction(state, registry, character, Restriction::CantQuest) {
        return Err(Rejected::CharacterCannotQuest(character));
    }
    // Questing requires a dry, ready character (§4.3.5.5), unless permitted to
    // quest while drying.
    if instance.conditions().drying
        && !has_permission(state, registry, character, Permission::QuestWhileDrying)
    {
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
    // §10.13: add this character's `{S}` to another chosen character's `{S}`.
    // Evaluated at resolution (`SourceStat`) so it reflects modifiers already on
    // the source — e.g. if another Support buffed it earlier this turn, this
    // Support adds the **combined** value (§7.8 current value at resolution).
    let _ = state.enqueue_trigger(
        controller,
        character,
        true, // "you may"
        Effect::GiveStrengthThisTurn {
            target: Target::ChosenCharacter {
                filter: CharacterFilter::any(TargetSide::Any)
                    .and(CharacterFilter::negate(CharacterFilter::IsSource)),
            },
            amount: Amount::StatOf {
                stat: Stat::Strength,
                target: Target::SelfCard,
            },
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
#[allow(clippy::too_many_lines)]
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
    // Current Strength includes continuous modifiers, clamped at 0 (§4.3.6.14,
    // §7.8.2); the challenger also gets Challenger +N while challenging (§10.5).
    // Challenger/Resist are effective values (printed + effect-granted).
    let challenger_strength = state
        .current_character_stats(challenger)
        .map_or(0, |s| s.strength)
        + effective_challenger_bonus(state, registry, challenger);
    let target_strength = state
        .current_character_stats(target)
        .map_or(0, |s| s.strength);

    // Resist +N reduces the damage taken (§10.8); "takes no damage" prevents it
    // (§7.7 / §1.2.2). See `combat_damage`.
    let damage_to_target = combat_damage(state, registry, challenger_strength, target);
    let damage_to_challenger = combat_damage(state, registry, target_strength, challenger);

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

    // Enqueue damage triggers
    if damage_to_target > 0 {
        enqueue_self_triggers(
            state,
            registry,
            target_owner,
            target,
            &TriggerCondition::WhenThisIsDealtDamage,
        );
        let opponent = active; // The challenger dealt damage to the target
        let opp_cards: Vec<_> = state
            .player(opponent)
            .expect("opponent exists")
            .play()
            .iter()
            .map(|c| (c.id(), c.definition()))
            .collect();
        for (card_id, def_id) in opp_cards {
            enqueue_triggers_for_def(
                state,
                registry,
                opponent,
                card_id,
                def_id,
                &TriggerCondition::WhenOpposingIsDealtDamage,
            );
        }
    }
    if damage_to_challenger > 0 {
        enqueue_self_triggers(
            state,
            registry,
            active,
            challenger,
            &TriggerCondition::WhenThisIsDealtDamage,
        );
        let opponent = target_owner; // The target dealt damage to the challenger
        let opp_cards: Vec<_> = state
            .player(opponent)
            .expect("opponent exists")
            .play()
            .iter()
            .map(|c| (c.id(), c.definition()))
            .collect();
        for (card_id, def_id) in opp_cards {
            enqueue_triggers_for_def(
                state,
                registry,
                opponent,
                card_id,
                def_id,
                &TriggerCondition::WhenOpposingIsDealtDamage,
            );
        }
    }

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

/// Resolve one endpoint of a move-damage effect: `SelfCard` is the source, an
/// already-resolved `Card` is itself, any other (chosen) target is `chosen`.
const fn move_endpoint(t: &Target, source: CardId, chosen: CardId) -> CardId {
    match t {
        Target::SelfCard => source,
        Target::Card(id) => *id,
        _ => chosen,
    }
}

/// Whether a target requires the controller to choose it at resolution (vs the
/// already-resolved `SelfCard` / `Card`).
const fn is_chosen_target(t: &Target) -> bool {
    !matches!(t, Target::SelfCard | Target::Card(_))
}

/// Resolve a player-scoped discard: apply to each player in scope, or prompt a
/// choose-a-player decision for a `Chosen*` scope with 2+ candidates (§8.4).
fn resolve_discard_effect(
    state: &mut GameState,
    controller: PlayerId,
    who: PlayerScope,
    amount: DiscardAmount,
    by: DiscardBy,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) -> Option<Choice> {
    match resolve_scope(state, controller, who) {
        ScopeOutcome::Players(players) => {
            resolve_scope_discard(state, &players, amount, by, events)
        }
        ScopeOutcome::Choose(options) => Some(choose_player(controller, options, effect)),
    }
}

/// Reveal `player`'s hand: emit a `HandRevealed` information event (§8.x).
fn reveal_hand(state: &GameState, player: PlayerId, events: &mut Vec<GameEvent>) {
    let cards: Vec<CardId> = state
        .player(player)
        .map(|p| p.hand().iter().map(CardInstance::id).collect())
        .unwrap_or_default();
    events.push(GameEvent::HandRevealed { player, cards });
}

/// An already-resolved move-damage endpoint: the source (`SelfCard`) or a
/// specific `Card`. `None` if it still needs to be chosen.
const fn resolved_endpoint(t: &Target, source: CardId) -> Option<CardId> {
    match t {
        Target::SelfCard => Some(source),
        Target::Card(id) => Some(*id),
        _ => None,
    }
}

/// Resolve a move-damage effect one endpoint at a time (§9.3): once both are
/// concrete, move; otherwise prompt for the next unresolved endpoint, excluding
/// the endpoint that's already fixed so the two can't be the same card.
#[allow(clippy::too_many_arguments)]
fn resolve_move_damage(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    from: &Target,
    to: &Target,
    amount: &Amount,
    effect: &Effect,
) -> Option<Choice> {
    match (
        resolved_endpoint(from, source),
        resolved_endpoint(to, source),
    ) {
        (Some(f), Some(t)) => {
            let max = state.eval_amount(controller, source, source, amount).max(0);
            move_damage(state, f, t, max);
            None
        }
        // Pick the first unresolved endpoint. (Once `from` is fixed, the `to`
        // filter already excludes it via `Not(IsCard(..))` — see substitution.)
        (fixed_from, _) => {
            let target = if fixed_from.is_none() { from } else { to };
            let options = endpoint_options(state, registry, controller, source, target);
            (!options.is_empty()).then(|| choose_card(controller, options, effect))
        }
    }
}

/// The cards a chosen move-damage endpoint (a `ChosenCharacter`) may pick from.
fn endpoint_options(
    state: &GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    target: &Target,
) -> Vec<CardId> {
    if let Target::ChosenCharacter { filter } = target {
        choosable_characters(state, registry, controller, source, filter)
    } else {
        Vec::new()
    }
}

/// Re-target a two-target move-damage onto the just-chosen endpoint: the first
/// still-chosen side becomes a resolved `Card`.
fn substitute_move_endpoint(effect: &Effect, chosen: CardId) -> Effect {
    if let Effect::MoveDamage { from, to, amount } = effect {
        if is_chosen_target(from) {
            // Fixing `from`: constrain the still-to-pick `to` to exclude it, so the
            // two endpoints can't be the same card (via the filter, §9.3).
            Effect::MoveDamage {
                from: Target::Card(chosen),
                to: exclude_card_from_target(to, chosen),
                amount: amount.clone(),
            }
        } else {
            Effect::MoveDamage {
                from: from.clone(),
                to: Target::Card(chosen),
                amount: amount.clone(),
            }
        }
    } else {
        effect.clone()
    }
}

/// Add a `Not(IsCard(card))` predicate to a chosen-character target's filter
/// (so a later pick can't reselect an already-resolved card).
fn exclude_card_from_target(target: &Target, card: CardId) -> Target {
    match target {
        Target::ChosenCharacter { filter } => Target::ChosenCharacter {
            filter: filter
                .clone()
                .and(CharacterFilter::negate(CharacterFilter::IsCard(card))),
        },
        other => other.clone(),
    }
}

/// Apply a [`Effect::MoveDamage`] to the resolved `target_card` (the chosen side);
/// the other side is the `SelfCard` source. Capped by `from`'s damage (§9.3).
#[allow(clippy::too_many_arguments)]
fn apply_move_damage(
    state: &mut GameState,
    controller: PlayerId,
    source: CardId,
    target_card: CardId,
    from: &Target,
    to: &Target,
    amount: &Amount,
) {
    let max = state
        .eval_amount(controller, source, target_card, amount)
        .max(0);
    let from_card = move_endpoint(from, source, target_card);
    let to_card = move_endpoint(to, source, target_card);
    move_damage(state, from_card, to_card, max);
}

/// Move up to `max` damage counters from `from` to `to`, capped by the damage
/// actually on `from` (§9.3). A no-op if they're the same card or `from` is clean.
fn move_damage(state: &mut GameState, from: CardId, to: CardId, max: i32) {
    if from == to {
        return;
    }
    let on_from = state
        .instance_in_play(from)
        .map_or(0, |c| c.conditions().damage);
    let moved = on_from.min(u32::try_from(max).unwrap_or(0));
    if moved == 0 {
        return;
    }
    if let Some(owner) = owner_holding(state, from)
        && let Some(c) = state
            .player_mut(owner)
            .and_then(|p| p.play_mut().iter_mut().find(|c| c.id() == from))
    {
        c.conditions_mut().damage -= moved;
    }
    if let Some(owner) = owner_holding(state, to) {
        add_damage(state, owner, to, moved);
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
    let printed = state
        .instance_in_play(card)
        .and_then(|i| registry.get(i.definition()))
        .is_some_and(|d| d.has_keyword(keyword));
    // Effect-granted keywords ("gains Alert/Evasive/…") OR in (§10, §1.2.1).
    printed || state.granted_keywords(card).iter().any(|k| k == keyword)
}

/// Whether `card` has a [`Permission`] — granted by an effect **or** implied by a
/// keyword (Alert ⇒ may challenge Evasive §10.2; Rush ⇒ may challenge while drying
/// §10.9), so the legality checks consult a single authority per permission.
fn has_permission(
    state: &GameState,
    registry: &CardRegistry,
    card: CardId,
    permission: Permission,
) -> bool {
    if state.has_permission(card, permission) {
        return true;
    }
    match permission {
        Permission::ChallengeEvasive => {
            character_has_keyword(state, registry, card, &Keyword::Alert)
        }
        Permission::ChallengeWhileDrying => {
            character_has_keyword(state, registry, card, &Keyword::Rush)
        }
        Permission::ChallengeReady | Permission::QuestWhileDrying => false,
    }
}

/// Whether `card` has a [`Restriction`] — granted by an effect **or** implied by a
/// keyword (Reckless ⇒ can't quest, §10.7.2). A single authority per restriction,
/// mirroring [`has_permission`].
fn has_restriction(
    state: &GameState,
    registry: &CardRegistry,
    card: CardId,
    restriction: Restriction,
) -> bool {
    if state.has_restriction(card, restriction) {
        return true;
    }
    match restriction {
        Restriction::CantQuest => character_has_keyword(state, registry, card, &Keyword::Reckless),
        // Ward ⇒ can't be chosen by opponents (§10.15).
        Restriction::CantBeChosen => character_has_keyword(state, registry, card, &Keyword::Ward),
        Restriction::CantChallenge
        | Restriction::CantBeChallenged
        | Restriction::CantReady
        | Restriction::TakesNoChallengeDamage => false,
    }
}

/// The damage `recipient` takes from an attacker of `attacker_strength` in a
/// challenge: 0 if it takes no challenge damage (§7.7/§1.2.2), else the attacker's
/// strength reduced by the recipient's effective Resist (§10.8).
fn combat_damage(
    state: &GameState,
    registry: &CardRegistry,
    attacker_strength: u32,
    recipient: CardId,
) -> u32 {
    if has_restriction(
        state,
        registry,
        recipient,
        Restriction::TakesNoChallengeDamage,
    ) {
        return 0;
    }
    attacker_strength.saturating_sub(effective_resist(state, registry, recipient))
}

/// The card's effective Challenger +N: printed plus any effect-granted Challenger.
fn effective_challenger_bonus(state: &GameState, registry: &CardRegistry, card: CardId) -> u32 {
    let printed = state
        .instance_in_play(card)
        .and_then(|i| registry.get(i.definition()))
        .map_or(0, CardDefinition::challenger_bonus);
    let granted: u32 = state
        .granted_keywords(card)
        .iter()
        .filter_map(|k| match k {
            Keyword::Challenger(n) => Some(*n),
            _ => None,
        })
        .sum();
    printed + granted
}

/// The card's effective Resist +N: printed plus any effect-granted Resist.
fn effective_resist(state: &GameState, registry: &CardRegistry, card: CardId) -> u32 {
    let printed = state
        .instance_in_play(card)
        .and_then(|i| registry.get(i.definition()))
        .map_or(0, CardDefinition::resist);
    let granted: u32 = state
        .granted_keywords(card)
        .iter()
        .filter_map(|k| match k {
            Keyword::Resist(n) => Some(*n),
            _ => None,
        })
        .sum();
    printed + granted
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
    // "Can't be challenged" effect/keyword (§1.2.2) — preventions win.
    if has_restriction(state, registry, target, Restriction::CantBeChallenged) {
        return Err(Rejected::TargetCannotBeChallenged(target));
    }
    // Must be exerted, unless the challenger may challenge ready characters (§4.3.6.7).
    if instance.conditions().ready
        && !has_permission(state, registry, challenger, Permission::ChallengeReady)
    {
        return Err(Rejected::TargetNotExerted(target));
    }
    // Evasive: only an Evasive challenger, or one permitted to challenge Evasive
    // (Alert / effect), may challenge it (§10.6/§10.2).
    if character_has_keyword(state, registry, target, &Keyword::Evasive)
        && !character_has_keyword(state, registry, challenger, &Keyword::Evasive)
        && !has_permission(state, registry, challenger, Permission::ChallengeEvasive)
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
    // Challenger side: a ready character, dry unless permitted to challenge while
    // drying (Rush or effect, §4.3.6.6/§10.9), and not under a "can't challenge"
    // prevention (§1.2.2).
    let challenger_instance = find_in_play(state, active, challenger)?;
    if !challenger_instance.is_character() {
        return Err(Rejected::NotACharacter(challenger));
    }
    if has_restriction(state, registry, challenger, Restriction::CantChallenge) {
        return Err(Rejected::CharacterCannotChallenge(challenger));
    }
    if challenger_instance.conditions().drying
        && !has_permission(
            state,
            registry,
            challenger,
            Permission::ChallengeWhileDrying,
        )
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
        .map(PlayerState::id)
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
    // Abilities available on `card`: its printed activated abilities, then any
    // granted to it by an effect (§7.5). `ability_index` spans the combined list.
    let printed = registry
        .get(instance.definition())
        .ok_or(Rejected::UnknownCard(card))?
        .activated_abilities()
        .iter()
        .map(|a| (a.cost.ink, a.cost.exert_self, a.effect.clone()));
    let granted = state
        .granted_activated()
        .iter()
        .filter(|g| g.source == card)
        .map(|g| (g.ink, g.exert_self, g.effect.clone()));
    let (ink, exert_self, effect) = printed
        .chain(granted)
        .nth(ability_index)
        .ok_or(Rejected::NoSuchAbility(card))?;

    // Cost legality. Drying characters can't pay an exert cost (§4.2.2.1).
    if exert_self {
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
        < ink
    {
        return Err(Rejected::InsufficientInk(card));
    }

    // --- pay the cost ---
    {
        let p = state.player_mut(active).expect("active player exists");
        p.exert_ink(ink);
        if exert_self && let Some(c) = p.play_mut().iter_mut().find(|c| c.id() == card) {
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
    // "Until end of turn" effects end here (§7.6.1). (Done before the end-of-turn
    // triggers; ordering vs §4.4.1.3 is immaterial for the current effect set.)
    state.expire_end_of_turn_modifiers();
    // "At the end of your turn" triggers (§4.4.1.1) and any delayed triggers due
    // at end of turn (§7.4.7) go to the bag and resolve; this may suspend.
    enqueue_turn_triggers(state, registry, active, &TriggerCondition::AtEndOfTurn);
    for delayed in state.take_delayed_due(DelayedWhen::EndOfTurn) {
        let _ = state.enqueue_trigger(
            delayed.controller(),
            delayed.source(),
            false,
            delayed.effect(),
        );
    }
    events.extend(resolve_bag(state, registry));
    // If an end-of-turn trigger is awaiting a decision, ending the turn and
    // starting the next one resumes from `resume_turn_progression`.
    if state.is_awaiting_decision() || state.is_finished() {
        return Ok(events);
    }
    events.extend(continue_after_end_phase(state, registry));
    Ok(events)
}

/// Run a player's Beginning phase (Ready → Set → Draw) and stop in the Main
/// phase, the next point that needs input (§4.2). The very first turn of the
/// game skips the Draw step (§4.2.3.2).
fn begin_turn(state: &mut GameState, registry: &CardRegistry, first_turn: bool) -> Vec<GameEvent> {
    let active = state.active_player();
    state.set_inked_this_turn(false);
    state.clear_boosted_this_turn();

    let mut events = vec![GameEvent::TurnStarted {
        player: active,
        turn: state.turn_number(),
    }];

    // Ready step (§4.2.1).
    state.set_phase(Phase::Beginning);
    state.set_step(Step::Ready);
    events.push(GameEvent::StepEntered { step: Step::Ready });
    ready_all(state, registry, active);
    events.extend(game_state_check(state));
    if state.is_finished() {
        return events;
    }

    // Set step (§4.2.2): dry characters, gain location lore (§6.5.6), then resolve
    // "at the start of your turn" triggers (§4.2.2.3) — which may suspend.
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
    enqueue_turn_triggers(state, registry, active, &TriggerCondition::AtStartOfTurn);
    events.extend(resolve_bag(state, registry));
    // If a start-of-turn trigger is awaiting a decision, the Draw/Main steps
    // resume from `resume_turn_progression` once it's answered.
    if state.is_awaiting_decision() || state.is_finished() {
        return events;
    }
    events.extend(finish_beginning_phase(state, first_turn));
    events
}

/// The Draw step (§4.2.3) and the move into the Main phase (§4.3). Split out so it
/// can run inline in `begin_turn` or resume after a start-of-turn trigger suspends.
fn finish_beginning_phase(state: &mut GameState, first_turn: bool) -> Vec<GameEvent> {
    let active = state.active_player();
    let mut events = Vec::new();

    state.set_step(Step::Draw);
    events.push(GameEvent::StepEntered { step: Step::Draw });
    if !first_turn {
        events.push(draw(state, active));
    }
    events.extend(game_state_check(state));
    if state.is_finished() {
        return events;
    }

    state.set_phase(Phase::Main);
    state.set_step(Step::Main);
    events.push(GameEvent::StepEntered { step: Step::Main });
    events.extend(game_state_check(state));
    events
}

/// Enqueue "at the start/end of your turn" triggers on the active player's
/// in-play cards (§4.2.2.3 / §4.4.1.1).
fn enqueue_turn_triggers(
    state: &mut GameState,
    registry: &CardRegistry,
    active: PlayerId,
    condition: &TriggerCondition,
) {
    let cards: Vec<CardId> = state
        .player(active)
        .map(|p| p.play().iter().map(CardInstance::id).collect())
        .unwrap_or_default();
    for card in cards {
        enqueue_self_triggers(state, registry, active, card, condition);
    }
}

/// Continue the turn after the End-phase bag has drained: end the turn and start
/// the next player's turn (§4.4.1.4). Used inline by `apply_end_turn` and on
/// resume by `resume_turn_progression`.
fn continue_after_end_phase(state: &mut GameState, registry: &CardRegistry) -> Vec<GameEvent> {
    let active = state.active_player();
    let mut events = vec![GameEvent::TurnEnded { player: active }];
    events.extend(game_state_check(state));
    if state.is_finished() {
        return events;
    }
    let next = next_active_player(state, active);
    state.set_active_player(next);
    state.increment_turn_number();
    events.extend(begin_turn(state, registry, false));
    events
}

/// After a bag decision drains the bag, resume an in-progress turn transition
/// (§4.2.2.3 / §4.4.1) that suspended on a start/end-of-turn trigger. A no-op
/// during normal Main-phase play.
fn resume_turn_progression(state: &mut GameState, registry: &CardRegistry) -> Vec<GameEvent> {
    if state.is_awaiting_decision() || state.is_finished() {
        return Vec::new();
    }
    match (state.phase(), state.step()) {
        (Phase::End, _) => continue_after_end_phase(state, registry),
        (Phase::Beginning, Step::Set) => finish_beginning_phase(state, false),
        _ => Vec::new(),
    }
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
fn ready_all(state: &mut GameState, registry: &CardRegistry, player: PlayerId) {
    // A card with the "can't ready" restriction (freeze / continuous) stays
    // exerted this turn (§"can't ready"). Computed before the mutable pass.
    let frozen: std::collections::HashSet<CardId> = state
        .player(player)
        .map(|p| {
            p.play()
                .iter()
                .map(CardInstance::id)
                .filter(|&c| state.has_restriction(c, Restriction::CantReady))
                .collect()
        })
        .unwrap_or_default();
    if let Some(p) = state.player_mut(player) {
        // Collect cards to ready and their IDs first
        let play_cards_to_ready: Vec<_> = p
            .play_mut()
            .iter_mut()
            .filter(|c| !frozen.contains(&c.id()))
            .filter_map(|c| {
                let was_not_ready = !c.conditions().ready;
                c.conditions_mut().ready = true;
                if was_not_ready { Some(c.id()) } else { None }
            })
            .collect();

        let inkwell_cards_to_ready: Vec<_> = p
            .inkwell_mut()
            .iter_mut()
            .filter_map(|c| {
                let was_not_ready = !c.conditions().ready;
                c.conditions_mut().ready = true;
                if was_not_ready { Some(c.id()) } else { None }
            })
            .collect();

        // Now enqueue triggers (outside the mutable borrow)
        for card_id in play_cards_to_ready {
            enqueue_self_triggers(
                state,
                registry,
                player,
                card_id,
                &TriggerCondition::WhenThisReadies,
            );
        }
        for card_id in inkwell_cards_to_ready {
            enqueue_self_triggers(
                state,
                registry,
                player,
                card_id,
                &TriggerCondition::WhenThisReadies,
            );
        }
    }
    // One-shot freezes (UntilStep { Ready, player }) are consumed now; continuous
    // "can't ready" (e.g. while a source is in play) persists.
    state.expire_step_modifiers(Step::Ready, player);
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
            .is_some_and(PlayerState::is_eliminated)
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
    let mut matches: Vec<(bool, Effect)> = definition
        .abilities()
        .iter()
        .filter(|a| a.condition == *condition)
        .map(|a| (a.optional, a.effect.clone()))
        .collect();
    // Also fire any triggered abilities granted to this card by an effect (§7.6).
    matches.extend(
        state
            .granted_triggers()
            .iter()
            .filter(|g| g.source == source && g.condition == *condition)
            .map(|g| (g.optional, g.effect.clone())),
    );
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
        let mut modifier = StatModifier::new(
            card,
            target,
            ability.stat,
            ability.delta,
            ModifierDuration::WhileSourceInPlay,
        );
        if let Some(condition) = ability.condition {
            modifier = modifier.with_condition(condition);
        }
        if let Some(per) = &ability.per {
            modifier = modifier.with_count(per.clone());
        }
        state.add_modifier(modifier);
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
        .map(PlayerState::id)
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
#[allow(clippy::too_many_lines)] // one big Choice -> PendingDecision dispatch
fn resolve_effects(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    effects: Vec<Effect>,
    events: &mut Vec<GameEvent>,
) {
    let mut work: std::collections::VecDeque<Effect> = effects.into();
    while let Some(effect) = work.pop_front() {
        // Flatten a sequence into the work-list so its tail becomes the `rest`
        // continuation if an earlier element suspends on a choice (§7.1.2).
        if let Effect::All(seq) = effect {
            for e in seq.into_iter().rev() {
                work.push_front(e);
            }
            continue;
        }
        if let Some(choice) = execute_effect(state, registry, controller, source, &effect, events) {
            let rest: Vec<Effect> = work.into_iter().collect();
            state.set_pending(choice_to_pending(choice, source, rest));
            return;
        }
    }
}

/// A target choice an effect needs the controller to make at resolution.
enum Choice {
    /// `player` is asked whether to resolve `inner` ("you may …", §7.1.3).
    May { player: PlayerId, inner: Effect },
    /// The general choose primitive: pick `min..=max` of `options`, then run `then`.
    Choose {
        player: PlayerId,
        options: Vec<ChoiceRef>,
        min: u32,
        max: u32,
        then: ChoiceThen,
    },
    /// `player` names a card; the named card is then matched against the revealed
    /// top of their deck (§8.2).
    NameCard {
        player: PlayerId,
        lore_on_match: Amount,
        match_to: Destination,
        otherwise_to: Destination,
    },
    /// `player` names a card; all character cards with that name in their discard
    /// return to their hand (§8.2).
    NameThenRecur { player: PlayerId },
    /// `player` picks one of the offered effects to resolve (§7.1.9).
    ChooseOne {
        player: PlayerId,
        options: Vec<Effect>,
    },
}

/// Map a [`Choice`] to the [`PendingDecision`] that stashes it with its `source`
/// and continuation `rest` while awaiting the player's input.
fn choice_to_pending(choice: Choice, source: CardId, rest: Vec<Effect>) -> PendingDecision {
    match choice {
        Choice::May { player, inner } => PendingDecision::MayResolveEffect {
            player,
            source,
            effect: inner,
            rest,
        },
        Choice::Choose {
            player,
            options,
            min,
            max,
            then,
        } => PendingDecision::Choose {
            player,
            source,
            options,
            min,
            max,
            then,
            rest,
        },
        Choice::NameCard {
            player,
            lore_on_match,
            match_to,
            otherwise_to,
        } => PendingDecision::NameCard {
            player,
            source,
            lore_on_match,
            match_to,
            otherwise_to,
            rest,
        },
        Choice::NameThenRecur { player } => PendingDecision::NameThenRecur {
            player,
            source,
            rest,
        },
        Choice::ChooseOne { player, options } => PendingDecision::ChooseOne {
            player,
            source,
            options,
            rest,
        },
    }
}

/// Build a single-pick [`Choice::Choose`] that substitutes the chosen player into
/// `effect` and re-resolves it (a `Chosen*` player scope, §7.1).
fn choose_player(player: PlayerId, options: Vec<PlayerId>, effect: &Effect) -> Choice {
    Choice::Choose {
        player,
        options: options.into_iter().map(ChoiceRef::Player).collect(),
        min: 1,
        max: 1,
        then: ChoiceThen::SubstituteAndResolve(Box::new(effect.clone())),
    }
}

/// Build a single-pick [`Choice::Choose`] that substitutes the chosen card into
/// `effect` and re-resolves it (a move-damage endpoint, §9.3).
fn choose_card(player: PlayerId, options: Vec<CardId>, effect: &Effect) -> Choice {
    Choice::Choose {
        player,
        options: options.into_iter().map(ChoiceRef::Card).collect(),
        min: 1,
        max: 1,
        then: ChoiceThen::SubstituteAndResolve(Box::new(effect.clone())),
    }
}

/// Build a single-pick [`Choice::Choose`] that applies `effect` to the chosen
/// card ("chosen character …", §7.1).
fn choose_one(player: PlayerId, options: Vec<CardId>, effect: &Effect) -> Choice {
    Choice::Choose {
        player,
        options: options.into_iter().map(ChoiceRef::Card).collect(),
        min: 1,
        max: 1,
        then: ChoiceThen::ApplyToEach(Box::new(effect.clone())),
    }
}

/// Build an "up to `max`" [`Choice::Choose`] that applies `effect` to each picked
/// card (§7.1.8).
fn choose_up_to(player: PlayerId, options: Vec<CardId>, max: u32, effect: &Effect) -> Choice {
    Choice::Choose {
        player,
        options: options.into_iter().map(ChoiceRef::Card).collect(),
        min: 0,
        max,
        then: ChoiceThen::ApplyToEach(Box::new(effect.clone())),
    }
}

/// Build a single-pick [`Choice::Choose`] that plays the chosen card for free (§6).
fn choose_play_free(player: PlayerId, options: Vec<CardId>) -> Choice {
    Choice::Choose {
        player,
        options: options.into_iter().map(ChoiceRef::Card).collect(),
        min: 1,
        max: 1,
        then: ChoiceThen::PlayFree,
    }
}

/// Build an exactly-`count` [`Choice::Choose`] for `player` to discard from hand,
/// continuing the discard down `remaining` players afterwards (§8.4).
fn choose_discard(
    player: PlayerId,
    hand: Vec<CardId>,
    count: u32,
    amount: DiscardAmount,
    by: DiscardBy,
    remaining: Vec<PlayerId>,
) -> Choice {
    Choice::Choose {
        player,
        options: hand.into_iter().map(ChoiceRef::Card).collect(),
        min: count,
        max: count,
        then: ChoiceThen::Discard {
            amount,
            by,
            remaining_players: remaining,
        },
    }
}

/// Build a single-pick [`Choice::Choose`] for `chooser` to pick one of `owner`'s
/// hand cards (`options`) for `owner` to discard (Lenny / Timon / Goldie, §8.4).
fn choose_discard_from(chooser: PlayerId, owner: PlayerId, options: Vec<CardId>) -> Choice {
    Choice::Choose {
        player: chooser,
        options: options.into_iter().map(ChoiceRef::Card).collect(),
        min: 1,
        max: 1,
        then: ChoiceThen::DiscardFrom { owner },
    }
}

/// Build an "up to `take_count`" [`Choice::Choose`] that takes the picked looked-at
/// cards to hand and returns the rest to `deck_owner`'s deck (look-at-top, §8.2).
#[allow(clippy::too_many_arguments)]
fn choose_take_revealed(
    player: PlayerId,
    deck_owner: PlayerId,
    looked_at: Vec<CardId>,
    options: Vec<CardId>,
    rest_position: DeckPosition,
    take_count: u32,
    _reorder: bool,
    rest_per_card: Option<Vec<DeckPosition>>,
) -> Choice {
    let then = if let Some(destinations) = rest_per_card {
        ChoiceThen::TakeRevealedPerCard {
            deck_owner,
            looked_at,
            destinations,
        }
    } else {
        ChoiceThen::TakeRevealed {
            deck_owner,
            looked_at,
            rest_position,
        }
    };
    Choice::Choose {
        player,
        options: options.into_iter().map(ChoiceRef::Card).collect(),
        min: 0,
        max: take_count,
        then,
    }
}

/// Build the [`Choice::NameCard`] for a [`Effect::NameThenReveal`].
fn name_card_choice(player: PlayerId, effect: &Effect) -> Choice {
    let Effect::NameThenReveal {
        lore_on_match,
        match_to,
        otherwise_to,
    } = effect
    else {
        unreachable!("name_card_choice called with non-NameThenReveal effect");
    };
    Choice::NameCard {
        player,
        lore_on_match: lore_on_match.clone(),
        match_to: *match_to,
        otherwise_to: *otherwise_to,
    }
}

/// Apply a built-in effect for `controller`. Returns `Some(Choice)` when the
/// effect needs the controller to choose target(s) (nothing applied yet);
/// otherwise applies the effect and returns `None`.
#[allow(clippy::too_many_lines)] // one big per-effect dispatch
fn execute_effect(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) -> Option<Choice> {
    match effect {
        // Each player in `who` draws / changes lore (may prompt a player choice).
        Effect::Draw { who, amount } | Effect::Lore { who, amount } => {
            return resolve_player_draw_lore(
                state, controller, source, *who, amount, effect, events,
            );
        }
        // Players in `who` discard; each chooses their own cards unless the whole
        // hand goes (§8.4). A `Chosen*` scope may first prompt a player choice.
        Effect::Discard { who, amount, by } => {
            return resolve_discard_effect(state, controller, *who, *amount, *by, effect, events);
        }
        // The named players reveal their hand — an information event (§8.x).
        Effect::RevealHand { whose } => {
            return match resolve_scope(state, controller, *whose) {
                ScopeOutcome::Choose(options) => Some(choose_player(controller, options, effect)),
                ScopeOutcome::Players(players) => {
                    for p in players {
                        reveal_hand(state, p, events);
                    }
                    None
                }
            };
        }
        // A chosen opponent reveals their hand; the controller picks a matching
        // card for them to discard (§8.4). Resolve the opponent (prompting in
        // multiplayer), then choose from their hand.
        Effect::OpponentDiscardsChosen { whose, filter } => {
            return match resolve_scope(state, controller, *whose) {
                ScopeOutcome::Choose(options) => Some(choose_player(controller, options, effect)),
                ScopeOutcome::Players(players) => players.first().and_then(|&owner| {
                    let options = hand_matching(state, registry, controller, owner, filter);
                    (!options.is_empty()).then(|| choose_discard_from(controller, owner, options))
                }),
            };
        }
        // Move the top `count` of each scoped player's deck to `to` (mill / dig).
        Effect::Move {
            what: MoveSource::DeckTop { who, count },
            to,
        } => {
            return resolve_deck_move(
                state, registry, controller, source, *who, count, *to, effect,
            );
        }
        // The controller plays an eligible card from hand for free (§6).
        Effect::PlayFreeFromHand { filter } => {
            let options = eligible_free_plays(state, registry, controller, filter);
            return (!options.is_empty()).then_some(choose_play_free(controller, options));
        }
        // Look at the top N; may take up to `take_count` matching `filter`, rest go to `rest` (§8.2).
        Effect::LookAtTopAndTake {
            whose,
            count,
            take_count,
            filter,
            rest,
            reorder,
            rest_per_card,
        } => {
            return resolve_look_at_top(
                state,
                registry,
                controller,
                *whose,
                *count,
                *take_count,
                filter,
                *rest,
                *reorder,
                rest_per_card.clone(),
                effect,
            );
        }
        // Search deck for up to `take_count` matching cards, take to hand, then shuffle.
        Effect::SearchDeckAndTake {
            whose,
            take_count,
            filter,
        } => {
            return resolve_search_deck(
                state,
                registry,
                controller,
                *whose,
                *take_count,
                filter,
                effect,
            );
        }
        // "Name a card, then reveal the top of your deck" — ask for the name (§8.2).
        Effect::NameThenReveal { .. } => return Some(name_card_choice(controller, effect)),
        // "Name a card, then recur all matching characters from your discard" (§8.2).
        Effect::NameThenRecur => return Some(Choice::NameThenRecur { player: controller }),
        // "You may …": ask the controller; resolve `inner` only on yes (§7.1.3).
        Effect::May(inner) => {
            return Some(Choice::May {
                player: controller,
                inner: (**inner).clone(),
            });
        }
        // A sequence reached as a nested branch (e.g. an `IfControl` `then`):
        // resolve in order. (Top-level sequences are flattened by `resolve_effects`,
        // which also handles suspension; a mid-sequence choice in a *nested* branch
        // is a known limitation — add when a card needs it.)
        Effect::All(effects) => {
            for e in effects {
                if let Some(choice) = execute_effect(state, registry, controller, source, e, events)
                {
                    return Some(choice);
                }
            }
        }
        // Schedule a one-shot delayed trigger to fire later (§7.4.7).
        Effect::ScheduleDelayed {
            when,
            effect: inner,
        } => {
            state.schedule_delayed(DelayedTrigger::new(
                controller,
                source,
                *when,
                (**inner).clone(),
            ));
        }
        // Conditional: resolve `then` only if the controller has a matching
        // in-play character. `then` may itself be targeted (delegated).
        Effect::IfControl {
            filter,
            at_least,
            then,
        } => {
            let count = matching_characters(state, controller, source, filter).len();
            if count >= *at_least as usize {
                return execute_effect(state, registry, controller, source, then, events);
            }
        }
        Effect::IfCount { condition, then } => {
            if count_condition_holds(state, controller, *condition) {
                return execute_effect(state, registry, controller, source, then, events);
            }
        }
        // Boost: put top cards of deck under this character, facedown (§10.4).
        Effect::Boost { count } => {
            let count = state.eval_amount(controller, source, source, count);
            if count > 0 {
                let deck_len = state.player(controller).map_or(0, |p| p.deck().len());
                let actual_count = count.min(i32::try_from(deck_len).unwrap_or(i32::MAX));
                if actual_count > 0 {
                    let mut cards_to_add = Vec::new();
                    {
                        let player = state.player_mut(controller).unwrap();
                        let player_deck = player.deck_mut();
                        for _ in 0..actual_count {
                            if let Some(mut card) = player_deck.pop_top() {
                                card.conditions_mut().facedown = true;
                                cards_to_add.push(card);
                            }
                        }
                    }
                    if let Some(character) = state
                        .player_mut(controller)
                        .and_then(|p| p.play_mut().iter_mut().find(|c| c.id() == source))
                    {
                        for card in cards_to_add {
                            character.push_under(card);
                        }
                        events.push(GameEvent::Boosted {
                            player: controller,
                            card: source,
                        });
                    }
                }
            }
        }
        // Modal "choose one": present the options to the controller (§7.1.9).
        Effect::ChooseOne { options, optional } => {
            if *optional {
                return Some(Choice::May {
                    player: controller,
                    inner: Effect::ChooseOne {
                        options: options.clone(),
                        optional: false,
                    },
                });
            }
            return Some(Choice::ChooseOne {
                player: controller,
                options: options.clone(),
            });
        }
        // Move damage between two characters (§9.3): resolve endpoints one at a
        // time (each pick excludes the already-fixed endpoint), then move.
        Effect::MoveDamage { from, to, amount } => {
            return resolve_move_damage(
                state, registry, controller, source, from, to, amount, effect,
            );
        }
        // Targeted effects: resolve the target now (self / all) or report that a
        // choice is needed (§7.1).
        Effect::Move {
            what: MoveSource::Card(target),
            ..
        }
        | Effect::GiveStrengthThisTurn { target, .. }
        | Effect::DealDamage { target, .. }
        | Effect::RemoveDamage { target, .. }
        | Effect::Banish(target)
        | Effect::Exert(target)
        | Effect::Ready(target)
        | Effect::Freeze(target)
        | Effect::GrantAbilityThisTurn { target, .. }
        | Effect::GrantActivatedThisTurn { target, .. }
        | Effect::GrantThisTurn { target, .. }
        | Effect::Grant { target, .. }
        | Effect::GrantNextTurn { target, .. }
        | Effect::IfTargetMatches { target, .. } => {
            return resolve_targeted(state, registry, controller, source, target, effect, events);
        }
    }
    None
}

/// Resolve a targeted effect's target: apply it now for `SelfCard` / `AllCharacters`,
/// or report the [`Choice`] the controller must make (§7.1, §7.1.8).
fn resolve_targeted(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    target: &Target,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) -> Option<Choice> {
    match target {
        Target::SelfCard => {
            apply_effect_to(state, registry, controller, source, source, effect, events);
            None
        }
        // An already-resolved card (e.g. a prior pick of a multi-target effect).
        Target::Card(card) => {
            apply_effect_to(state, registry, controller, source, *card, effect, events);
            None
        }
        Target::ChosenCharacter { filter } => {
            let options = choosable_characters(state, registry, controller, source, filter);
            (!options.is_empty()).then(|| choose_one(controller, options, effect))
        }
        Target::AllCharacters { filter } => {
            // "All characters" affects every match — it does not *choose*, so Ward
            // does not apply (§10.15); use the raw matching set.
            let targets = matching_characters(state, controller, source, filter);
            for card in targets {
                apply_effect_to(state, registry, controller, source, card, effect, events);
            }
            None
        }
        Target::UpToCharacters { filter, max } => {
            let options = choosable_characters(state, registry, controller, source, filter);
            (!options.is_empty()).then(|| choose_up_to(controller, options, *max, effect))
        }
        Target::ChosenPermanent { filter } => {
            let options = choosable_permanents(state, registry, controller, source, filter);
            (!options.is_empty()).then(|| choose_one(controller, options, effect))
        }
    }
}

/// The in-play **permanents** (characters / items / locations) `controller` may
/// choose for a [`Target::ChosenPermanent`]: every in-play card matching the
/// filter algebra, minus those an opponent can't choose (Ward, §10.15). Unlike
/// [`choosable_characters`] there is no character gate — the filter's `Category`
/// predicate decides the kind.
fn choosable_permanents(
    state: &GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    filter: &CharacterFilter,
) -> Vec<CardId> {
    let mut out = Vec::new();
    for player in state.players() {
        let owner = player.id();
        for card in player.play().iter() {
            if state.matches_filter(controller, source, owner, card, filter) {
                out.push(card.id());
            }
        }
    }
    out.into_iter()
        .filter(|&card| {
            state.card_owner_in_play(card) == Some(controller)
                || !has_restriction(state, registry, card, Restriction::CantBeChosen)
        })
        .collect()
}

/// Map a move [`Destination`] to the internal self-move destination.
const fn destination_to_self(to: Destination) -> SelfDestination {
    match to {
        Destination::Hand => SelfDestination::Hand,
        Destination::Inkwell => SelfDestination::Inkwell,
        Destination::Discard => SelfDestination::Discard,
        Destination::Deck(DeckPosition::Top) => SelfDestination::TopOfDeck,
        Destination::Deck(DeckPosition::Bottom) => SelfDestination::BottomOfDeck,
        Destination::Deck(DeckPosition::Shuffle) => SelfDestination::ShuffleIntoDeck,
    }
}

/// Freeze `card`: it can't ready at its controller's next ready step. The
/// `CantReady` modifier is sourced to the card itself (so it survives the freezer
/// leaving play) and expires when that controller next readies (§"can't ready").
fn freeze_card(state: &mut GameState, card: CardId) {
    if let Some(owner) = state.card_owner_in_play(card) {
        state.add_property_modifier(PropertyModifier::new(
            card,
            ModifierTarget::Card(card),
            Property::Restriction(Restriction::CantReady),
            ModifierDuration::UntilStep {
                step: Step::Ready,
                player: owner,
            },
        ));
    }
}

/// Add a continuous [`Property`] (keyword / restriction / permission) to a target
/// for the rest of the turn (§7.6.1).
fn grant_property(state: &mut GameState, source: CardId, target: CardId, property: Property) {
    state.add_property_modifier(PropertyModifier::new(
        source,
        ModifierTarget::Card(target),
        property,
        ModifierDuration::UntilEndOfTurn,
    ));
}

/// Resolve a player-scoped draw/lore: apply to each player in scope, or prompt a
/// choose-a-player decision for a `Chosen*` scope with 2+ candidates.
#[allow(clippy::too_many_arguments)]
fn resolve_player_draw_lore(
    state: &mut GameState,
    controller: PlayerId,
    source: CardId,
    who: PlayerScope,
    amount: &Amount,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) -> Option<Choice> {
    let n = state.eval_amount(controller, source, source, amount);
    match resolve_scope(state, controller, who) {
        ScopeOutcome::Players(players) => {
            for p in players {
                apply_player_amount(state, p, effect, n, events);
            }
            None
        }
        ScopeOutcome::Choose(options) => Some(choose_player(controller, options, effect)),
    }
}

/// Apply a pre-evaluated draw/lore `amount` to a single `player`. Draw uses the
/// count (clamped ≥0); `Lore` adds when positive, loses when negative.
fn apply_player_amount(
    state: &mut GameState,
    player: PlayerId,
    effect: &Effect,
    amount: i32,
    events: &mut Vec<GameEvent>,
) {
    match effect {
        Effect::Draw { .. } => {
            for _ in 0..amount.max(0) {
                events.push(draw(state, player));
            }
        }
        Effect::Lore { .. } if amount >= 0 => {
            let gained = u32::try_from(amount).unwrap_or(0);
            if let Some(p) = state.player_mut(player) {
                p.add_lore(gained);
            }
            events.push(GameEvent::LoreGained {
                player,
                amount: gained,
            });
        }
        Effect::Lore { .. } => {
            let lost = u32::try_from(-amount).unwrap_or(0);
            if let Some(p) = state.player_mut(player) {
                p.lose_lore(lost);
            }
            events.push(GameEvent::LoreLost {
                player,
                amount: lost,
            });
        }
        _ => {}
    }
}

/// The outcome of resolving a [`PlayerScope`]: either the concrete players it
/// applies to, or (for a `Chosen*` scope with 2+ candidates) the candidates the
/// controller must choose one from.
enum ScopeOutcome {
    Players(Vec<PlayerId>),
    Choose(Vec<PlayerId>),
}

/// Resolve a [`PlayerScope`] from `controller`'s perspective. `Chosen*` scopes
/// auto-resolve when there's a single candidate (e.g. a "chosen opponent" in a
/// 2-player game) and otherwise require a choice (3–4 player games).
fn resolve_scope(state: &GameState, controller: PlayerId, who: PlayerScope) -> ScopeOutcome {
    let all: Vec<PlayerId> = state.players().iter().map(PlayerState::id).collect();
    let opponents =
        || -> Vec<PlayerId> { all.iter().copied().filter(|p| *p != controller).collect() };
    match who {
        PlayerScope::You => ScopeOutcome::Players(vec![controller]),
        PlayerScope::EachOpponent => ScopeOutcome::Players(opponents()),
        PlayerScope::EachPlayer => {
            ScopeOutcome::Players(std::iter::once(controller).chain(opponents()).collect())
        }
        PlayerScope::Player(p) => ScopeOutcome::Players(vec![p]),
        PlayerScope::ChosenOpponent => match opponents() {
            o if o.len() <= 1 => ScopeOutcome::Players(o),
            o => ScopeOutcome::Choose(o),
        },
        PlayerScope::ChosenPlayer if all.len() <= 1 => ScopeOutcome::Players(all),
        PlayerScope::ChosenPlayer => ScopeOutcome::Choose(all),
    }
}

/// Re-target a player-scoped effect onto a now-resolved single player (after a
/// `ChoosePlayer` decision). Effects without a player scope are unchanged.
fn substitute_chosen_player(effect: &Effect, player: PlayerId) -> Effect {
    match effect {
        Effect::Draw { amount, .. } => Effect::Draw {
            who: PlayerScope::Player(player),
            amount: amount.clone(),
        },
        Effect::Lore { amount, .. } => Effect::Lore {
            who: PlayerScope::Player(player),
            amount: amount.clone(),
        },
        Effect::Discard { amount, by, .. } => Effect::Discard {
            who: PlayerScope::Player(player),
            amount: *amount,
            by: *by,
        },
        Effect::RevealHand { .. } => Effect::RevealHand {
            whose: PlayerScope::Player(player),
        },
        Effect::OpponentDiscardsChosen { filter, .. } => Effect::OpponentDiscardsChosen {
            whose: PlayerScope::Player(player),
            filter: filter.clone(),
        },
        Effect::LookAtTopAndTake {
            count,
            take_count,
            filter,
            rest,
            reorder,
            rest_per_card,
            ..
        } => Effect::LookAtTopAndTake {
            whose: PlayerScope::Player(player),
            count: *count,
            take_count: *take_count,
            filter: filter.clone(),
            rest: *rest,
            reorder: *reorder,
            rest_per_card: rest_per_card.clone(),
        },
        Effect::SearchDeckAndTake {
            take_count, filter, ..
        } => Effect::SearchDeckAndTake {
            whose: PlayerScope::Player(player),
            take_count: *take_count,
            filter: filter.clone(),
        },
        Effect::ChooseOne { options, optional } => Effect::ChooseOne {
            options: options.clone(),
            optional: *optional,
        },
        Effect::Move {
            what: MoveSource::DeckTop { count, .. },
            to,
        } => Effect::Move {
            what: MoveSource::DeckTop {
                who: PlayerScope::Player(player),
                count: count.clone(),
            },
            to: *to,
        },
        other => other.clone(),
    }
}

/// Resolve a [`MoveSource::DeckTop`] move: each scoped player moves the top
/// `count` cards of their deck to `to` (mill → discard, dig → hand, …), or a
/// `Chosen*` scope prompts a player choice.
#[allow(clippy::too_many_arguments)]
fn resolve_deck_move(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    who: PlayerScope,
    count: &Amount,
    to: Destination,
    effect: &Effect,
) -> Option<Choice> {
    let n =
        usize::try_from(state.eval_amount(controller, source, source, count).max(0)).unwrap_or(0);
    match resolve_scope(state, controller, who) {
        ScopeOutcome::Players(players) => {
            let mut all_cards_to_inkwell: Vec<(PlayerId, CardId)> = Vec::new();
            for p in players {
                let cards = move_deck_top(state, p, n, to);
                for card_id in cards {
                    all_cards_to_inkwell.push((p, card_id));
                }
            }
            // Enqueue inkwell triggers for cards moved to inkwell
            for (player, _card_id) in all_cards_to_inkwell {
                enqueue_turn_triggers(
                    state,
                    registry,
                    player,
                    &TriggerCondition::WhenCardPutInInkwell,
                );
            }
            None
        }
        ScopeOutcome::Choose(options) => Some(choose_player(controller, options, effect)),
    }
}

/// Move the top `n` cards of `player`'s deck to `to` (§8).
/// Returns the IDs of cards moved to inkwell (for trigger enqueueing).
fn move_deck_top(
    state: &mut GameState,
    player: PlayerId,
    n: usize,
    to: Destination,
) -> Vec<CardId> {
    let dest = destination_to_self(to);
    let mut cards_moved_to_inkwell = Vec::new();
    for _ in 0..n {
        let Some(p) = state.player_mut(player) else {
            return cards_moved_to_inkwell;
        };
        let Some(mut card) = p.deck_mut().pop_top() else {
            return cards_moved_to_inkwell;
        };
        match dest {
            SelfDestination::Discard => {
                *card.conditions_mut() = Conditions::faceup_idle();
                p.discard_mut().push(card);
            }
            SelfDestination::Hand => {
                *card.conditions_mut() = Conditions::faceup_idle();
                p.hand_mut().push(card);
            }
            SelfDestination::Inkwell => {
                *card.conditions_mut() = Conditions::in_inkwell();
                cards_moved_to_inkwell.push(card.id());
                p.inkwell_mut().push(card);
            }
            SelfDestination::BottomOfDeck => {
                p.deck_mut().insert_bottom(card);
            }
            SelfDestination::TopOfDeck | SelfDestination::ShuffleIntoDeck => {
                p.deck_mut().push(card);
            }
        }
    }
    cards_moved_to_inkwell
}

/// Resolve a discard across `players` in order: discard the whole hand outright
/// when `amount` covers it, else ask the first such player to choose (carrying the
/// remaining players to discard afterwards), §8.4.
fn resolve_scope_discard(
    state: &mut GameState,
    players: &[PlayerId],
    amount: DiscardAmount,
    by: DiscardBy,
    events: &mut Vec<GameEvent>,
) -> Option<Choice> {
    for (i, &player) in players.iter().enumerate() {
        let mut hand: Vec<CardId> = state
            .player(player)
            .map(|p| p.hand().iter().map(CardInstance::id).collect())
            .unwrap_or_default();
        let count = match amount {
            DiscardAmount::WholeHand => u32::try_from(hand.len()).unwrap_or(u32::MAX),
            DiscardAmount::Count(n) => n,
        };
        if count as usize >= hand.len() {
            // The whole hand goes regardless of how it's selected.
            for card in hand {
                discard_card(state, player, card, events);
            }
        } else {
            match by {
                // Random: discard `count` uniformly-random cards (no choice, §8.4).
                DiscardBy::Random => {
                    for _ in 0..count {
                        let idx = state.rng_mut().below(hand.len());
                        discard_card(state, player, hand.remove(idx), events);
                    }
                }
                // Owner chooses: suspend on a discard pick (carrying the rest).
                DiscardBy::Owner => {
                    return Some(choose_discard(
                        player,
                        hand,
                        count,
                        amount,
                        by,
                        players[i + 1..].to_vec(),
                    ));
                }
            }
        }
    }
    None
}

/// The cards in `owner`'s hand matching `filter`, evaluated against each card's
/// definition from `controller`'s perspective (§6, §8.4).
fn hand_matching(
    state: &GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    owner: PlayerId,
    filter: &CharacterFilter,
) -> Vec<CardId> {
    state
        .player(owner)
        .map(|p| {
            p.hand()
                .iter()
                .filter(|c| {
                    registry
                        .get(c.definition())
                        .is_some_and(|d| def_matches_filter(controller, owner, d, filter))
                })
                .map(CardInstance::id)
                .collect()
        })
        .unwrap_or_default()
}

/// The cards in `player`'s hand that a "play for free" effect with `filter` may
/// play (§6).
fn eligible_free_plays(
    state: &GameState,
    registry: &CardRegistry,
    player: PlayerId,
    filter: &CharacterFilter,
) -> Vec<CardId> {
    hand_matching(state, registry, player, player, filter)
}

/// Resolve "look at the top N": offer up to `take_count` matching cards to take
/// into hand, or (if none match) send the looked-at cards to `rest` immediately (§8.2).
#[allow(clippy::too_many_arguments)]
fn resolve_look_at_top(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    whose: PlayerScope,
    count: u32,
    take_count: u32,
    filter: &CharacterFilter,
    rest: DeckPosition,
    reorder: bool,
    rest_per_card: Option<Vec<DeckPosition>>,
    effect: &Effect,
) -> Option<Choice> {
    // The deck looked at: resolve the scope to a single owner (prompt if needed).
    let owner = match resolve_scope(state, controller, whose) {
        ScopeOutcome::Players(players) => match players.first() {
            Some(p) => *p,
            None => return None,
        },
        ScopeOutcome::Choose(options) => {
            return Some(choose_player(controller, options, effect));
        }
    };
    let looked_at = peek_top(state, owner, count);
    let options: Vec<CardId> = looked_at
        .iter()
        .copied()
        .filter(|&id| {
            deck_card_def(state, owner, id)
                .and_then(|d| registry.get(d))
                .is_some_and(|def| def_matches_filter(controller, owner, def, filter))
        })
        .collect();
    if options.is_empty() {
        // Use per-card destinations if specified, otherwise use the single rest position
        if let Some(destinations) = rest_per_card {
            place_revealed_rest_per_card(state, owner, &looked_at, &destinations);
        } else {
            place_revealed_rest(state, owner, &looked_at, rest);
        }
        None
    } else {
        // `controller` chooses and receives; `owner`'s deck holds the rest.
        Some(choose_take_revealed(
            controller,
            owner,
            looked_at,
            options,
            rest,
            take_count,
            reorder,
            rest_per_card,
        ))
    }
}

/// Resolve "search deck": find all cards matching `filter` in `whose` deck, offer up
/// to `take_count` to take into hand, then shuffle the deck (§8.2).
#[allow(clippy::too_many_arguments)]
fn resolve_search_deck(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    whose: PlayerScope,
    take_count: u32,
    filter: &CharacterFilter,
    effect: &Effect,
) -> Option<Choice> {
    // The deck searched: resolve the scope to a single owner (prompt if needed).
    let owner = match resolve_scope(state, controller, whose) {
        ScopeOutcome::Players(players) => match players.first() {
            Some(p) => *p,
            None => return None,
        },
        ScopeOutcome::Choose(options) => {
            return Some(choose_player(controller, options, effect));
        }
    };
    // Find all cards in the deck matching the filter.
    let options: Vec<CardId> = state
        .player(owner)
        .map(|p| p.deck().iter().map(CardInstance::id).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|&id| {
            deck_card_def(state, owner, id)
                .and_then(|d| registry.get(d))
                .is_some_and(|def| def_matches_filter(controller, owner, def, filter))
        })
        .collect();
    if options.is_empty() {
        // No matches: shuffle the deck and continue.
        state.shuffle_deck(owner);
        None
    } else {
        // Offer the player to choose up to `take_count` cards.
        Some(Choice::Choose {
            player: controller,
            options: options.into_iter().map(ChoiceRef::Card).collect(),
            min: 0,
            max: take_count,
            then: ChoiceThen::SearchDeckTake { deck_owner: owner },
        })
    }
}

/// Evaluate whether a count-based condition holds for the controller.
fn count_condition_holds(
    state: &GameState,
    controller: PlayerId,
    condition: CountCondition,
) -> bool {
    use crate::domain::effects::CountCondition;
    let opponents = || -> Vec<PlayerId> {
        state
            .players()
            .iter()
            .map(PlayerState::id)
            .filter(|p| *p != controller)
            .collect()
    };
    match condition {
        CountCondition::HandSizeAtLeast(n) => state
            .player(controller)
            .is_some_and(|p| u32::try_from(p.hand().len()).unwrap_or(0) >= n),
        CountCondition::HandSizeMoreThan(n) => state
            .player(controller)
            .is_some_and(|p| u32::try_from(p.hand().len()).unwrap_or(0) > n),
        CountCondition::LoreAtLeast(n) => state.player(controller).is_some_and(|p| p.lore() >= n),
        CountCondition::LoreMoreThan(n) => state.player(controller).is_some_and(|p| p.lore() > n),
        CountCondition::LoreMoreThanOpponent => {
            let my_lore = state.player(controller).map_or(0, PlayerState::lore);
            let opp_lore = opponents()
                .first()
                .and_then(|p| state.player(*p).map(PlayerState::lore))
                .unwrap_or(0);
            my_lore > opp_lore
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::effects::CountCondition;

    #[test]
    fn test_count_condition_hands() {
        let mut state = GameState::new(
            vec![
                (0..30).map(CardDefId::from_raw).collect(),
                (0..30).map(CardDefId::from_raw).collect(),
            ],
            7,
        );

        let player = PlayerId::from_index(0);

        // Add cards to hand
        for i in 0..5 {
            let card = CardInstance::new(
                CardId::from_raw(100 + i),
                CardDefId::from_raw(999),
                Conditions::in_deck(),
            );
            state.player_mut(player).unwrap().hand_mut().push(card);
        }

        assert!(count_condition_holds(
            &state,
            player,
            CountCondition::HandSizeAtLeast(3)
        ));
        assert!(count_condition_holds(
            &state,
            player,
            CountCondition::HandSizeMoreThan(2)
        ));
        assert!(!count_condition_holds(
            &state,
            player,
            CountCondition::HandSizeMoreThan(10)
        ));
    }

    #[test]
    fn test_count_condition_lore() {
        let state = GameState::new(
            vec![
                (0..30).map(CardDefId::from_raw).collect(),
                (0..30).map(CardDefId::from_raw).collect(),
            ],
            7,
        );

        let player = PlayerId::from_index(0);

        assert!(count_condition_holds(
            &state,
            player,
            CountCondition::LoreAtLeast(0)
        ));
        assert!(!count_condition_holds(
            &state,
            player,
            CountCondition::LoreAtLeast(1)
        ));
        assert!(!count_condition_holds(
            &state,
            player,
            CountCondition::LoreMoreThan(0)
        ));
    }

    #[test]
    fn test_count_condition_lore_vs_opponent() {
        let mut state = GameState::new(
            vec![
                (0..30).map(CardDefId::from_raw).collect(),
                (0..30).map(CardDefId::from_raw).collect(),
            ],
            7,
        );

        let player1 = PlayerId::from_index(0);
        let player2 = PlayerId::from_index(1);

        // Player 1 has more lore
        state.player_mut(player1).unwrap().add_lore(5);
        state.player_mut(player2).unwrap().add_lore(2);

        assert!(count_condition_holds(
            &state,
            player1,
            CountCondition::LoreMoreThanOpponent
        ));
        assert!(!count_condition_holds(
            &state,
            player2,
            CountCondition::LoreMoreThanOpponent
        ));

        // Equal lore
        state.player_mut(player2).unwrap().add_lore(3);
        assert!(!count_condition_holds(
            &state,
            player1,
            CountCondition::LoreMoreThanOpponent
        ));
    }

    #[test]
    fn test_count_condition_boundary_values() {
        let mut state = GameState::new(
            vec![
                (0..30).map(CardDefId::from_raw).collect(),
                (0..30).map(CardDefId::from_raw).collect(),
            ],
            7,
        );

        let player = PlayerId::from_index(0);

        // Exactly at threshold
        for i in 0..3 {
            let card = CardInstance::new(
                CardId::from_raw(100 + i),
                CardDefId::from_raw(999),
                Conditions::in_deck(),
            );
            state.player_mut(player).unwrap().hand_mut().push(card);
        }

        assert!(count_condition_holds(
            &state,
            player,
            CountCondition::HandSizeAtLeast(3)
        ));
        assert!(!count_condition_holds(
            &state,
            player,
            CountCondition::HandSizeMoreThan(3)
        ));

        // One above threshold
        let card = CardInstance::new(
            CardId::from_raw(103),
            CardDefId::from_raw(999),
            Conditions::in_deck(),
        );
        state.player_mut(player).unwrap().hand_mut().push(card);

        assert!(count_condition_holds(
            &state,
            player,
            CountCondition::HandSizeMoreThan(3)
        ));
    }
}

/// Apply an amount-bearing targeted effect (give `{S}` / deal / remove damage) to
/// `target_card`, evaluating its [`Amount`] at resolution.
fn apply_amount_effect(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    target_card: CardId,
    effect: &Effect,
    amount: &Amount,
) {
    let value = state.eval_amount(controller, source, target_card, amount);
    match effect {
        Effect::GiveStrengthThisTurn { .. } => {
            state.add_modifier(StatModifier::new(
                source,
                ModifierTarget::Card(target_card),
                Stat::Strength,
                value,
                ModifierDuration::UntilEndOfTurn,
            ));
        }
        Effect::DealDamage { .. } => {
            if let Some(owner) = owner_holding(state, target_card) {
                add_damage(
                    state,
                    owner,
                    target_card,
                    u32::try_from(value.max(0)).unwrap_or(0),
                );
            }
        }
        Effect::RemoveDamage { .. } => {
            let remove = u32::try_from(value.max(0)).unwrap_or(0);
            if remove > 0
                && let Some(owner) = owner_holding(state, target_card)
                && let Some(c) = state
                    .player_mut(owner)
                    .and_then(|p| p.play_mut().iter_mut().find(|c| c.id() == target_card))
            {
                let conditions = c.conditions_mut();
                conditions.damage = conditions.damage.saturating_sub(remove);
                // Enqueue "whenever you remove damage from this character" triggers
                enqueue_self_triggers(
                    state,
                    registry,
                    owner,
                    target_card,
                    &TriggerCondition::WhenDamageRemovedFromThis,
                );
            }
        }
        _ => {}
    }
}

/// Evaluate the [`CharacterFilter`] algebra against a card **definition** (a card
/// in hand/deck, from `controller`'s view with the card owned by `owner`). Only
/// the printed predicates apply; instance-only ones (damage/exert/`{S}`/source/
/// specific-card) are false for a zoned card (§6).
fn def_matches_filter(
    controller: PlayerId,
    owner: PlayerId,
    def: &CardDefinition,
    filter: &CharacterFilter,
) -> bool {
    let recurse = |f: &CharacterFilter| def_matches_filter(controller, owner, def, f);
    match filter {
        CharacterFilter::Any | CharacterFilter::Side(TargetSide::Any) => true,
        CharacterFilter::Side(TargetSide::Yours) => owner == controller,
        CharacterFilter::Side(TargetSide::Opposing) => owner != controller,
        CharacterFilter::Classification(c) => def.has_classification(c),
        CharacterFilter::Category(cat) => category_matches(cat, def),
        CharacterFilter::Named(n) => def.has_name(n),
        CharacterFilter::Cost(nf) => nf.matches(def.cost()),
        // Willpower/Lore use the *printed* stat for a zoned card (§6); a card with
        // no such stat (action/item) never matches the threshold.
        CharacterFilter::Willpower(nf) => def.printed_willpower().is_some_and(|w| nf.matches(w)),
        CharacterFilter::Lore(nf) => def.printed_lore().is_some_and(|l| nf.matches(l)),
        CharacterFilter::Strength(_)
        | CharacterFilter::Damaged(_)
        | CharacterFilter::Exerted(_)
        | CharacterFilter::IsSource
        | CharacterFilter::IsCard(_) => false,
        CharacterFilter::And(fs) => fs.iter().all(recurse),
        CharacterFilter::Or(fs) => fs.iter().any(recurse),
        CharacterFilter::Not(f) => !recurse(f),
    }
}

/// The top `count` cards of `player`'s deck, in deck order (bottom-to-top of the
/// slice; the very top is last).
fn peek_top(state: &GameState, player: PlayerId, count: u32) -> Vec<CardId> {
    state
        .player(player)
        .map(|p| {
            let deck = p.deck();
            let skip = deck.len().saturating_sub(count as usize);
            deck.iter().skip(skip).map(CardInstance::id).collect()
        })
        .unwrap_or_default()
}

/// The definition of a card currently in `player`'s deck, if present.
fn deck_card_def(state: &GameState, player: PlayerId, card: CardId) -> Option<CardDefId> {
    state
        .player(player)?
        .deck()
        .iter()
        .find(|c| c.id() == card)
        .map(CardInstance::definition)
}

/// Move looked-at cards that weren't taken to the top/bottom of `player`'s deck
/// (shuffling in if `DeckPosition::Shuffle`), §8.2.
fn place_revealed_rest(
    state: &mut GameState,
    player: PlayerId,
    ids: &[CardId],
    position: DeckPosition,
) {
    for &id in ids {
        if let Some(p) = state.player_mut(player)
            && let Some(instance) = p.deck_mut().take(id)
        {
            match position {
                DeckPosition::Bottom => p.deck_mut().insert_bottom(instance),
                DeckPosition::Top | DeckPosition::Shuffle => p.deck_mut().push(instance),
            }
        }
    }
    if matches!(position, DeckPosition::Shuffle) {
        state.shuffle_deck(player);
    }
}

/// Place revealed cards back into the deck according to per-card destinations
/// (for split top/bottom effects like Dr. Facilier).
fn place_revealed_rest_per_card(
    state: &mut GameState,
    player: PlayerId,
    ids: &[CardId],
    destinations: &[DeckPosition],
) {
    for (&id, &dest) in ids.iter().zip(destinations.iter()) {
        if let Some(p) = state.player_mut(player)
            && let Some(instance) = p.deck_mut().take(id)
        {
            match dest {
                DeckPosition::Bottom => p.deck_mut().insert_bottom(instance),
                DeckPosition::Top => p.deck_mut().push(instance),
                DeckPosition::Shuffle => {
                    p.deck_mut().push(instance);
                    state.shuffle_deck(player);
                }
            }
        }
    }
}

/// Play `card` from `player`'s hand **for free** (no ink, §6). Actions resolve
/// their effect and go to the discard; permanents enter play (ready) with their
/// statics and enters-play triggers. (A free-played Bodyguard enters ready — the
/// optional enter-exerted prompt is skipped, which is a legal choice.)
fn play_card_free(
    state: &mut GameState,
    registry: &CardRegistry,
    player: PlayerId,
    card: CardId,
    events: &mut Vec<GameEvent>,
) {
    let Some(def_id) = state
        .player(player)
        .and_then(|p| p.hand().iter().find(|c| c.id() == card))
        .map(CardInstance::definition)
    else {
        return;
    };
    let Some(definition) = registry.get(def_id) else {
        return;
    };
    if matches!(definition.kind(), CardKind::Action) {
        let effects = definition.action_effects().to_vec();
        events.extend(resolve_action_play(
            state, registry, player, card, def_id, effects,
        ));
        return;
    }
    let statics = definition.static_abilities().to_vec();
    let rule_statics = definition.rule_statics().to_vec();
    place_permanent_free(state, player, card, definition);
    apply_enter_statics(state, player, card, &statics);
    apply_enter_rule_statics(state, player, card, &rule_statics);
    events.push(GameEvent::CardPlayed { player, card });
    enqueue_enter_play_triggers(state, registry, player, card, def_id, false);
}

/// Put a permanent into play from `player`'s hand without paying its cost
/// (helper for [`play_card_free`]; Shift is never used here).
fn place_permanent_free(
    state: &mut GameState,
    player: PlayerId,
    card: CardId,
    definition: &CardDefinition,
) {
    let (conditions, char_stats, loc_stats) = match definition.kind() {
        CardKind::Character {
            strength,
            willpower,
            lore,
        } => (
            Conditions::entering_play(),
            Some(CharacterStats::new(strength, willpower, lore)),
            None,
        ),
        CardKind::Location {
            move_cost,
            willpower,
            lore,
        } => (
            Conditions::faceup_idle(),
            None,
            Some(LocationStats::new(willpower, lore, move_cost)),
        ),
        CardKind::Item => (Conditions::faceup_idle(), None, None),
        CardKind::Action => return,
    };
    let classifications = definition.classifications().to_vec();
    if let Some(p) = state.player_mut(player)
        && let Some(mut instance) = p.hand_mut().take(card)
    {
        *instance.conditions_mut() = conditions;
        instance.set_stats(char_stats);
        instance.set_location_stats(loc_stats);
        instance.set_classifications(classifications);
        instance.set_printed_cost(definition.cost());
        instance.set_names(definition.names().to_vec());
        p.play_mut().push(instance);
    }
}

/// Move `card` from `player`'s hand to their discard pile (§8.4).
fn discard_card(
    state: &mut GameState,
    player: PlayerId,
    card: CardId,
    events: &mut Vec<GameEvent>,
) {
    if let Some(p) = state.player_mut(player)
        && let Some(instance) = p.hand_mut().take(card)
    {
        p.discard_mut().push(instance);
        events.push(GameEvent::CardDiscarded { player, card });
    }
}

/// A zone an effect can move the source card to.
#[derive(Clone, Copy)]
enum SelfDestination {
    Hand,
    Inkwell,
    Discard,
    TopOfDeck,
    BottomOfDeck,
    ShuffleIntoDeck,
}

/// Move `card` (the effect's source) from play or the discard to `owner`'s hand
/// or inkwell, dissolving any stack into the destination (§5.1.7). If it was in
/// play, its continuous modifiers end (§7.6.4).
fn move_self_card(
    state: &mut GameState,
    registry: &CardRegistry,
    owner: PlayerId,
    card: CardId,
    dest: SelfDestination,
) {
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
            SelfDestination::Hand | SelfDestination::Discard => Conditions::faceup_idle(),
            // Facedown and exerted (Gramma Tala "facedown and exerted").
            SelfDestination::Inkwell => Conditions {
                ready: false,
                damage: 0,
                drying: false,
                facedown: true,
            },
            // Deck cards are facedown (§5.1.13.5).
            SelfDestination::TopOfDeck
            | SelfDestination::BottomOfDeck
            | SelfDestination::ShuffleIntoDeck => Conditions::in_deck(),
        };
        // A stack dissolves into the destination as separate cards (§5.1.7).
        for moved in instance.dissolve(conditions) {
            match dest {
                SelfDestination::Hand => p.hand_mut().push(moved),
                SelfDestination::Discard => p.discard_mut().push(moved),
                SelfDestination::Inkwell => p.inkwell_mut().push(moved),
                SelfDestination::TopOfDeck | SelfDestination::ShuffleIntoDeck => {
                    p.deck_mut().push(moved);
                }
                SelfDestination::BottomOfDeck => p.deck_mut().insert_bottom(moved),
            }
        }
    }
    if was_in_play {
        state.remove_modifiers_from_source(card);
    }
    // Enqueue inkwell triggers if cards were moved to inkwell
    if matches!(dest, SelfDestination::Inkwell) {
        enqueue_turn_triggers(
            state,
            registry,
            owner,
            &TriggerCondition::WhenCardPutInInkwell,
        );
    }
    // §8.2.4.1: a shuffled-in stack's cards take a free (RNG) order.
    if matches!(dest, SelfDestination::ShuffleIntoDeck) {
        state.shuffle_deck(owner);
    }
}

/// Apply a targeted effect to an already-resolved `target_card` (after a
/// `SelfCard`/`AllCharacters` resolution or a `ChooseTarget` decision). The
/// untargeted variants never reach here.
fn apply_effect_to(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    target_card: CardId,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) {
    match effect {
        // Move the target out of play / discard to its owner's hand, inkwell, or
        // deck, dissolving any stack into the destination (§5.1.7).
        Effect::Move {
            what: MoveSource::Card(_),
            to,
        } => {
            if let Some(owner) = owner_holding(state, target_card) {
                move_self_card(
                    state,
                    registry,
                    owner,
                    target_card,
                    destination_to_self(*to),
                );
            }
        }
        // Move damage from `from` to `to` (§9.3).
        Effect::MoveDamage {
            from, to, amount, ..
        } => apply_move_damage(state, controller, source, target_card, from, to, amount),
        Effect::GiveStrengthThisTurn { amount, .. }
        | Effect::DealDamage { amount, .. }
        | Effect::RemoveDamage { amount, .. } => {
            apply_amount_effect(
                state,
                registry,
                controller,
                source,
                target_card,
                effect,
                amount,
            );
        }
        Effect::Banish(_) => {
            banish_by_effect(state, registry, target_card, events);
        }
        Effect::Exert(_) | Effect::Ready(_) => {
            let ready = matches!(effect, Effect::Ready(_));
            if let Some(owner) = owner_holding(state, target_card)
                && let Some(c) = state
                    .player_mut(owner)
                    .and_then(|p| p.play_mut().iter_mut().find(|c| c.id() == target_card))
            {
                let was_not_ready = !c.conditions().ready;
                c.conditions_mut().ready = ready;
                if ready && was_not_ready {
                    // Enqueue "whenever this character is readied" triggers
                    enqueue_self_triggers(
                        state,
                        registry,
                        owner,
                        target_card,
                        &TriggerCondition::WhenThisReadies,
                    );
                }
            }
        }
        Effect::Freeze(_) => freeze_card(state, target_card),
        // Grant / restrict / conditional / never-reached effects.
        _ => apply_effect_to_rest(
            state,
            registry,
            controller,
            source,
            target_card,
            effect,
            events,
        ),
    }
}

/// Continuation of [`apply_effect_to`] (split to keep the match small): grant a
/// keyword / restriction / permission, the conditional-on-target branch, and the
/// untargeted variants that never reach here.
fn apply_effect_to_rest(
    state: &mut GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    target_card: CardId,
    effect: &Effect,
    events: &mut Vec<GameEvent>,
) {
    match effect {
        // Grant a triggered ability to the target until end of turn (§7.6).
        Effect::GrantAbilityThisTurn {
            condition,
            effect: granted,
            optional,
            ..
        } => {
            state.add_granted_trigger(GrantedTrigger {
                source: target_card,
                condition: condition.clone(),
                effect: (**granted).clone(),
                optional: *optional,
                duration: ModifierDuration::UntilEndOfTurn,
            });
        }
        // Grant a keyword / restriction / permission to the target until end of
        // turn (a single `UntilEndOfTurn` property modifier).
        Effect::GrantThisTurn { property, .. } => {
            grant_property(state, source, target_card, property.clone());
        }
        // Grant a property permanently (lasts while the target is in play, §7.6):
        // source = the target, so the leave-play sweep clears it.
        Effect::Grant { property, .. } => {
            state.add_property_modifier(PropertyModifier::new(
                target_card,
                ModifierTarget::Card(target_card),
                property.clone(),
                ModifierDuration::Permanent,
            ));
        }
        // Grant until the target's controller next readies ("at the start of their
        // next turn"); sourced to the target so it survives the granter leaving.
        Effect::GrantNextTurn { property, .. } => {
            if let Some(owner) = state.card_owner_in_play(target_card) {
                state.add_property_modifier(PropertyModifier::new(
                    target_card,
                    ModifierTarget::Card(target_card),
                    property.clone(),
                    ModifierDuration::UntilStep {
                        step: Step::Ready,
                        player: owner,
                    },
                ));
            }
        }
        // Grant an activated ability to the target until end of turn (§7.5).
        Effect::GrantActivatedThisTurn {
            ink,
            exert_self,
            effect: granted,
            ..
        } => {
            state.add_granted_activated(GrantedActivated {
                source: target_card,
                ink: *ink,
                exert_self: *exert_self,
                effect: (**granted).clone(),
                duration: ModifierDuration::UntilEndOfTurn,
            });
        }
        Effect::IfTargetMatches {
            filter,
            then,
            otherwise,
            ..
        } => {
            let owner = state.card_owner_in_play(target_card).unwrap_or(controller);
            let matched = state
                .instance_in_play(target_card)
                .is_some_and(|c| state.matches_filter(controller, source, owner, c, filter));
            let branch = if matched { then } else { otherwise };
            apply_effect_to(
                state,
                registry,
                controller,
                source,
                target_card,
                branch,
                events,
            );
        }
        // Everything else (untargeted effects resolved in `execute_effect`, and the
        // targeted effects handled by `apply_effect_to`) never reaches here.
        _ => {}
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

/// The in-play characters matching a [`CharacterFilter`] from `controller`'s
/// perspective (the algebra is evaluated per card).
fn matching_characters(
    state: &GameState,
    controller: PlayerId,
    source: CardId,
    filter: &CharacterFilter,
) -> Vec<CardId> {
    let mut out = Vec::new();
    for player in state.players() {
        let owner = player.id();
        for card in player.play().iter() {
            if card.is_character() && state.matches_filter(controller, source, owner, card, filter)
            {
                out.push(card.id());
            }
        }
    }
    out
}

/// The characters `controller` may **choose** as a target: the matching
/// characters minus those an opponent can't choose (Ward / "can't be chosen",
/// §10.15). Used only by the *choosing* targets — effects that affect all
/// characters use [`matching_characters`] directly, since they don't choose.
fn choosable_characters(
    state: &GameState,
    registry: &CardRegistry,
    controller: PlayerId,
    source: CardId,
    filter: &CharacterFilter,
) -> Vec<CardId> {
    matching_characters(state, controller, source, filter)
        .into_iter()
        .filter(|&card| {
            state.card_owner_in_play(card) == Some(controller)
                || !has_restriction(state, registry, card, Restriction::CantBeChosen)
        })
        .collect()
}

/// The player whose play area or discard currently holds `card`.
fn owner_holding(state: &GameState, card: CardId) -> Option<PlayerId> {
    state
        .players()
        .iter()
        .find(|p| p.play().contains(card) || p.discard().contains(card))
        .map(PlayerState::id)
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
            apply_enter_exerted_decision(state, registry, player, card, exert);
        }
        // Effect-resolution choices (target / up-to-N / discard / play-free / may).
        (pending, decision) => {
            apply_choice_decision(state, registry, pending, decision, &mut events)?;
        }
    }
    if !state.is_awaiting_decision() {
        events.extend(resolve_bag(state, registry));
    }
    // Resume a turn transition that suspended on a start/end-of-turn trigger.
    events.extend(resume_turn_progression(state, registry));
    Ok(events)
}

/// Dispatch the effect-resolution decisions (the general `Choose`, "may", and the
/// name-a-card variants) to their handlers.
#[allow(clippy::too_many_lines)] // one big (PendingDecision, Decision) dispatch
fn apply_choice_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    pending: PendingDecision,
    decision: Decision,
    events: &mut Vec<GameEvent>,
) -> Result<(), Rejected> {
    match (pending, decision) {
        (
            PendingDecision::MayResolveEffect {
                player,
                source,
                effect,
                rest,
            },
            Decision::May(yes),
        ) => {
            apply_may_decision(state, registry, player, source, effect, rest, yes, events);
            Ok(())
        }
        (
            PendingDecision::Choose {
                player,
                source,
                options,
                min,
                max,
                then,
                rest,
            },
            decision,
        ) => apply_choose_decision(
            state, registry, player, source, &options, min, max, &then, &decision, rest, events,
        ),
        (
            PendingDecision::NameCard {
                player,
                source,
                lore_on_match,
                match_to,
                otherwise_to,
                rest,
            },
            Decision::NameCard(named),
        ) => {
            apply_name_card_decision(
                state,
                registry,
                player,
                source,
                &named,
                &lore_on_match,
                match_to,
                otherwise_to,
                rest,
                events,
            );
            Ok(())
        }
        (
            PendingDecision::NameThenRecur {
                player,
                source,
                rest,
            },
            Decision::NameCard(named),
        ) => {
            apply_name_then_recur_decision(state, registry, player, source, &named, rest, events);
            Ok(())
        }
        (
            PendingDecision::ChooseOne {
                player,
                source,
                options,
                rest,
            },
            Decision::ChooseOption(index),
        ) => {
            apply_choose_one_decision(
                state, registry, player, source, &options, index, rest, events,
            );
            Ok(())
        }
        _ => Err(Rejected::InvalidDecision),
    }
}

/// Answer a [`PendingDecision::NameThenRecur`]: return every character card in
/// `player`'s discard whose definition has the `named` name to their hand, then
/// resume the continuation (§8.2; Blast from Your Past).
fn apply_name_then_recur_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    player: PlayerId,
    source: CardId,
    named: &str,
    rest: Vec<Effect>,
    events: &mut Vec<GameEvent>,
) {
    let _ = state.take_pending();
    let matching: Vec<CardId> = state.player(player).map_or_else(Vec::new, |p| {
        p.discard()
            .iter()
            .filter(|c| {
                registry.get(c.definition()).is_some_and(|d| {
                    d.has_name(named) && matches!(d.kind(), CardKind::Character { .. })
                })
            })
            .map(CardInstance::id)
            .collect()
    });
    for card in matching {
        move_self_card(state, registry, player, card, SelfDestination::Hand);
    }
    resolve_effects(state, registry, player, source, rest, events);
    events.extend(game_state_check_with_triggers(state, registry));
}

/// Answer a [`PendingDecision::NameCard`]: reveal the top of `player`'s deck; if
/// it has the `named` name, move it to `match_to` and gain `lore_on_match`, else
/// move it to `otherwise_to`; then resume the continuation (§8.2).
#[allow(clippy::too_many_arguments)]
fn apply_name_card_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    player: PlayerId,
    source: CardId,
    named: &str,
    lore_on_match: &Amount,
    match_to: Destination,
    otherwise_to: Destination,
    rest: Vec<Effect>,
    events: &mut Vec<GameEvent>,
) {
    let _ = state.take_pending();
    if let Some(&top) = peek_top(state, player, 1).first() {
        let matched = deck_card_def(state, player, top)
            .and_then(|d| registry.get(d))
            .is_some_and(|def| def.has_name(named));
        let _ = move_deck_top(
            state,
            player,
            1,
            if matched { match_to } else { otherwise_to },
        );
        if matched {
            let gained = u32::try_from(
                state
                    .eval_amount(player, source, source, lore_on_match)
                    .max(0),
            )
            .unwrap_or(0);
            if let Some(p) = state.player_mut(player) {
                p.add_lore(gained);
            }
            events.push(GameEvent::LoreGained {
                player,
                amount: gained,
            });
        }
    }
    resolve_effects(state, registry, player, source, rest, events);
    events.extend(game_state_check_with_triggers(state, registry));
}

/// Answer a [`PendingDecision::ChooseOne`]: execute the chosen effect option
/// (§7.1.9).
#[allow(clippy::too_many_arguments)]
fn apply_choose_one_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    player: PlayerId,
    source: CardId,
    options: &[Effect],
    index: u32,
    rest: Vec<Effect>,
    events: &mut Vec<GameEvent>,
) {
    let _ = state.take_pending();
    let chosen_effect = options
        .get(index as usize)
        .cloned()
        .unwrap_or_else(|| Effect::All(vec![])); // Default to no-op if index is invalid
    resolve_effects(state, registry, player, source, vec![chosen_effect], events);
    resolve_effects(state, registry, player, source, rest, events);
    events.extend(game_state_check_with_triggers(state, registry));
}

/// Answer the general [`PendingDecision::Choose`]: read the pick(s) from the
/// decision, validate them against `options` and the `min..=max` count, then run
/// the continuation `then` (§7.1).
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn apply_choose_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    player: PlayerId,
    source: CardId,
    options: &[ChoiceRef],
    min: u32,
    max: u32,
    then: &ChoiceThen,
    decision: &Decision,
    rest: Vec<Effect>,
    events: &mut Vec<GameEvent>,
) -> Result<(), Rejected> {
    let picks: Vec<ChoiceRef> = match decision {
        Decision::ChooseTarget(c) | Decision::PlayFreeChoice(c) => vec![ChoiceRef::Card(*c)],
        Decision::ChoosePlayer(p) => vec![ChoiceRef::Player(*p)],
        Decision::ChooseTargets(cs) | Decision::DiscardCards(cs) => {
            cs.iter().map(|c| ChoiceRef::Card(*c)).collect()
        }
        Decision::TakeRevealed(opt) => opt.iter().map(|c| ChoiceRef::Card(*c)).collect(),
        _ => return Err(Rejected::InvalidDecision),
    };
    let n = u32::try_from(picks.len()).unwrap_or(u32::MAX);
    let distinct = picks.iter().collect::<std::collections::HashSet<_>>().len() == picks.len();
    if n < min || n > max || !distinct || picks.iter().any(|r| !options.contains(r)) {
        return Err(Rejected::InvalidDecision);
    }
    let _ = state.take_pending();
    match then {
        // Single-pick: substitute the pick into the effect, then re-resolve.
        ChoiceThen::SubstituteAndResolve(effect) => {
            let resolved = match picks[0] {
                ChoiceRef::Card(c) => substitute_move_endpoint(effect, c),
                ChoiceRef::Player(p) => substitute_chosen_player(effect, p),
            };
            let effects: Vec<Effect> = std::iter::once(resolved).chain(rest).collect();
            resolve_effects(state, registry, player, source, effects, events);
        }
        // Apply the effect to each picked card (cards only), then the rest.
        ChoiceThen::ApplyToEach(effect) => {
            for pick in &picks {
                if let ChoiceRef::Card(c) = pick {
                    apply_effect_to(state, registry, player, source, *c, effect, events);
                }
            }
            resolve_effects(state, registry, player, source, rest, events);
        }
        // Play each picked card for free (a single pick), then the rest (§6).
        ChoiceThen::PlayFree => {
            for pick in &picks {
                if let ChoiceRef::Card(c) = pick {
                    play_card_free(state, registry, player, *c, events);
                }
            }
            resolve_effects(state, registry, player, source, rest, events);
        }
        // Take the (up-to-one) picked looked-at card to hand; the rest of
        // `looked_at` return to `deck_owner`'s deck at `rest_position` (§8.2).
        ChoiceThen::TakeRevealed {
            deck_owner,
            looked_at,
            rest_position,
        } => {
            let taken = match picks.first() {
                Some(ChoiceRef::Card(c)) => Some(*c),
                _ => None,
            };
            if let Some(card) = taken
                && let Some(mut instance) = state
                    .player_mut(*deck_owner)
                    .and_then(|p| p.deck_mut().take(card))
            {
                *instance.conditions_mut() = Conditions::faceup_idle();
                if let Some(p) = state.player_mut(player) {
                    p.hand_mut().push(instance);
                }
            }
            let remaining: Vec<CardId> = looked_at
                .iter()
                .copied()
                .filter(|&id| Some(id) != taken)
                .collect();
            place_revealed_rest(state, *deck_owner, &remaining, *rest_position);
            resolve_effects(state, registry, player, source, rest, events);
        }
        ChoiceThen::TakeRevealedPerCard {
            deck_owner,
            looked_at,
            destinations,
        } => {
            let taken = match picks.first() {
                Some(ChoiceRef::Card(c)) => Some(*c),
                _ => None,
            };
            // Take the chosen card to hand if any
            if let Some(card) = taken
                && let Some(mut instance) = state
                    .player_mut(*deck_owner)
                    .and_then(|p| p.deck_mut().take(card))
            {
                *instance.conditions_mut() = Conditions::faceup_idle();
                if let Some(p) = state.player_mut(player) {
                    p.hand_mut().push(instance);
                }
            }
            // Place remaining cards according to their destinations
            for (card, dest) in looked_at.iter().zip(destinations.iter()) {
                if Some(*card) != taken
                    && let Some(p) = state.player_mut(*deck_owner)
                    && let Some(instance) = p.deck_mut().take(*card)
                {
                    match dest {
                        DeckPosition::Bottom => p.deck_mut().insert_bottom(instance),
                        DeckPosition::Top => p.deck_mut().push(instance),
                        DeckPosition::Shuffle => {
                            p.deck_mut().push(instance);
                            state.shuffle_deck(*deck_owner);
                        }
                    }
                }
            }
            resolve_effects(state, registry, player, source, rest, events);
        }
        // Take the picked cards from the deck into hand, then shuffle (search deck, §8.2).
        ChoiceThen::SearchDeckTake { deck_owner } => {
            for pick in &picks {
                if let ChoiceRef::Card(c) = pick
                    && let Some(mut instance) = state
                        .player_mut(*deck_owner)
                        .and_then(|p| p.deck_mut().take(*c))
                {
                    *instance.conditions_mut() = Conditions::faceup_idle();
                    if let Some(p) = state.player_mut(player) {
                        p.hand_mut().push(instance);
                    }
                }
            }
            state.shuffle_deck(*deck_owner);
            resolve_effects(state, registry, player, source, rest, events);
        }
        // Discard the picked cards, then continue down the remaining players; the
        // next player who must choose suspends again (carrying `rest`, §8.4).
        ChoiceThen::Discard {
            amount,
            by,
            remaining_players,
        } => {
            for pick in &picks {
                if let ChoiceRef::Card(c) = pick {
                    discard_card(state, player, *c, events);
                }
            }
            if let Some(choice) =
                resolve_scope_discard(state, remaining_players, *amount, *by, events)
            {
                state.set_pending(choice_to_pending(choice, source, rest));
                return Ok(());
            }
            resolve_effects(state, registry, player, source, rest, events);
        }
        // Discard each picked card from `owner`'s hand (chosen by someone else, §8.4).
        ChoiceThen::DiscardFrom { owner } => {
            for pick in &picks {
                if let ChoiceRef::Card(c) = pick {
                    discard_card(state, *owner, *c, events);
                }
            }
            resolve_effects(state, registry, player, source, rest, events);
        }
    }
    events.extend(game_state_check_with_triggers(state, registry));
    Ok(())
}

/// Answer a [`PendingDecision::MayResolveEffect`]: resolve `effect` first if the
/// player agreed, then the continuation either way ("you may …", §7.1.3).
#[allow(clippy::too_many_arguments)]
fn apply_may_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    player: PlayerId,
    source: CardId,
    effect: Effect,
    rest: Vec<Effect>,
    yes: bool,
    events: &mut Vec<GameEvent>,
) {
    let effects: Vec<Effect> = if yes {
        std::iter::once(effect).chain(rest).collect()
    } else {
        rest
    };
    let _ = state.take_pending();
    resolve_effects(state, registry, player, source, effects, events);
    events.extend(game_state_check_with_triggers(state, registry));
}

/// Answer a [`PendingDecision::EnterPlayExerted`] (Bodyguard, §10.3.2): optionally
/// exert the entering character, then run its enters-play triggers.
fn apply_enter_exerted_decision(
    state: &mut GameState,
    registry: &CardRegistry,
    player: PlayerId,
    card: CardId,
    exert: bool,
) {
    let _ = state.take_pending();
    if exert
        && let Some(p) = state.player_mut(player)
        && let Some(c) = p.play_mut().iter_mut().find(|c| c.id() == card)
    {
        c.conditions_mut().ready = false;
    }
    if let Some(definition_id) = state.instance_in_play(card).map(CardInstance::definition) {
        enqueue_enter_play_triggers(state, registry, player, card, definition_id, false);
    }
}
