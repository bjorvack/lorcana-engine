//! Validate a single card's TOML (including its effect-DSL abilities) through the
//! engine's real loader. Reads a TOML document from stdin; prints `OK` and exits 0
//! if every card maps, or `ERROR: <detail>` and exits 1 otherwise.
//!
//! Accepts either a set document (`[[card]]` array) or a single **per-card** file
//! (top-level fields, as under `cards/<set>/*.toml`) — the latter is wrapped in a
//! `[[card]]` so the same loader path validates it.
//!
//! With `--debug` (or `-d`), on success prints a canonical `{:?}` dump of each
//! card's parsed abilities instead — so the benchmark can compare a model's
//! drafted DSL against a ground-truth card by *semantic* AST equality rather than
//! brittle text matching.
//!
//! Used by the card-authoring skills (`.devin/skills/card-dsl-draft`,
//! `.devin/skills/card-author`) as the validation gate: a drafted/authored ability
//! is only accepted if it parses here.

use std::io::Read;

/// A per-card file has fields at the top level (no `[[card]]`); wrap it as a
/// one-card set document, re-nesting `[[abilities]]`-style tables under `card.`.
fn as_set_document(input: &str) -> String {
    if input.contains("[[card]]") {
        return input.to_string();
    }
    let body: String = input
        .lines()
        .map(|line| {
            if line.starts_with("[[") && !line.starts_with("[[card.") {
                line.replacen("[[", "[[card.", 1)
            } else if line.starts_with('[')
                && !line.starts_with("[card.")
                && !line.starts_with("[[")
            {
                line.replacen('[', "[card.", 1)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("[[card]]\n{body}")
}

fn main() {
    let debug = std::env::args().any(|a| a == "--debug" || a == "-d");

    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("ERROR: could not read stdin");
        std::process::exit(1);
    }
    let input = as_set_document(&input);

    match lorcana_engine::load_toml(&input) {
        Ok(defs) => {
            if debug {
                for d in &defs {
                    println!(
                        "abilities={:?} activated={:?} statics={:?} rule_statics={:?}",
                        d.abilities(),
                        d.activated_abilities(),
                        d.static_abilities(),
                        d.rule_statics(),
                    );
                }
            } else {
                println!("OK: {} card(s)", defs.len());
            }
        }
        Err(e) => {
            println!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
