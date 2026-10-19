#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)
EXT_DIR="$ROOT_DIR/src/prim-language-server"
OUT_DIR="$ROOT_DIR/build/vscode"
ICON_DIR="$ROOT_DIR/icons"

uv run "$ROOT_DIR/scripts/fig_logo.py" --out "$ICON_DIR"
mkdir -p "$EXT_DIR/icons"
cp -f "$ICON_DIR"/logo.png "$ICON_DIR"/logo.svg "$ICON_DIR"/prim-light.svg "$ICON_DIR"/prim-dark.svg "$EXT_DIR/icons/"

"$SCRIPT_DIR/validate.sh"
node "$SCRIPT_DIR/sync-builtins.js"

cd "$EXT_DIR"

if ! npm ls --depth=0 >/dev/null 2>&1; then
  if [ -z "${http_proxy:-}${HTTPS_PROXY:-}${https_proxy:-}" ]; then
    echo "warning: no proxy environment detected; npm install may fail" >&2
  fi
  npm ci --no-audit --no-fund
fi

npm test

VERSION=$(node -p "require('./package.json').version")
mkdir -p "$OUT_DIR"
VSIX="$OUT_DIR/prim-$VERSION.vsix"
./node_modules/.bin/vsce package --out "$VSIX"
echo "VSIX: $VSIX"
