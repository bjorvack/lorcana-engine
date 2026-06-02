//! Win/loss evaluation — the single seam used by the game-state check (§1.9.2).
//!
//! Win/loss conditions are **not** a fixed list. The active set is derived each
//! check from a base layer plus modifications contributed by effects in play:
//!
//! * **base layer** (implemented here): reach the lore threshold to win
//!   (§1.9.1.1), lose on drawing from an empty deck (§1.9.1.2), and win as the
//!   last player standing (§3.2.1.3);
//! * **modification layer** (added with static abilities / the effect DSL):
//!   effects may **add** new conditions ("you win if you control 7 distinct
//!   Seven Dwarfs"), **remove/suppress** conditions ("you can't lose the
//!   game"), or **override** their parameters (Donald Duck – Flustered Sorcerer:
//!   "opponents need 25 lore to win"). Per the Golden Rules a card supersedes a
//!   game rule (§1.2.1) and a prevention beats a permission (§1.2.2), so a
//!   suppression wins over an add for the same outcome.
//!
//! Slice 1 implements only the base layer; the modification layer plugs into
//! [`check_win_loss`] once effects exist, without changing its call sites.

use super::RequiredAction;
use crate::domain::game::GameState;
use crate::domain::types::ids::PlayerId;

/// The amount of lore `player` must reach to win the game.
///
/// The base requirement is 20 lore (§1.9.1.1). Once static abilities exist, this
/// is where threshold modifiers (e.g. "opponents need 25 lore to win") apply, so
/// it takes the full game state and the player rather than being a constant.
#[must_use]
pub const fn lore_to_win(_state: &GameState, _player: PlayerId) -> u32 {
    20
}

/// Evaluate all win/loss conditions for the current state and return the
/// resulting required actions (§1.9.1, §1.9.2).
///
/// Eliminated players are ignored. The win-beats-lose tie-break (§1.9.2.1) is
/// applied: if a player would simultaneously win and lose, only the win is
/// returned. Results are ordered by seat.
#[must_use]
pub fn check_win_loss(state: &GameState) -> Vec<RequiredAction> {
    let mut actions = Vec::new();

    for player in state.players() {
        if player.is_eliminated() {
            continue;
        }
        let id = player.id();

        if player.lore() >= lore_to_win(state, id) {
            actions.push(RequiredAction::PlayerWins(id));
        } else if player.drew_from_empty_deck() {
            // `else` enforces the win-beats-lose tie-break (§1.9.2.1).
            actions.push(RequiredAction::PlayerLoses(id));
        }
    }

    // Last player standing wins (§3.2.1.3): only meaningful once at least one
    // player has already been eliminated.
    let remaining: Vec<&_> = state
        .players()
        .iter()
        .filter(|p| !p.is_eliminated())
        .collect();
    if state.players().len() > 1 && remaining.len() == 1 {
        let winner = RequiredAction::PlayerWins(remaining[0].id());
        if !actions.contains(&winner) {
            actions.push(winner);
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::{check_win_loss, lore_to_win};
    use crate::domain::game::GameState;
    use crate::domain::rules::RequiredAction;
    use crate::domain::types::ids::{CardDefId, PlayerId};

    // TODO(modification layer / Slice 5+): once static abilities and the effect
    // DSL can add / remove / override win-loss conditions, add tests for the
    // following edge cases. The active condition set is derived from the base
    // layer plus effect modifications, with prevention beating permission
    // (§1.2.2) and a card superseding a game rule (§1.2.1).
    //
    // Tracked by Slice 5 in docs/planning/IMPLEMENTATION_PLAN.md ("Win/loss
    // modification layer"), which converts these bullets into real tests.
    //
    // Override (tune an existing condition's parameters):
    //   - "Opponents need 25 lore to win" (Donald Duck – Flustered Sorcerer):
    //     opponent at 24 does not win, at 25 wins; the controller's own
    //     threshold stays 20.
    //   - Stacking threshold modifiers (e.g. two +5 effects => 30).
    //   - A hypothetical lowering override (e.g. "you win at 15 lore") works too.
    //
    // Add (introduce a brand-new condition):
    //   - "You win if you control 7 distinct Seven Dwarfs characters": 6 distinct
    //     => no win, 7 distinct => win, duplicates do not count.
    //   - "Opponents lose if ...": a new loss condition fires for opponents only.
    //
    // Remove / suppress (a prevention):
    //   - "You can't lose the game while this card is in play": a deck-out draw
    //     does NOT cause a loss while the card is present.
    //   - "Opponents can't win while this card is in play": an opponent at 20+
    //     does not win.
    //   - "You can't lose and your opponents can't win while this card is in
    //     play": both suppressions apply at once.
    //   - "The 20-lore win doesn't apply to you": controller at 20 does not win.
    //
    // Precedence / interaction:
    //   - Prevention beats add (§1.2.2): an added alternate win is suppressed by
    //     an opposing "you can't win".
    //   - Win still beats lose with modifiers present (an added win + a loss
    //     condition at once => win).
    //   - Multiple "can't lose" sources: removing one still leaves loss
    //     suppressed by the other.
    //
    // Duration / "as long as this card is on the field":
    //   - When the suppressing card leaves play, suppression ends immediately and
    //     a pending loss/win is applied at the very next game-state check (e.g. a
    //     deck-out flag set earlier now causes the loss).
    //
    // Multiplayer:
    //   - Simultaneous wins by multiple players in the same check.

    fn two_player_game() -> GameState {
        let decks = vec![
            (0..10).map(CardDefId::from_raw).collect::<Vec<_>>(),
            (10..20).map(CardDefId::from_raw).collect::<Vec<_>>(),
        ];
        GameState::new(decks, 1)
    }

    #[test]
    fn no_actions_at_game_start() {
        let state = two_player_game();
        assert!(check_win_loss(&state).is_empty());
    }

    #[test]
    fn reaching_the_lore_threshold_wins() {
        let mut state = two_player_game();
        let p0 = PlayerId::from_index(0);
        let threshold = lore_to_win(&state, p0);
        state.player_mut(p0).unwrap().add_lore(threshold);

        assert_eq!(check_win_loss(&state), vec![RequiredAction::PlayerWins(p0)]);
    }

    #[test]
    fn drawing_from_empty_deck_loses() {
        let mut state = two_player_game();
        let p1 = PlayerId::from_index(1);
        state.player_mut(p1).unwrap().note_drew_from_empty_deck();

        assert_eq!(
            check_win_loss(&state),
            vec![RequiredAction::PlayerLoses(p1)]
        );
    }

    #[test]
    fn winning_beats_losing_for_the_same_player() {
        let mut state = two_player_game();
        let p0 = PlayerId::from_index(0);
        {
            let player = state.player_mut(p0).unwrap();
            player.add_lore(20);
            player.note_drew_from_empty_deck();
        }

        // §1.9.2.1: a player who would win and lose at once wins.
        assert_eq!(check_win_loss(&state), vec![RequiredAction::PlayerWins(p0)]);
    }

    #[test]
    fn last_player_standing_wins() {
        let mut state = two_player_game();
        let p0 = PlayerId::from_index(0);
        let p1 = PlayerId::from_index(1);
        state.player_mut(p0).unwrap().eliminate();

        assert_eq!(check_win_loss(&state), vec![RequiredAction::PlayerWins(p1)]);
    }
}
