#!/usr/bin/env bash
# Build the `lorcana-wasm` crate into the web app's lib folder.
#
# wasm-pack invokes `cargo`, which must be the rustup-managed toolchain (it has
# the wasm32 std + the right edition), not a Homebrew `cargo`. We prepend the
# rustup bin dir so the right toolchain is used regardless of PATH ordering.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"

wasm-pack build "${repo_root}/crates/lorcana-wasm" \
  --target web \
  --out-dir "${repo_root}/web/src/lib/wasm" \
  --out-name lorcana_wasm \
  "$@"
