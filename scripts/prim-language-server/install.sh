#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)
EXT_DIR="$ROOT_DIR/src/prim-language-server"

"$SCRIPT_DIR/build.sh"

VERSION=$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' "$EXT_DIR/package.json")
VSIX="$ROOT_DIR/build/vscode/prim-$VERSION.vsix"

if [ -n "${PRIM_VSCODE_CLI:-}" ]; then
  "$PRIM_VSCODE_CLI" --install-extension "$VSIX" --force
elif command -v code >/dev/null 2>&1; then
  code --install-extension "$VSIX" --force
elif command -v codium >/dev/null 2>&1; then
  codium --install-extension "$VSIX" --force
elif command -v code-server >/dev/null 2>&1; then
  code-server --install-extension "$VSIX" --force
else
  echo "VSIX built: $VSIX"
  echo "Install it from VS Code: Extensions -> ... -> Install from VSIX..."
  exit 0
fi

echo "Installed: $VSIX"
