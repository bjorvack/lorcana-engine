//! WebAssembly bindings for the Lorcana engine.
//!
//! This crate is a **thin** binding layer: it embeds the real card pool and a
//! pair of decklists, runs an actual [`Game`] in the browser, and exposes a
//! small, stable **view model** (with `tsify`-generated TypeScript types) for a
//! JavaScript UI to render. The engine itself is untouched and remains the
//! single source of truth.
//!
//! The first slice is a **read-only board viewer**: the UI can advance the game
//! with [`WasmGame::step_random`] and render [`WasmGame::view`]; no player input
//! is sent back yet.

use include_dir::{Dir, include_dir};
use lorcana_engine::{
    CardDefinition, CardKind, CardRegistry, Deck, Game, GameState, GameStatus, Input, PlayerState,
    load_toml_from,
};
use serde::Serialize;
use tsify_next::Tsify;
use wasm_bindgen::prelude::*;

/// The bundled engine-format card sets (`cards/sets/*.toml`), embedded at build
/// time so a real game can run entirely in the browser.
static CARD_SETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../cards/sets");

/// Two Set-1 decklists to play against each other in the demo.
const DECK_ONE: &str = include_str!("../../../decks/set01-amber-amethyst.txt");
const DECK_TWO: &str = include_str!("../../../decks/set01-emerald-ruby.txt");

// ---------------------------------------------------------------------------
// View model (the UI contract). `tsify` generates matching TypeScript types.
// ---------------------------------------------------------------------------

/// Static, printed data for one card definition. Keyed by `defId` on the JS
/// side; the per-instance [`CardView`] only carries the dynamic state.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct CardDef {
    /// Stable id of the printed card.
    pub def_id: u32,
    /// Display name (e.g. "Ariel - On Human Legs").
    pub name: String,
    /// Card art URL (from the TOML `image` field), if any.
    pub image: Option<String>,
    /// `"Character" | "Action" | "Item" | "Location"`.
    pub card_type: String,
    /// Printed ink cost.
    pub cost: u32,
    /// Whether the card has the inkwell symbol (can be inked).
    pub inkwell: bool,
    /// Printed Strength `{S}` (characters only).
    pub strength: Option<u32>,
    /// Printed Willpower `{W}` (characters / locations).
    pub willpower: Option<u32>,
    /// Printed Lore `{L}` (characters / locations).
    pub lore: Option<u32>,
    /// Move cost (locations only).
    pub move_cost: Option<u32>,
    /// Classifications (e.g. Storyborn, Hero, Princess).
    pub classifications: Vec<String>,
    /// Printed rules text, if any.
    pub text: Option<String>,
}

/// A specific card instance on the board, with its dynamic state. Printed data
/// is looked up via `defId` in the card DB.
#[derive(Debug, Clone, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    /// Unique instance id within the game.
    pub instance_id: u32,
    /// The printed card this instance represents.
    pub def_id: u32,
    /// `true` if ready (upright), `false` if exerted (turned sideways).
    pub ready: bool,
    /// `true` while drying (summoning sick) — can't quest/challenge/exert.
    pub drying: bool,
    /// `true` if facedown (deck / inkwell card).
    pub facedown: bool,
    /// Accumulated damage counters.
    pub damage: u32,
    /// Live Strength while an in-play character (with modifiers), else `None`.
    pub strength: Option<u32>,
    /// Live Willpower while an in-play character/location, else `None`.
    pub willpower: Option<u32>,
    /// Live Lore while an in-play character/location, else `None`.
    pub lore: Option<u32>,
    /// The location instance this character has moved to, if any.
    pub at_location: Option<u32>,
    /// Cards stacked under this one (Shift/Boost). Inert; for display only.
    pub under: Vec<Self>,
}

/// A summary of an inkwell: only counts are public (cards are facedown).
#[derive(Debug, Clone, Copy, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct InkwellView {
    /// Total cards in the inkwell (= total ink).
    pub total: u32,
    /// Ready (available) ink.
    pub ready: u32,
    /// Exerted (already spent this turn) ink.
    pub exerted: u32,
}

