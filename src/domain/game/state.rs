//! The authoritative game state.

use super::{CardInstance, Conditions, PlayerState, SeededRng, Zone};
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
    active_player: PlayerId,
    turn_number: u32,
    phase: Phase,
    step: Step,
    next_card_id: u32,
}

impl GameState {
    /// Create a new game from one deck per player (each deck is an ordered list
    /// of printed-card ids) and a seed.
    ///
    /// Each deck is populated with freshly allocated, facedown card instances
    /// and then shuffled deterministically using the seed. The first turn
    /// begins for the player in seat 0, at the Beginning phase's Ready step.
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
            active_player: PlayerId::from_index(0),
            turn_number: 1,
            phase: Phase::Beginning,
            step: Step::Ready,
            next_card_id,
        }
    }

    /// The seed this game was created with.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// All player states, indexed by seat.
    #[must_use]
    pub fn players(&self) -> &[PlayerState] {
        &self.players
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

    /// The player whose turn it currently is.
    #[must_use]
    pub const fn active_player(&self) -> PlayerId {
        self.active_player
    }

    /// The current turn number (1-based).
    #[must_use]
    pub const fn turn_number(&self) -> u32 {
        self.turn_number
    }

    /// The current phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// The current step.
    #[must_use]
    pub const fn step(&self) -> Step {
        self.step
    }
}
