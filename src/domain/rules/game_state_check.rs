//! The game-state check (§1.9): apply required actions until the state is stable.

use super::{RequiredAction, check_win_loss};
use crate::domain::game::{GameEvent, GameState, GameStatus, PlayerState};
use crate::domain::types::ids::PlayerId;

/// Run the game-state check to completion (§1.9.2): repeatedly evaluate win/loss
/// conditions and apply the resulting required actions until none remain.
///
/// Win/loss is resolved before any other required action (§1.9.2); other
/// required actions (e.g. banishment) are added in later slices. Losers are
/// eliminated (emitting [`GameEvent::PlayerLost`]); when a win is detected the
/// game finishes (emitting [`GameEvent::GameEnded`]). The empty-deck-draw flag is
/// "since the last game state check" (§1.9.1.2), so it is cleared once the check
/// completes. Returns the events produced.
pub fn game_state_check(state: &mut GameState) -> Vec<GameEvent> {
    let mut events = Vec::new();

    while !state.is_finished() {
        let actions = check_win_loss(state);
        if actions.is_empty() {
            break;
        }

        let mut winners: Vec<PlayerId> = Vec::new();
        for action in actions {
            match action {
                RequiredAction::PlayerWins(player) => {
                    if !winners.contains(&player) {
                        winners.push(player);
                    }
                }
                RequiredAction::PlayerLoses(player) => {
                    if let Some(p) = state.player_mut(player)
                        && !p.is_eliminated()
                    {
                        p.eliminate();
                        events.push(GameEvent::PlayerLost { player });
                    }
                }
            }
        }

        if !winners.is_empty() {
            state.set_status(GameStatus::Finished {
                winners: winners.clone(),
            });
            events.push(GameEvent::GameEnded { winners });
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
