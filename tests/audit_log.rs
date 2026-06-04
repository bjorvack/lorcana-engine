//! Behaviour audit log (feature `audit-log`): the whole file compiles only when
//! the feature is enabled, so a release build (without it) carries nothing.
#![cfg(feature = "audit-log")]

use std::path::Path;

#[test]
fn audit_log_pairs_card_text_with_events() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let log = lorcana_engine::application::audit::audit_from_files(
        &root.join("cards/sets"),
        &root.join("decks/set01-amber-amethyst.txt"),
        &root.join("decks/set01-emerald-ruby.txt"),
        7,
        4_000,
    )
    .expect("audit runs");

    assert!(log.contains("Game over:"), "transcript ends with game-over");
    assert!(
        log.contains("text:"),
        "at least one action is annotated with the acting card's printed text"
    );
    assert!(log.contains("->"), "events are rendered");
    // Card ids must be resolved to names, not left as raw CardId(n) tokens.
    assert!(
        !log.contains("CardId("),
        "all card ids should be resolved to names"
    );
    assert!(
        !log.contains("REJECTED"),
        "no legal-action/apply disagreement during the audited game"
    );
}
