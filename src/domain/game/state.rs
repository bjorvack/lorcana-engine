//! The authoritative game state.

use super::{
    BagEntry, CardInstance, CharacterStats, Conditions, GameStatus, PendingDecision, PlayerState,
    SeededRng, Stat, StatModifier, TriggerId, Zone,
};
use crate::domain::effects::Effect;
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
    /// Triggered abilities waiting to resolve (§8.7).
    bag: Vec<BagEntry>,
    /// A decision the engine is waiting on before it can continue resolving.
    pending: Option<PendingDecision>,
    next_trigger_id: u32,
    /// Active continuous stat modifiers (§7.6, §7.8).
    modifiers: Vec<StatModifier>,
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
            bag: Vec::new(),
            pending: None,
            next_trigger_id: 0,
            modifiers: Vec::new(),
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

    /// The triggered abilities currently waiting in the bag (§8.7).
    #[must_use]
    pub fn bag(&self) -> &[BagEntry] {
        &self.bag
    }

    /// The decision the engine is currently waiting on, if any.
    #[must_use]
    pub const fn pending(&self) -> Option<&PendingDecision> {
        self.pending.as_ref()
    }

    /// `true` if the engine is waiting on a player decision.
    #[must_use]
    pub const fn is_awaiting_decision(&self) -> bool {
        self.pending.is_some()
    }

    /// Set the pending decision.
    pub fn set_pending(&mut self, pending: PendingDecision) {
        self.pending = Some(pending);
    }

    /// Take (clear) the pending decision.
    pub const fn take_pending(&mut self) -> Option<PendingDecision> {
        self.pending.take()
    }

    /// Add a triggered ability to the bag, returning its id (§8.7.3).
    pub fn enqueue_trigger(
        &mut self,
        controller: PlayerId,
        source: CardId,
        optional: bool,
        effect: Effect,
    ) -> TriggerId {
        let id = TriggerId::from_raw(self.next_trigger_id);
        self.next_trigger_id += 1;
        self.bag
            .push(BagEntry::new(id, controller, source, optional, effect));
        id
    }

    /// Remove and return a bag entry by id.
    pub fn remove_trigger(&mut self, id: TriggerId) -> Option<BagEntry> {
        let index = self.bag.iter().position(|e| e.id() == id)?;
        Some(self.bag.remove(index))
    }

    /// The ids of the bag entries controlled by a player, in bag order.
    #[must_use]
    pub fn triggers_for(&self, player: PlayerId) -> Vec<TriggerId> {
        self.bag
            .iter()
            .filter(|e| e.controller() == player)
            .map(BagEntry::id)
            .collect()
    }

    /// The active continuous stat modifiers (§7.6, §7.8).
    #[must_use]
    pub fn modifiers(&self) -> &[StatModifier] {
        &self.modifiers
    }

    /// Add a continuous stat modifier.
    pub fn add_modifier(&mut self, modifier: StatModifier) {
        self.modifiers.push(modifier);
    }

    /// Remove all modifiers generated by `source` (e.g. when it leaves play,
    /// §7.6.4).
    pub fn remove_modifiers_from_source(&mut self, source: CardId) {
        self.modifiers.retain(|m| m.source() != source);
    }

    /// Find an in-play card instance by id, searching every player's play area.
    #[must_use]
    pub fn instance_in_play(&self, card: CardId) -> Option<&CardInstance> {
        self.players
            .iter()
            .find_map(|p| p.play().iter().find(|c| c.id() == card))
    }

    /// The current stats of an in-play character: printed base plus the sum of
    /// applicable modifiers, each characteristic clamped to 0 at this point of
    /// use while the combined value is computed from the true (signed) total
    /// (§7.8.1.2/§7.8.2/§7.8.3). `None` if the card isn't an in-play character.
    #[must_use]
    pub fn current_character_stats(&self, card: CardId) -> Option<CharacterStats> {
        let base = self.instance_in_play(card)?.stats()?;
        Some(CharacterStats::new(
            apply_delta(base.strength, self.stat_delta(card, Stat::Strength)),
            apply_delta(base.willpower, self.stat_delta(card, Stat::Willpower)),
            apply_delta(base.lore, self.stat_delta(card, Stat::Lore)),
        ))
    }

    /// The combined (signed) modifier delta applying to a card's characteristic.
    #[must_use]
    pub fn stat_delta(&self, card: CardId, stat: Stat) -> i32 {
        self.modifiers
            .iter()
            .filter(|m| m.stat() == stat && self.target_matches(m.target(), card))
            .map(StatModifier::delta)
            .sum()
    }

    /// The player whose play area contains `card`, if any.
    #[must_use]
    pub fn card_owner_in_play(&self, card: CardId) -> Option<PlayerId> {
        self.players
            .iter()
            .find(|p| p.play().iter().any(|c| c.id() == card))
            .map(PlayerState::id)
    }

    /// Whether a modifier target applies to `card` (an in-play card).
    #[must_use]
    pub fn target_matches(&self, target: &super::ModifierTarget, card: CardId) -> bool {
        use super::ModifierTarget;
        match target {
            ModifierTarget::Card(c) => *c == card,
            ModifierTarget::OwnedCharacters {
                owner,
                classifications,
                except,
            } => {
                if *except == Some(card) {
                    return false;
                }
                let Some(instance) = self.instance_in_play(card) else {
                    return false;
                };
                if !instance.is_character() || self.card_owner_in_play(card) != Some(*owner) {
                    return false;
                }
                classifications.is_empty()
                    || classifications
                        .iter()
                        .any(|c| instance.has_classification(c))
            }
        }
    }
}

/// Combine a printed base value with a signed modifier delta, clamping the
/// result to 0 (the value as used; §7.8.2/§7.8.3).
fn apply_delta(base: u32, delta: i32) -> u32 {
    let total = i64::from(base) + i64::from(delta);
    u32::try_from(total.max(0)).unwrap_or(u32::MAX)
}
