//! Regenerate `docs/dsl/EFFECT_DSL.md` — the human/AI-facing card-effect DSL
//! reference — from `lorcana_engine::dsl_reference`. Run after changing the DSL:
//!
//! ```sh
//! cargo run --bin dsl_reference
//! ```
//!
//! `tests/dsl_reference.rs` fails if the committed doc drifts from this output.

use std::path::Path;

fn main() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/dsl/EFFECT_DSL.md");
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).expect("create docs/dsl");
    }
    std::fs::write(&out, lorcana_engine::dsl_reference::reference_markdown())
        .expect("write EFFECT_DSL.md");
    println!("wrote {}", out.display());
}
