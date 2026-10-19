#!/usr/bin/env bash
# Run inside an existing CPU worker, from the shared project directory.
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
cd "$ROOT"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/prim-rs-target-${USER:-user}}"
OUT="${PRIM_BENCH_DIR:-$ROOT/build/prim-rs}"
mkdir -p "$OUT"
cargo run --release --offline --locked --example benchmark -- "${1:-9}" \
  > "$OUT/optimization-benchmark.jsonl"
{
  date -u '+%Y-%m-%dT%H:%M:%SZ'
  rustc --version
  llvm-config --version
  uname -srmo
  lscpu
} > "$OUT/optimization-environment.txt"
{
  sha256sum Cargo.toml Cargo.lock "$CARGO_TARGET_DIR/release/examples/benchmark"
  rg --files -0 src/prim-rs | sort -z | xargs -0 sha256sum
} > "$OUT/optimization-sources.sha256"
printf 'Benchmark results: %s\n' "$OUT/optimization-benchmark.jsonl"
