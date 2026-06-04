//! A thin text host over [`Game`](crate::application::Game).
//!
//! Renders the state + legal actions, offers an interactive loop, and a
//! deterministic auto-play `demo` (the demo is what the test/CI exercises; the
//! interactive loop is for humans).

use super::Game;
use crate::domain::cards::{CardDefinition, CardRegistry, load_toml_from};
use crate::domain::deck::Deck;
use crate::domain::game::GameStatus;
use crate::domain::types::ids::CardDefId;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

/// Build a combined card registry from every `*.toml` under `sets_dir`, assigning
/// unique ids across files (cards span sets).
///
/// # Errors
/// I/O or load errors are surfaced as a message string.
pub fn registry_from_dir(sets_dir: &Path) -> Result<CardRegistry, String> {
    let mut registry = CardRegistry::new();
    let mut next_id = 0u32;
    let mut files: Vec<_> = std::fs::read_dir(sets_dir)
        .map_err(|e| format!("read {}: {e}", sets_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    files.sort();
    for path in files {
        let toml =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let defs =
            load_toml_from(&toml, next_id).map_err(|e| format!("{}: {e}", path.display()))?;
        next_id += u32::try_from(defs.len()).unwrap_or(0) + 1;
        for def in defs {
            registry.insert(def);
        }
    }
    Ok(registry)
}

/// Load the card pool from `sets_dir`, the two decklists from `p1`/`p2` (the
/// community `count name` text format), build a game, and auto-play it to a
/// transcript.
///
/// # Errors
/// Returns a message if the pool/decklists can't be loaded or a deck is illegal.
pub fn play_from_files(
    sets_dir: &Path,
    p1: &Path,
    p2: &Path,
    seed: u64,
    max_steps: usize,
) -> Result<String, String> {
    let registry = registry_from_dir(sets_dir)?;
    let read = |p: &Path| -> Result<Deck, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        Deck::from_text(&text, &registry).map_err(|e| format!("{}: {e}", p.display()))
    };
    let decks = [read(p1)?, read(p2)?];
    let mut game = Game::from_decks(&decks, seed, registry).map_err(|e| format!("{e}"))?;
    Ok(auto_play(&mut game, seed, max_steps))
}

/// A self-contained demo registry: 30 vanilla 2/2 cost-1 characters.
fn demo_registry() -> CardRegistry {
    let mut reg = CardRegistry::new();
    for n in 0..30 {
        reg.insert(CardDefinition::character(
            CardDefId::from_raw(n),
            1,
            true,
            2,
            2,
            1,
        ));
    }
    reg
}

fn demo_decks() -> Vec<Vec<CardDefId>> {
    vec![
        (0..30).map(CardDefId::from_raw).collect(),
        (0..30).map(CardDefId::from_raw).collect(),
    ]
}

const fn is_finished(game: &Game) -> bool {
    matches!(game.status(), GameStatus::Finished { .. })
}

/// A human-readable one-screen summary of the game + its legal actions.
#[must_use]
pub fn render(game: &Game) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "== status: {:?} ==", game.status());
    for p in game.state().players() {
        let _ = writeln!(
            out,
            "  P{}: lore={} hand={} play={} ink={} deck={} discard={}",
            p.id().index(),
            p.lore(),
            p.hand().iter().count(),
            p.play().iter().count(),
            p.inkwell().iter().count(),
            p.deck().iter().count(),
            p.discard().iter().count(),
        );
    }
    out.push_str("  legal actions:\n");
    for (i, a) in game.legal_actions().iter().enumerate() {
        let _ = writeln!(out, "    [{i}] {a:?}");
    }
    out
}

/// Auto-play a deterministic random game (using `seed`), returning a transcript.
/// Picks a random legal action each step until the game finishes or `max_steps`.
///
/// # Panics
/// Panics if the demo game can't be created, or if a reported-legal action is
/// rejected (which would indicate a `legal_actions`/`apply` disagreement bug).
#[must_use]
pub fn demo(seed: u64, max_steps: usize) -> String {
    let mut game = Game::new(demo_decks(), seed, demo_registry()).expect("new game");
    auto_play(&mut game, seed, max_steps)
}

/// Drive `game` by submitting a deterministic random legal action each step until
/// it finishes or `max_steps`, returning the transcript.
///
/// # Panics
/// Panics if a reported-legal action is rejected (a `legal_actions`/`apply` bug).
fn auto_play(game: &mut Game, seed: u64, max_steps: usize) -> String {
    let mut state = seed.wrapping_add(1);
    let mut next = |bound: usize| -> usize {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        ((state >> 33) as usize) % bound.max(1)
    };

    let mut out = String::new();
    for step in 0..max_steps {
        if is_finished(game) {
            break;
        }
        let actions = game.legal_actions();
        if actions.is_empty() {
            out.push_str("(no enumerable action — stopping)\n");
            break;
        }
        let pick = actions[next(actions.len())].clone();
        let _ = writeln!(out, "step {step}: {pick:?}");
        let _ = game.submit(pick).expect("legal action accepted");
    }
    let _ = writeln!(out, "Game over: {:?}", game.status());
    out
}

/// An interactive stdin/stdout loop: print the state, read an action index, apply.
///
/// # Panics
/// Panics if the demo game can't be created.
pub fn run_interactive(seed: u64) {
    let mut game = Game::new(demo_decks(), seed, demo_registry()).expect("new game");
    let stdin = std::io::stdin();
    loop {
        if is_finished(&game) {
            println!("Game over: {:?}", game.status());
            return;
        }
        print!("{}", render(&game));
        let actions = game.legal_actions();
        if actions.is_empty() {
            println!("(no enumerable action available — this decision needs richer input)");
            return;
        }
        print!("> choose [0..{}]: ", actions.len() - 1);
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            return; // EOF
        }
        let idx = line
            .trim()
            .parse::<usize>()
            .unwrap_or(0)
            .min(actions.len() - 1);
        match game.submit(actions[idx].clone()) {
            Ok(events) => println!("ok ({} event(s))", events.len()),
            Err(e) => println!("rejected: {e:?}"),
        }
    }
}