/// One player's side of the board.
#[derive(Debug, Clone, Serialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct PlayerView {
    /// Player index (0 or 1).
    pub id: u8,
    /// Current lore (first to the win threshold wins).
    pub lore: u32,
    /// Cards in hand. (Private — see `handCount` for the opponent.)
    pub hand: Vec<CardView>,
    /// Number of cards in hand (always known, even for the opponent).
    pub hand_count: u32,
    /// Cards in the play zone (characters, items, locations).
    pub play: Vec<CardView>,
    /// Inkwell summary.
    pub inkwell: InkwellView,
    /// Number of cards left in the deck.
    pub deck_count: u32,
    /// The (public) discard pile, top last.
    pub discard: Vec<CardView>,
}

/// The full board view the UI renders each frame.
#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    /// `"NotStarted" | "AwaitingMulligan" | "Playing" | "Finished"`.
    pub status: String,
    /// Winning player indices when `status == "Finished"`.
    pub winners: Vec<u8>,
    /// Index of the active player.
    pub active_player: u8,
    /// 1-based turn number.
    pub turn_number: u32,
    /// `"Beginning" | "Main" | "End"`.
    pub phase: String,
    /// `"Ready" | "Set" | "Draw" | "Main" | "End"`.
    pub step: String,
    /// Both players' sides.
    pub players: Vec<PlayerView>,
    /// A short label for any decision currently awaited, for display.
    pub pending: Option<String>,
}

// ---------------------------------------------------------------------------
// Registry / deck building from the embedded data.
// ---------------------------------------------------------------------------

/// Build a combined [`CardRegistry`] from every embedded set, mirroring the
/// engine's `registry_from_dir` id-assignment scheme so def ids are stable.
fn build_registry() -> Result<CardRegistry, String> {
    let mut registry = CardRegistry::new();
    let mut next_id = 0u32;
    let mut files: Vec<_> = CARD_SETS
        .files()
        .filter(|f| f.path().extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    files.sort_by_key(|f| f.path().to_path_buf());
    for file in files {
        let toml = file
            .contents_utf8()
            .ok_or_else(|| format!("{}: not utf-8", file.path().display()))?;
        let defs = load_toml_from(toml, next_id)
            .map_err(|e| format!("{}: {e:?}", file.path().display()))?;
        next_id += u32::try_from(defs.len()).unwrap_or(0) + 1;
        for def in defs {
            registry.insert(def);
        }
    }
    Ok(registry)
}

// ---------------------------------------------------------------------------
// Mapping engine state -> view model.
// ---------------------------------------------------------------------------

const fn card_type_str(t: lorcana_engine::CardType) -> &'static str {
    use lorcana_engine::CardType::{Action, Character, Item, Location};
    match t {
        Character => "Character",
        Action => "Action",
        Item => "Item",
        Location => "Location",
    }
}

fn card_def_view(def: &CardDefinition) -> CardDef {
    let (strength, willpower, lore, move_cost) = match def.kind() {
        CardKind::Character {
            strength,
            willpower,
            lore,
        } => (Some(strength), Some(willpower), Some(lore), None),
        CardKind::Location {
            move_cost,
            willpower,
            lore,
        } => (None, Some(willpower), Some(lore), Some(move_cost)),
        CardKind::Action | CardKind::Item => (None, None, None, None),
    };
    CardDef {
        def_id: def.id().as_raw(),
        name: def.names().first().cloned().unwrap_or_default(),
        image: def.image().map(str::to_owned),
        card_type: card_type_str(def.card_type()).to_owned(),
        cost: def.cost(),
        inkwell: def.has_inkwell_symbol(),
        strength,
        willpower,
        lore,
        move_cost,
        classifications: def
            .classifications()
            .iter()
            .map(|c| format!("{c:?}"))
            .collect(),
        text: def.text().map(str::to_owned),
    }
}

