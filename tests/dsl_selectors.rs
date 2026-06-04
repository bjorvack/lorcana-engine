//! Guard: every authored selector string in `cards/sets/*.toml` must resolve to
//! real classifications / predicates — no token may silently degrade to a bogus
//! `Classification` (which would match nothing). This catches typos, unsupported
//! predicates, and — importantly — **new multi-word classifications** (e.g. a
//! future set adding one beyond "Seven Dwarfs"): authored in the compact form
//! they'd split into bogus tokens, and this test fails loudly instead.
//!
//! Mirrors `parse_filter`'s tokenization in `src/domain/cards/dsl.rs`; keep in
//! sync. When it flags a new multi-word classification, either teach the parser
//! that phrase or author the card with the structured filter form.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Words the parser consumes structurally / as predicates (mirror of dsl.rs:
/// `STRUCTURAL` + the state predicates damaged/exerted/ready).
const KNOWN: &[&str] = &[
    "all",
    "chosen",
    "another",
    "other",
    "opposing",
    "your",
    "yours",
    "mine",
    "of",
    "own",
    "character",
    "characters",
    "item",
    "items",
    "location",
    "locations",
    "permanent",
    "permanents",
    "named",
    "with",
    "and",
    "or",
    "than",
    "less",
    "fewer",
    "more",
    "greater",
    "cost",
    "strength",
    "willpower",
    "lore",
    "{s}",
    "{w}",
    "{l}",
    "damaged",
    "exerted",
    "ready",
];

fn classification_vocab(root: &Path) -> HashSet<String> {
    let mut vocab = HashSet::new();
    for entry in fs::read_dir(root.join("cards/sets")).expect("read cards/sets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read set");
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("classifications = [") {
                for raw in rest.split(',') {
                    let c = raw.trim().trim_matches([']', '"', ' ']);
                    if !c.is_empty() {
                        let _ = vocab.insert(c.to_lowercase());
                    }
                }
            }
        }
    }
    vocab
}

/// Tokens that the parser would (mis)read as a classification but that are not a
/// real classification — i.e. silent no-ops.
fn bogus_tokens(selector: &str, vocab: &HashSet<String>) -> Vec<String> {
    let toks: Vec<&str> = selector.split_whitespace().collect();
    let low: Vec<String> = toks.iter().map(|t| t.to_lowercase()).collect();
    // Multi-word classifications (lowercased token runs), longest first, mirroring
    // the parser's data-driven matching.
    let mut multiword: Vec<Vec<String>> = vocab
        .iter()
        .filter(|c| c.contains(' '))
        .map(|c| c.split_whitespace().map(str::to_string).collect())
        .collect();
    multiword.sort_by_key(|p| std::cmp::Reverse(p.len()));

    let mut bad = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &low[i];
        if t == "named" {
            // "named" consumes the (possibly multi-word) name: every following
            // token up to the next structural/numeric word.
            i += 1;
            while i < low.len()
                && !KNOWN.contains(&low[i].as_str())
                && !low[i].chars().all(|c| c.is_ascii_digit())
            {
                i += 1;
            }
            continue;
        }
        if let Some(parts) = multiword
            .iter()
            .find(|parts| low[i..].starts_with(parts.as_slice()))
        {
            i += parts.len(); // a known multi-word classification
            continue;
        }
        if KNOWN.contains(&t.as_str()) || t.chars().all(|c| c.is_ascii_digit()) {
            i += 1;
            continue;
        }
        if !vocab.contains(t) {
            bad.push(toks[i].to_string());
        }
        i += 1;
    }
    bad
}

#[test]
fn authored_selectors_have_no_silent_noop_classifications() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let vocab = classification_vocab(root);
    let keys = ["to", "target", "from", "if_you_have", "per"];

    let mut problems = Vec::new();
    for entry in fs::read_dir(root.join("cards/sets")).expect("read cards/sets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read set");
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        for line in text.lines() {
            for key in keys {
                let needle = format!("{key} = \"");
                let mut from = 0;
                while let Some(p) = line[from..].find(&needle) {
                    let start = from + p + needle.len();
                    if let Some(end) = line[start..].find('"') {
                        let sel = &line[start..start + end];
                        if ["character", "item", "location", "permanent"]
                            .iter()
                            .any(|w| sel.to_lowercase().contains(w))
                        {
                            let bad = bogus_tokens(sel, &vocab);
                            if !bad.is_empty() {
                                problems.push(format!("{file}: {sel:?} -> bogus {bad:?}"));
                            }
                        }
                        from = start + end + 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "authored selectors with tokens that silently match nothing \
         (fix the selector, or teach parse_filter a new multi-word classification):\n{}",
        problems.join("\n")
    );
}
