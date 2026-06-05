//! A small facade over the engine: create a game, submit actions, inspect state,
//! and enumerate the **legal actions** available right now.
//!
//! `legal_actions` reuses the reducer's own validation by *trying* each candidate
//! on a clone (so it can never drift from `apply`), then keeping the ones that are
//! accepted. Pending-decision options are read directly from the decision.

use crate::domain::cards::CardRegistry;
use crate::domain::deck::{Deck, DeckError};
use crate::domain::engine::{Decision, Input, Rejected, apply, start};
use crate::domain::game::{
    CardInstance, ChoiceRef, GameEvent, GameState, GameStatus, PendingDecision,
};
use crate::domain::types::ids::{CardDefId, CardId};

/// Why building a game from decks failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetupError {
    /// A deck (by index) violated the deck-building rules (§2.1.1).
    #[error("deck {index} is illegal: {errors:?}")]
    IllegalDeck {
        /// Index of the offending deck in the input slice.
        index: usize,
        /// The violations found.
        errors: Vec<DeckError>,
    },
    /// The game couldn't be started.
    #[error("could not start the game: {0:?}")]
    Start(Rejected),
}

/// A playable game: the [`GameState`] plus the [`CardRegistry`] it's played with.
#[derive(Debug, Clone)]
pub struct Game {
    state: GameState,
    registry: CardRegistry,
}

impl Game {
    /// Create and **start** a game (deals hands; leaves it awaiting mulligans).
    ///
    /// # Errors
    /// Propagates a [`Rejected`] from `start` (e.g. an already-started state).
    pub fn new(
        decks: Vec<Vec<CardDefId>>,
        seed: u64,
        registry: CardRegistry,
    ) -> Result<Self, Rejected> {
        let mut state = GameState::new(decks, seed);
        let _ = start(&mut state)?;
        Ok(Self { state, registry })
    }

    /// Validate each [`Deck`] against `registry` (§2.1.1), then build and start a
    /// game from them — the ergonomic entry point for real decklists.
    ///
    /// # Errors
    /// [`SetupError::IllegalDeck`] if any deck is illegal; [`SetupError::Start`]
    /// if starting fails.
    pub fn from_decks(
        decks: &[Deck],
        seed: u64,
        registry: CardRegistry,
    ) -> Result<Self, SetupError> {
        for (index, deck) in decks.iter().enumerate() {
            deck.validate(&registry)
                .map_err(|errors| SetupError::IllegalDeck { index, errors })?;
        }
        let lists = decks.iter().map(Deck::expand).collect();
        Self::new(lists, seed, registry).map_err(SetupError::Start)
    }

    /// Apply an action, returning the events it produced.
    ///
    /// # Errors
    /// Returns [`Rejected`] if the action is illegal in the current state.
    pub fn submit(&mut self, input: Input) -> Result<Vec<GameEvent>, Rejected> {
        apply(&mut self.state, &self.registry, input)
    }

    /// The current game state.
    #[must_use]
    pub const fn state(&self) -> &GameState {
        &self.state
    }

    /// The card registry this game uses.
    #[must_use]
    pub const fn registry(&self) -> &CardRegistry {
        &self.registry
    }

    /// The current status (setup / playing / finished).
    #[must_use]
    pub const fn status(&self) -> &GameStatus {
        self.state.status()
    }

    /// The decision currently awaited, if any.
    #[must_use]
    pub const fn pending(&self) -> Option<&PendingDecision> {
        self.state.pending()
    }

    /// Enumerate the actions that are legal right now.
    ///
    /// - While a decision is pending: the answers to that decision (open-ended
    ///   "name a card" decisions yield none — the host supplies the name).
    /// - While awaiting a mulligan: the keep-all mulligan (the host chooses which
    ///   cards, if any, to put back).
    /// - Otherwise: every accepted turn action (validated by trying it on a clone).
    ///
    /// Multi-pick decisions and Shift/Sing options are best-effort for now.
    #[must_use]
    pub fn legal_actions(&self) -> Vec<Input> {
        if let Some(pending) = self.state.pending() {
            return decision_actions(pending);
        }
        if let GameStatus::AwaitingMulligan(player) = *self.state.status() {
            return vec![Input::Mulligan {
                player,
                put_back: Vec::new(),
            }];
        }
        self.candidate_moves()
            .into_iter()
            .filter(|input| self.is_legal(input))
            .collect()
    }

