//! Guards that keep the generated DSL reference (`docs/dsl/EFFECT_DSL.md`) honest,
//! so it is correct "at any time" (CI fails otherwise):
//!
//! - the committed doc equals the generator output (no drift);
//! - every documented effect-verb example actually parses (no stale syntax);
//! - every verb / trigger / restriction the parser recognises is listed (no
//!   undocumented syntax).

use lorcana_engine::{dsl_reference, load_toml};
use std::fs;
use std::path::Path;

fn dsl_src() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domain/cards/dsl.rs"))
        .expect("read dsl.rs")
}

#[test]
fn committed_reference_matches_the_generator() {
    let committed =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/dsl/EFFECT_DSL.md"))
            .expect("read docs/dsl/EFFECT_DSL.md");
    assert_eq!(
        committed,
        dsl_reference::reference_markdown(),
        "docs/dsl/EFFECT_DSL.md is stale — run `cargo run --bin dsl_reference`",
    );
}

#[test]
fn every_documented_verb_example_parses() {
    for entry in dsl_reference::effect_verbs() {
        let card = format!(
            "[[card]]\nname = \"T\"\ntype = \"Action\"\ncost = 1\n\
             [[card.abilities]]\non = \"play\"\ndo = {}\n",
            entry.example,
        );
        let _ = load_toml(&card).unwrap_or_else(|e| {
            panic!(
                "verb `{}` example {:?} failed: {e:?}",
                entry.name, entry.example
            )
        });
    }
}

/// Effect-table keys the parser reads that are *arguments*, not verbs — they
/// don't get their own reference row.
const STRUCTURAL_KEYS: &[&str] = &[
    "to",
    "from",
    "target",
    "who",
    "whose",
    "duration",
    "then",
    "at_least",
    "take",
    "take_count",
    "rest",
    "reorder",
    "rest_per_card",
    "then_to",
    "apply_to",
    "optional",
    "ink",
    "exert",
    "banish",
    "while",
    "per",
];

fn verbs_documented() -> std::collections::HashSet<String> {
    dsl_reference::effect_verbs()
        .into_iter()
        .map(|e| e.name.to_string())
        .collect()
}

#[test]
fn every_parser_verb_key_is_documented() {
    let src = dsl_src();
    let documented = verbs_documented();
    // Verb keys are read via `t.contains_key("…")` / `t.get("…")` in the effect
    // dispatch; anything not a known structural argument must be a documented verb.
    let re = regex_lite(&src, "contains_key", "get");
    let mut missing = vec![];
    for key in re {
        if !STRUCTURAL_KEYS.contains(&key.as_str()) && !documented.contains(&key) {
            missing.push(key);
        }
    }
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "undocumented effect verbs in dsl.rs: {missing:?}"
    );
}

#[test]
fn triggers_restrictions_scopes_are_documented() {
    let doc = dsl_reference::reference_markdown();
    // Trigger tokens (trigger_from): `"x" => TriggerCondition::…`.
    for tok in tokens_before(&dsl_src(), "=> TriggerCondition::") {
        assert!(
            doc.contains(&format!("`{tok}`")),
            "undocumented trigger `{tok}`"
        );
    }
    // Restriction tokens (restriction_from): `"x" => Restriction::…`.
    for tok in tokens_before(&dsl_src(), "=> Restriction::") {
        assert!(
            doc.contains(&format!("`{tok}`")),
            "undocumented restriction `{tok}`"
        );
    }
}

/// Collect the verb-key string literals passed to `t.<m1|m2>("…")` in `src`.
fn regex_lite(src: &str, m1: &str, m2: &str) -> Vec<String> {
    let mut out = vec![];
    for marker in [m1, m2] {
        let pat = format!("t.{marker}(\"");
        let mut rest = src;
        while let Some(i) = rest.find(&pat) {
            rest = &rest[i + pat.len()..];
            if let Some(end) = rest.find('"') {
                out.push(rest[..end].to_string());
            }
        }
    }
    out
}

/// Collect the string literals that appear on a line containing `marker`, e.g.
/// the `"x"` / `"y"` in `"x" | "y" => Restriction::…`.
fn tokens_before(src: &str, marker: &str) -> Vec<String> {
    let mut out = vec![];
    for line in src.lines().filter(|l| l.contains(marker)) {
        let head = &line[..line.find(marker).unwrap()];
        let mut rest = head;
        while let Some(i) = rest.find('"') {
            rest = &rest[i + 1..];
            if let Some(end) = rest.find('"') {
                out.push(rest[..end].to_string());
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
    }
    out
}
