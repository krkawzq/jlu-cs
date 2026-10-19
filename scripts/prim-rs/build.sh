#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/prim-rs-target-${USER:-user}}"
cargo build --manifest-path "$ROOT/src/prim-rs/Cargo.toml" --release --offline --locked
mkdir -p "$ROOT/build/prim-rs"
cp "$CARGO_TARGET_DIR/release/prim" "$ROOT/build/prim-rs/prim"
printf 'Built %s\n' "$ROOT/build/prim-rs/prim"
