//! The authoritative game state.

use super::{CardInstance, Conditions, GameStatus, PlayerState, SeededRng, Zone};
use crate::domain::types::ids::{CardDefId, CardId, PlayerId};
use crate::domain::types::turn::{Phase, Step};
use serde::{Deserialize, Serialize};

/// The single source of truth for a game.
///
/// The state is fully self-contained and serializable: it carries the seed and
/// the live [`SeededRng`] so that a saved game (or a replay built from the same
/// seed and inputs) reproduces exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameState {
    seed: u64,
    rng: SeededRng,
    players: Vec<PlayerState>,
    status: GameStatus,
    active_player: PlayerId,
    turn_number: u32,
    phase: Phase,
    step: Step,
    /// Whether the active player has used their once-per-turn inkwell action
    /// (§4.3.3). Reset at the start of each turn.
    inked_this_turn: bool,
    next_card_id: u32,
}

impl GameState {
    /// Build a new, **not yet started** game from one deck per player (each deck
    /// is an ordered list of printed-card ids) and a seed.
    ///
    /// Each deck is populated with freshly allocated, facedown card instances and
    /// shuffled deterministically using the seed. No hands are dealt and no
    /// starting player is chosen yet; call the engine's `start` to do that. The
    /// turn fields hold placeholder values until the game is `Playing`.
    ///
    /// # Panics
    ///
    /// Panics if more than `u8::MAX` decks are supplied (there cannot be that
    /// many players).
    #[must_use]
    pub fn new(decks: Vec<Vec<CardDefId>>, seed: u64) -> Self {
        let mut rng = SeededRng::from_seed(seed);
        let mut next_card_id: u32 = 0;
        let mut players = Vec::with_capacity(decks.len());

        for (seat, deck_defs) in decks.into_iter().enumerate() {
            let seat = u8::try_from(seat).expect("a game cannot have more than 255 players");
            let mut deck = Zone::new();
            for definition in deck_defs {
                let instance = CardInstance::new(
                    CardId::from_raw(next_card_id),
                    definition,
                    Conditions::in_deck(),
                );
                next_card_id += 1;
                deck.push(instance);
            }
            deck.shuffle(&mut rng);
            players.push(PlayerState::new(PlayerId::from_index(seat), deck));
        }

        Self {
            seed,
            rng,
            players,
            status: GameStatus::NotStarted,
            active_player: PlayerId::from_index(0),
            turn_number: 1,
            phase: Phase::Beginning,
            step: Step::Ready,
            inked_this_turn: false,
            next_card_id,
        }
    }

    /// The seed this game was created with.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// The game's lifecycle status.
    #[must_use]
    pub const fn status(&self) -> &GameStatus {
        &self.status
    }

    /// `true` once the game has finished.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(self.status, GameStatus::Finished { .. })
    }

    /// Set the game's status (used by the engine as it advances the game).
    pub fn set_status(&mut self, status: GameStatus) {
        self.status = status;
    }

    /// All player states, indexed by seat.
    #[must_use]
    pub fn players(&self) -> &[PlayerState] {
        &self.players
    }

    /// Mutable access to all player states.
    pub fn players_mut(&mut self) -> &mut [PlayerState] {
        &mut self.players
    }

    /// The number of players in the game.
    #[must_use]
    pub const fn player_count(&self) -> usize {
        self.players.len()
    }

    /// The state of a specific player, if present.
    #[must_use]
    pub fn player(&self, id: PlayerId) -> Option<&PlayerState> {
        self.players.get(usize::from(id.index()))
    }

    /// Mutable access to a specific player's state, if present.
    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut PlayerState> {
        self.players.get_mut(usize::from(id.index()))
    }

    /// Mutable access to the deterministic RNG (for shuffles and choices).
    pub const fn rng_mut(&mut self) -> &mut SeededRng {
        &mut self.rng
    }

    /// Deterministically shuffle a player's deck using the game RNG.
    pub fn shuffle_deck(&mut self, id: PlayerId) {
        if let Some(player) = self.players.get_mut(usize::from(id.index())) {
            player.deck_mut().shuffle(&mut self.rng);
        }
    }

    /// The player whose turn it currently is.
    #[must_use]
    pub const fn active_player(&self) -> PlayerId {
        self.active_player
    }

    /// Set the active player.
    pub const fn set_active_player(&mut self, player: PlayerId) {
        self.active_player = player;
    }

    /// The current turn number (1-based).
    #[must_use]
    pub const fn turn_number(&self) -> u32 {
        self.turn_number
    }

    /// Advance to the next turn number.
    pub const fn increment_turn_number(&mut self) {
        self.turn_number += 1;
    }

    /// The current phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Set the current phase.
    pub const fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
    }

    /// The current step.
    #[must_use]
    pub const fn step(&self) -> Step {
        self.step
    }

    /// Set the current step.
    pub const fn set_step(&mut self, step: Step) {
        self.step = step;
    }

    /// Whether the active player has taken their once-per-turn inkwell action.
    #[must_use]
    pub const fn inked_this_turn(&self) -> bool {
        self.inked_this_turn
    }

    /// Record whether the active player has used their inkwell action this turn.
    pub const fn set_inked_this_turn(&mut self, value: bool) {
        self.inked_this_turn = value;
    }
}
