//! Behaviour audit logging (feature `audit-log`, off in release builds).
//!
//! Plays a game and emits a human/AI-reviewable transcript that pairs each
//! acting card's **printed text** with the **events it produced**, so a reviewer
//! can spot "the text says X but the engine did Y" across the authored cards.
//!
//! Card ids in the action/event debug output are resolved to card **names**.

use super::host::registry_from_dir;
use super::{Game, SetupError};
use crate::domain::cards::CardRegistry;
use crate::domain::deck::Deck;
use crate::domain::game::GameStatus;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Maps a card's debug token (`"CardId(7)"`) to its `(name, printed text)`.
type Names = BTreeMap<String, (String, Option<String>)>;

/// Accumulate every card currently in any zone into `names` (cards keep moving
/// zones, so the map is built cumulatively to resolve ids seen at any point).
fn collect_names(game: &Game, names: &mut Names) {
    let registry = game.registry();
    for player in game.state().players() {
        let zones = [
            player.deck(),
            player.hand(),
            player.play(),
            player.inkwell(),
            player.discard(),
        ];
        for zone in zones {
            for inst in zone.iter() {
                let token = format!("{:?}", inst.id());
                if let Some(def) = registry.get(inst.definition()) {
                    let name = def
                        .names()
                        .first()
                        .cloned()
                        .unwrap_or_else(|| token.clone());
                    let _ = names
                        .entry(token)
                        .or_insert_with(|| (name, def.text().map(str::to_owned)));
                }
            }
        }
    }
}

/// Replace each `CardId(n)` token in a debug string with the card's name.
fn with_names(debug: &str, names: &Names) -> String {
    let mut out = String::new();
    let mut rest = debug;
    while let Some(pos) = rest.find("CardId(") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + "CardId(".len()..];
        if let Some(end) = after.find(')') {
            let token = format!("CardId({})", &after[..end]);
            match names.get(&token) {
                Some((name, _)) => out.push_str(name),
                None => out.push_str(&token),
            }
            rest = &after[end + 1..];
        } else {
            out.push_str(rest);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// The printed text of the first card referenced by an action's debug string
/// (the actor — the card played/quested/challenged/etc.).
fn actor_text(action_debug: &str, names: &Names) -> Option<String> {
    let pos = action_debug.find("CardId(")?;
    let after = &action_debug[pos + "CardId(".len()..];
    let end = after.find(')')?;
    let token = format!("CardId({})", &after[..end]);
    names.get(&token).and_then(|(_, text)| text.clone())
}

const fn finished(game: &Game) -> bool {
    matches!(game.status(), GameStatus::Finished { .. })
}

/// Play a game from `decks` and return the behaviour audit transcript.
///
/// # Errors
/// Propagates a [`SetupError`] if the decks are illegal or the game can't start.
pub fn play_and_log(
    decks: &[Deck],
    seed: u64,
    registry: CardRegistry,
    max_steps: usize,
) -> Result<String, SetupError> {
    let mut game = Game::from_decks(decks, seed, registry)?;
    let mut names = Names::new();
    let mut rng = seed.wrapping_add(1);
    let mut next = |bound: usize| -> usize {
        rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        ((rng >> 33) as usize) % bound.max(1)
    };

    let mut log = String::new();
    let _ = writeln!(log, "# Behaviour audit (seed {seed})\n");
    for step in 0..max_steps {
        collect_names(&game, &mut names);
        if finished(&game) {
            break;
        }
        let actions = game.legal_actions();
        if actions.is_empty() {
            log.push_str("(no enumerable action — stopping)\n");
            break;
        }
        let pick = actions[next(actions.len())].clone();
        let raw = format!("{pick:?}");
        let text = actor_text(&raw, &names);
        let Ok(events) = game.submit(pick) else {
            let _ = writeln!(
                log,
                "{step}. {} -> REJECTED (legal/apply bug!)",
                with_names(&raw, &names)
            );
            continue;
        };
        collect_names(&game, &mut names);

        let _ = writeln!(log, "{step}. {}", with_names(&raw, &names));
        if let Some(text) = text {
            let _ = writeln!(log, "      text: {text}");
        }
        for event in &events {
            let _ = writeln!(
                log,
                "      -> {}",
                with_names(&format!("{event:?}"), &names)
            );
        }
    }
    let _ = writeln!(log, "\nGame over: {:?}", game.status());
    Ok(log)
}

/// Build the card pool from `sets_dir`, load two decklists, and return the audit
/// transcript of a game between them.
///
/// # Errors
/// Returns a message if the pool/decklists can't be loaded or a deck is illegal.
pub fn audit_from_files(
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
    play_and_log(&decks, seed, registry, max_steps).map_err(|e| format!("{e}"))
}
