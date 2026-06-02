//! The game-state check (§1.9): apply required actions until the state is stable.

use super::{RequiredAction, check_win_loss};
use crate::domain::game::{GameEvent, GameState, GameStatus, PlayerState};
use crate::domain::types::ids::{CardId, PlayerId};

/// Run the game-state check to completion (§1.9.2): repeatedly evaluate the
/// conditions and apply the resulting required actions until none remain.
///
/// Win/loss is resolved **before** any other required action each pass (§1.9.2):
/// losers are eliminated (emitting [`GameEvent::PlayerLost`]) and a win finishes
/// the game (emitting [`GameEvent::GameEnded`]). Only when no win/loss applies are
/// other required actions taken — currently banishment of characters whose damage
/// has reached their willpower (§1.9.1.3), moving them to discard and clearing
/// their counters (§9.4, §8.6.2). The empty-deck-draw flag is "since the last game
/// state check" (§1.9.1.2), so it is cleared once the check completes.
///
/// TODO(triggers/replacement): banishment is a hook point — "when this character
/// is banished" / "whenever this character banishes another in a challenge"
/// triggers go to the bag (Slice 4), and banishment can be replaced/prevented
/// (Slice 8). §1.9.1.3's "banished by that character" attribution is needed for
/// those triggers and isn't tracked yet.
pub fn game_state_check(state: &mut GameState) -> Vec<GameEvent> {
    let mut events = Vec::new();

    loop {
        if state.is_finished() {
            break;
        }

        // Win/loss first (§1.9.2).
        let win_loss = check_win_loss(state);
        if !win_loss.is_empty() {
            apply_win_loss(state, &win_loss, &mut events);
            continue;
        }

        // Then other required actions: banishment.
        let banishable = banishable_cards(state);
        if banishable.is_empty() {
            break;
        }
        for (player, card) in banishable {
            banish(state, player, card, &mut events);
        }
    }

    // A game with no players left standing and no winner is a draw.
    if !state.is_finished()
        && !state.players().is_empty()
        && state.players().iter().all(PlayerState::is_eliminated)
    {
        state.set_status(GameStatus::Finished {
            winners: Vec::new(),
        });
        events.push(GameEvent::GameEnded {
            winners: Vec::new(),
        });
    }

    for player in state.players_mut() {
        player.clear_drew_from_empty_deck();
    }

    events
}

/// Apply the win/loss required actions from a single check pass.
fn apply_win_loss(state: &mut GameState, actions: &[RequiredAction], events: &mut Vec<GameEvent>) {
    let mut winners: Vec<PlayerId> = Vec::new();
    for action in actions {
        match action {
            RequiredAction::PlayerWins(player) => {
                if !winners.contains(player) {
                    winners.push(*player);
                }
            }
            RequiredAction::PlayerLoses(player) => {
                if let Some(p) = state.player_mut(*player)
                    && !p.is_eliminated()
                {
                    p.eliminate();
                    events.push(GameEvent::PlayerLost { player: *player });
                }
            }
            // check_win_loss only produces win/loss actions.
            RequiredAction::Banish { .. } => {}
        }
    }

    if !winners.is_empty() {
        state.set_status(GameStatus::Finished {
            winners: winners.clone(),
        });
        events.push(GameEvent::GameEnded { winners });
    }
}

/// Collect the in-play characters whose damage has reached their (current,
/// modifier-adjusted) willpower.
fn banishable_cards(state: &GameState) -> Vec<(PlayerId, CardId)> {
    let mut out = Vec::new();
    for player in state.players() {
        for card in player.play().iter() {
            if let Some(stats) = state.current_character_stats(card.id())
                && card.conditions().damage >= stats.willpower
            {
                out.push((player.id(), card.id()));
            }
        }
    }
    out
}

/// Banish a card: move it from play to its owner's discard, clearing its damage
/// counters (§9.4) and in-play stats.
fn banish(state: &mut GameState, player: PlayerId, card: CardId, events: &mut Vec<GameEvent>) {
    if let Some(p) = state.player_mut(player)
        && let Some(mut instance) = p.play_mut().take(card)
    {
        instance.conditions_mut().damage = 0;
        instance.set_stats(None);
        p.discard_mut().push(instance);
        events.push(GameEvent::Banished { player, card });
        // The card left play: any continuous modifiers it generated end (§7.6.4).
        state.remove_modifiers_from_source(card);
    }
}