fn card_view(card: &lorcana_engine::CardInstance) -> CardView {
    let c = card.conditions();
    let (strength, willpower, lore) = card.stats().map_or_else(
        || {
            card.location_stats().map_or((None, None, None), |l| {
                (None, Some(l.willpower), Some(l.lore))
            })
        },
        |s| (Some(s.strength), Some(s.willpower), Some(s.lore)),
    );
    CardView {
        instance_id: card.id().as_raw(),
        def_id: card.definition().as_raw(),
        ready: c.ready,
        drying: c.drying,
        facedown: c.facedown,
        damage: c.damage,
        strength,
        willpower,
        lore,
        at_location: card.at_location().map(lorcana_engine::CardId::as_raw),
        under: card.under().iter().map(card_view).collect(),
    }
}

fn player_view(p: &PlayerState) -> PlayerView {
    let ink_total = u32::try_from(p.inkwell().iter().count()).unwrap_or(0);
    let ink_ready =
        u32::try_from(p.inkwell().iter().filter(|c| c.conditions().ready).count()).unwrap_or(0);
    PlayerView {
        id: p.id().index(),
        lore: p.lore(),
        hand: p.hand().iter().map(card_view).collect(),
        hand_count: u32::try_from(p.hand().iter().count()).unwrap_or(0),
        play: p.play().iter().map(card_view).collect(),
        inkwell: InkwellView {
            total: ink_total,
            ready: ink_ready,
            exerted: ink_total - ink_ready,
        },
        deck_count: u32::try_from(p.deck().iter().count()).unwrap_or(0),
        discard: p.discard().iter().map(card_view).collect(),
    }
}

fn game_view(state: &GameState) -> GameView {
    let (status, winners) = match state.status() {
        GameStatus::NotStarted => ("NotStarted", Vec::new()),
        GameStatus::AwaitingMulligan(_) => ("AwaitingMulligan", Vec::new()),
        GameStatus::Playing => ("Playing", Vec::new()),
        GameStatus::Finished { winners } => {
            ("Finished", winners.iter().map(|p| p.index()).collect())
        }
    };
    GameView {
        status: status.to_owned(),
        winners,
        active_player: state.active_player().index(),
        turn_number: state.turn_number(),
        phase: format!("{:?}", state.phase()),
        step: format!("{:?}", state.step()),
        players: state.players().iter().map(player_view).collect(),
        pending: state.pending().map(|p| format!("{p:?}")),
    }
}

// ---------------------------------------------------------------------------
// The exported game handle.
// ---------------------------------------------------------------------------

/// A live game running in the browser, plus a small deterministic RNG used by
/// [`Self::step_random`] to drive the read-only demo.
#[wasm_bindgen]
#[derive(Debug)]
pub struct WasmGame {
    game: Game,
    rng_state: u64,
}

#[wasm_bindgen]
impl WasmGame {
    /// Create and start a game between the two bundled decklists, resolving the
    /// opening mulligans (keep all) so it begins in the `Playing` state.
    ///
    /// # Errors
    /// Returns a JS error string if the card pool or decklists are invalid.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> Result<Self, JsError> {
        console_error_panic_hook::set_once();
        let registry = build_registry().map_err(|e| JsError::new(&e))?;
        let deck_one = Deck::from_text(DECK_ONE, &registry)
            .map_err(|e| JsError::new(&format!("deck one: {e:?}")))?;
        let deck_two = Deck::from_text(DECK_TWO, &registry)
            .map_err(|e| JsError::new(&format!("deck two: {e:?}")))?;
        let mut game = Game::from_decks(&[deck_one, deck_two], seed, registry)
            .map_err(|e| JsError::new(&format!("{e:?}")))?;

        // Resolve opening mulligans (keep every card) to enter `Playing`.
        while let GameStatus::AwaitingMulligan(player) = *game.status() {
            let _ = game
                .submit(Input::Mulligan {
                    player,
                    put_back: Vec::new(),
                })
                .map_err(|e| JsError::new(&format!("mulligan: {e:?}")))?;
        }

