#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)
OUT_DIR="$ROOT_DIR/build/vscode/frontend-parity"
mkdir -p "$OUT_DIR"
cd "$ROOT_DIR"

sha256sum src/prim-rs/src/{syntax,ir,liveness,builtins}.rs > "$OUT_DIR/source.sha256"
rustc --edition=2024 src/prim-language-server/test/frontend-probe.rs -o "$OUT_DIR/frontend-probe"
PRIM_FRONTEND_PROBE="$OUT_DIR/frontend-probe" node --test src/prim-language-server/test/parity.test.js
sha256sum --check "$OUT_DIR/source.sha256"