    /// Whether `input` would be accepted (tried on a clone — no mutation here).
    fn is_legal(&self, input: &Input) -> bool {
        let mut probe = self.state.clone();
        apply(&mut probe, &self.registry, input.clone()).is_ok()
    }

    /// All turn actions worth trying for the active player (filtered by `is_legal`).
    fn candidate_moves(&self) -> Vec<Input> {
        let active = self.state.active_player();
        let Some(me) = self.state.player(active) else {
            return Vec::new();
        };
        let hand: Vec<CardId> = me.hand().iter().map(CardInstance::id).collect();
        let own: Vec<CardId> = me.play().iter().map(CardInstance::id).collect();
        let locations: Vec<CardId> = me
            .play()
            .iter()
            .filter(|c| c.is_location())
            .map(CardInstance::id)
            .collect();
        let foes: Vec<CardId> = self
            .state
            .players()
            .iter()
            .filter(|p| p.id() != active)
            .flat_map(|p| p.play().iter().map(CardInstance::id))
            .collect();

        let mut out = vec![Input::EndTurn];
        for card in hand {
            out.push(Input::PutCardInInkwell { card });
            out.push(Input::PlayCard {
                card,
                shift_onto: None,
            });
        }
        for &card in &own {
            out.push(Input::Quest { character: card });
            out.push(Input::Boost { card });
            for &location in &locations {
                out.push(Input::MoveCharacter {
                    character: card,
                    location,
                });
            }
            for &target in &foes {
                out.push(Input::Challenge {
                    challenger: card,
                    target,
                });
            }
            if let Some(def) = me
                .play()
                .iter()
                .find(|c| c.id() == card)
                .map(CardInstance::definition)
                .and_then(|d| self.registry.get(d))
            {
                for ability in 0..def.activated_abilities().len() {
                    out.push(Input::UseAbility { card, ability });
                }
            }
        }
        out
    }
}

/// The decisions that answer a pending decision.
fn decision_actions(pending: &PendingDecision) -> Vec<Input> {
    let decide = |d: Decision| Input::Decide(d);
    match pending {
        PendingDecision::OrderTriggers { options, .. } => options
            .iter()
            .map(|t| decide(Decision::ResolveNext(*t)))
            .collect(),
        PendingDecision::MayResolveEffect { .. } => {
            vec![decide(Decision::May(true)), decide(Decision::May(false))]
        }
        PendingDecision::EnterPlayExerted { .. } => vec![
            decide(Decision::EnterExerted(true)),
            decide(Decision::EnterExerted(false)),
        ],
        PendingDecision::Choose {
            options, min, max, ..
        } => choose_actions(options, *min, *max),
        // Open-ended: the host supplies the named card.
        PendingDecision::NameCard { .. } | PendingDecision::NameThenRecur { .. } => Vec::new(),
        PendingDecision::ChooseOne { options, .. } => {
            let decide = |d: Decision| Input::Decide(d);
            options
                .iter()
                .enumerate()
                .filter_map(|(i, _)| u32::try_from(i).ok())
                .map(|i| decide(Decision::ChooseOption(i)))
                .collect()
        }
    }
}

/// Answers to a general `Choose` decision. Single-pick is fully enumerated;
/// multi-pick offers one valid pick (subset enumeration is a follow-up).
fn choose_actions(options: &[ChoiceRef], min: u32, max: u32) -> Vec<Input> {
    let decide = |d: Decision| Input::Decide(d);
    let mut out = Vec::new();
    if max <= 1 {
        for opt in options {
            out.push(match opt {
                ChoiceRef::Card(c) => decide(Decision::ChooseTarget(*c)),
                ChoiceRef::Player(p) => decide(Decision::ChoosePlayer(*p)),
            });
        }
        if min == 0 {
            out.push(decide(Decision::ChooseTargets(Vec::new())));
        }
    } else {
        let cards: Vec<CardId> = options
            .iter()
            .filter_map(|o| match o {
                ChoiceRef::Card(c) => Some(*c),
                ChoiceRef::Player(_) => None,
            })
            .collect();
        if min == 0 {
            out.push(decide(Decision::ChooseTargets(Vec::new())));
        }
        if cards.len() >= min as usize {
            let pick: Vec<CardId> = cards.into_iter().take(max as usize).collect();
            out.push(decide(Decision::ChooseTargets(pick)));
        }
    }
    out
}