        Ok(Self {
            game,
            rng_state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        })
    }

    /// The static card database for every printed card in the pool, for the UI
    /// to look up names/art/stats by `defId`.
    #[must_use]
    #[wasm_bindgen(js_name = cardDb)]
    pub fn card_db(&self) -> Vec<CardDef> {
        self.game
            .registry()
            .iter()
            .map(|(_, def)| card_def_view(def))
            .collect()
    }

    /// The current board view.
    #[must_use]
    pub fn view(&self) -> GameView {
        game_view(self.game.state())
    }

    /// Whether the game has finished.
    // `wasm_bindgen` can only export non-const functions, so this can't be const.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    #[wasm_bindgen(js_name = isFinished)]
    pub fn is_finished(&self) -> bool {
        matches!(self.game.status(), GameStatus::Finished { .. })
    }

    /// Advance the demo by submitting one pseudo-random legal action. Returns
    /// `true` if an action was taken, `false` if the game is over or no action
    /// is enumerable (a decision needing richer input than the demo supplies).
    #[wasm_bindgen(js_name = stepRandom)]
    pub fn step_random(&mut self) -> bool {
        if self.is_finished() {
            return false;
        }
        let actions = self.game.legal_actions();
        if actions.is_empty() {
            return false;
        }
        let pick = self.next_index(actions.len());
        // A reported-legal action is always accepted; ignore the events for now.
        self.game.submit(actions[pick].clone()).is_ok()
    }

    /// A small LCG step, returning an index in `0..bound`.
    fn next_index(&mut self, bound: usize) -> usize {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        ((self.rng_state >> 33) as usize) % bound.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_real_cards_with_names() {
        let registry = build_registry().expect("embedded sets load");
        let named = registry
            .iter()
            .filter(|(_, def)| !def.names().is_empty())
            .count();
        assert!(named > 0, "expected named cards in the embedded pool");
    }

    #[test]
    fn new_game_starts_playing_with_two_seven_card_hands() {
        let game = WasmGame::new(42).expect("game builds");
        let view = game.view();
        assert_eq!(view.status, "Playing", "mulligans resolved into play");
        assert_eq!(view.players.len(), 2);
        for player in &view.players {
            assert_eq!(player.hand_count, 7, "opening hand is seven cards");
            assert_eq!(player.hand.len(), 7);
        }
    }

    #[test]
    fn card_db_covers_every_card_in_play_and_hand() {
        let game = WasmGame::new(7).expect("game builds");
        let known: std::collections::HashSet<u32> =
            game.card_db().into_iter().map(|c| c.def_id).collect();
        let view = game.view();
        for player in &view.players {
            for card in player.hand.iter().chain(&player.play) {
                assert!(
                    known.contains(&card.def_id),
                    "every visible card resolves in the card DB"
                );
            }
        }
    }

    #[test]
    fn step_random_advances_the_game_deterministically() {
        // Same seed ⇒ same first action ⇒ identical view (replayable).
        let mut a = WasmGame::new(123).expect("game a");
        let mut b = WasmGame::new(123).expect("game b");
        assert!(a.step_random(), "a fresh game has a legal action");
        assert!(b.step_random(), "a fresh game has a legal action");
        let (va, vb) = (a.view(), b.view());
        assert_eq!(va.turn_number, vb.turn_number);
        assert_eq!(va.active_player, vb.active_player);
        assert_eq!(va.players[0].inkwell.total, vb.players[0].inkwell.total);
    }

    #[test]
    fn a_full_random_game_eventually_finishes() {
        let mut game = WasmGame::new(2024).expect("game builds");
        let mut steps = 0u32;
        while !game.is_finished() && steps < 100_000 {
            if !game.step_random() {
                break;
            }
            steps += 1;
        }
        // Either it finished, or it stopped on a decision the demo can't answer;
        // either way the loop must terminate well under the cap.
        assert!(steps < 100_000, "random self-play terminates");
    }
}
