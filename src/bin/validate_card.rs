//! Validate a single card's TOML (including its effect-DSL abilities) through the
//! engine's real loader. Reads a TOML document from stdin; prints `OK` and exits 0
//! if every card maps, or `ERROR: <detail>` and exits 1 otherwise.
//!
//! With `--debug` (or `-d`), on success prints a canonical `{:?}` dump of each
//! card's parsed abilities instead — so the benchmark can compare a model's
//! drafted DSL against a ground-truth card by *semantic* AST equality rather than
//! brittle text matching.
//!
//! Used by the card-authoring draft loop (`.devin/skills/card-dsl-draft`) as the
//! validation gate: a model-drafted ability is only accepted if it parses here.

use std::io::Read;

fn main() {
    let debug = std::env::args().any(|a| a == "--debug" || a == "-d");

    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("ERROR: could not read stdin");
        std::process::exit(1);
    }

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
