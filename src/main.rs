//! Thin CLI host for the Lorcana engine.
//!
//! Usage:
//!   cargo run                              # interactive loop (seed 7)
//!   cargo run -- demo [seed]               # auto-play a small random game
//!   cargo run -- play <d1.txt> <d2.txt> [seed]
//!                                          # auto-play a real game from two
//!                                          # decklists, using cards/sets/

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("demo") => {
            let seed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(7);
            print!("{}", lorcana_engine::application::host::demo(seed, 5_000));
        }
        Some("play") => {
            let (Some(d1), Some(d2)) = (args.get(2), args.get(3)) else {
                eprintln!("usage: play <deck1.txt> <deck2.txt> [seed]");
                std::process::exit(2);
            };
            let seed = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(7);
            match lorcana_engine::application::host::play_from_files(
                Path::new("cards/sets"),
                Path::new(d1),
                Path::new(d2),
                seed,
                10_000,
            ) {
                Ok(transcript) => print!("{transcript}"),
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                }
            }
        }
        _ => {
            let seed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(7);
            lorcana_engine::application::host::run_interactive(seed);
        }
    }
}
