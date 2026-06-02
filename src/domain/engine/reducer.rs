//! The reducer: `start` sets a game up, `apply` advances it by one input.

use super::input::{Decision, Input, Rejected};
use crate::domain::cards::{CardKind, CardRegistry, GameRuleStatic, StaticAbility, StaticTarget};
use crate::domain::effects::{Effect, TriggerCondition};
use crate::domain::game::{
    CardInstance, CharacterStats, Conditions, GameEvent, GameState, GameStatus, ModifierDuration,
    ModifierTarget, PendingDecision, RuleModifier, StatModifier, TriggerId,
};
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
        Input::PlayCard { card } => apply_play_card(state, registry, card),
        Input::Quest { character } => apply_quest(state, registry, character),
        Input::Challenge { challenger, target } => apply_challenge(state, challenger, target),
        Input::UseAbility { card, ability } => apply_use_ability(state, registry, card, ability),
        Input::EndTurn => apply_end_turn(state),
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
    let CardKind::Character {
        strength,
        willpower,
        lore,
    } = definition.kind()
    else {
        return Err(Rejected::CardTypeNotPlayableYet(card));
    };
    if state
        .player(active)
        .expect("active player exists")
        .ready_ink()
        < definition.cost()
    {
        return Err(Rejected::InsufficientInk(card));
    }
    let statics = definition.static_abilities().to_vec();
    let rule_statics = definition.rule_statics().to_vec();
    let classifications = definition.classifications().to_vec();

    // --- mutate ---
    {
        let p = state.player_mut(active).expect("active player exists");
        p.exert_ink(definition.cost());
        let mut instance = p.hand_mut().take(card).expect("validated present");
        *instance.conditions_mut() = Conditions::entering_play();
        instance.set_stats(Some(CharacterStats::new(strength, willpower, lore)));
        instance.set_classifications(classifications);
        p.play_mut().push(instance);
    }
    // Static abilities apply as the card enters play (§7.6.2).
    apply_enter_statics(state, active, card, &statics);
    apply_enter_rule_statics(state, active, card, &rule_statics);

    let mut events = vec![GameEvent::CardPlayed {
        player: active,
        card,
    }];
    events.extend(game_state_check(state));
    if !state.is_finished() {
        // "When you play this character" triggers go to the bag (§4.3.4.8).
        enqueue_self_triggers(
            state,
            registry,
            active,
            card,
            TriggerCondition::WhenYouPlayThis,
        );
        events.extend(resolve_bag(state, registry));
    }
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
    // Questing requires a dry, ready character (§4.3.5.5).
    // TODO(keywords/effects): Reckless prevents questing (Slice 6); effects can
    // also forbid a specific character from questing (Slice 4/8).
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
            TriggerCondition::WhenThisQuests,
        );
        events.extend(resolve_bag(state, registry));
    }
    Ok(events)
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
/// Resolution hooks (absent for vanilla):
///   - "Whenever this character challenges / is challenged / banishes another in
///     a challenge" triggers (Scar, Mulan, Captain Hook, Cheshire Cat,
///     Marshmallow) go to the bag — Slice 4.
///   - Damage modification (Resist, "takes no damage from the challenge") —
///     Slice 6 / replacement effects Slice 8.
fn apply_challenge(
    state: &mut GameState,
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

    // Challenger: the active player's dry, ready character (§4.3.6.6).
    let challenger_instance = find_in_play(state, active, challenger)?;
    if !challenger_instance.is_character() {
        return Err(Rejected::NotACharacter(challenger));
    }
    if challenger_instance.conditions().drying {
        return Err(Rejected::CharacterStillDrying(challenger));
    }
    if !challenger_instance.conditions().ready {
        return Err(Rejected::CharacterExerted(challenger));
    }

    // Target: an exerted character belonging to another player (§4.3.6.7).
    let target_owner =
        opposing_owner_of(state, active, target).ok_or(Rejected::TargetNotInPlay(target))?;
    let target_instance = find_in_play(state, target_owner, target)?;
    if !target_instance.is_character() {
        return Err(Rejected::TargetNotACharacter(target));
    }
    if target_instance.conditions().ready {
        return Err(Rejected::TargetNotExerted(target));
    }

    // Current Strength includes continuous modifiers, clamped at 0 (§4.3.6.14,
    // §7.8.2).
    let challenger_strength = state
        .current_character_stats(challenger)
        .map_or(0, |s| s.strength);
    let target_strength = state
        .current_character_stats(target)
        .map_or(0, |s| s.strength);

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
    add_damage(state, active, challenger, target_strength);
    add_damage(state, target_owner, target, challenger_strength);

    let mut events = vec![GameEvent::Challenged {
        player: active,
        challenger,
        target,
    }];
    events.extend(game_state_check(state));
    Ok(events)
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
) -> Result<crate::domain::types::ids::CardDefId, Rejected> {
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
    let effect = ability.effect;

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
    execute_effect(state, active, effect, &mut events);
    events.extend(game_state_check(state));
    Ok(events)
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
    // "Until end of turn" effects end here (§7.6.1).
    state.expire_end_of_turn_modifiers();
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
    condition: TriggerCondition,
) {
    let Ok(instance) = find_in_play(state, controller, source) else {
        return;
    };
    let Some(definition) = registry.get(instance.definition()) else {
        return;
    };
    let matches: Vec<(bool, Effect)> = definition
        .abilities()
        .iter()
        .filter(|a| a.condition == condition)
        .map(|a| (a.optional, a.effect))
        .collect();
    for (optional, effect) in matches {
        let _ = state.enqueue_trigger(controller, source, optional, effect);
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
    execute_effect(state, entry.controller(), entry.effect(), events);
    let _ = registry; // effects don't consult the registry yet
    events.extend(game_state_check(state));
}

/// Apply a built-in effect for `controller`.
fn execute_effect(
    state: &mut GameState,
    controller: PlayerId,
    effect: Effect,
    events: &mut Vec<GameEvent>,
) {
    match effect {
        Effect::DrawCards(n) => {
            for _ in 0..n {
                events.push(draw(state, controller));
            }
        }
        Effect::GainLore(n) => {
            if let Some(p) = state.player_mut(controller) {
                p.add_lore(n);
            }
            events.push(GameEvent::LoreGained {
                player: controller,
                amount: n,
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
                    p.lose_lore(n);
                }
                events.push(GameEvent::LoreLost {
                    player: opponent,
                    amount: n,
                });
            }
        }
    }
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
        _ => return Err(Rejected::InvalidDecision),
    }
    if !state.is_awaiting_decision() {
        events.extend(resolve_bag(state, registry));
    }
    Ok(events)
}
