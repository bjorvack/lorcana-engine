//! Thin CLI host for the Lorcana engine.
//!
//! Usage:
//!   cargo run                 # interactive loop (seed 7)
//!   cargo run -- demo [seed]  # auto-play a random game, printing a transcript

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(7);
    match args.get(1).map(String::as_str) {
        Some("demo") => print!("{}", lorcana_engine::application::host::demo(seed, 5_000)),
        _ => lorcana_engine::application::host::run_interactive(seed),
    }
}
