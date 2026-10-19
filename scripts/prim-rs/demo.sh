#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
BIN="$ROOT/build/prim-rs/prim"
if [[ ! -x "$BIN" ]]; then
    printf 'Build first: %s/scripts/prim-rs/build.sh\n' "$ROOT" >&2
    exit 1
fi
for example in hello closures decorators jit primes; do
    printf '\n=== %s ===\n' "$example"
    "$BIN" --jit-log --stats --emit-llvm "$ROOT/build/prim-rs/ir/$example" "$ROOT/src/prim-rs/examples/$example.pr"
done
printf '\n=== explicit interpreter comparison ===\n'
"$BIN" --jit=off --stats "$ROOT/src/prim-rs/examples/jit.pr"
printf '\n=== expected runtime error ===\n'
if "$BIN" --jit-log "$ROOT/src/prim-rs/examples/error.pr"; then
    printf 'Expected the divide-by-zero example to fail\n' >&2
    exit 1
fi
