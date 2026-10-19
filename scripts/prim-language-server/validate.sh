#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "$SCRIPT_DIR/../.." && pwd)
EXT_DIR="$ROOT_DIR/src/prim-language-server"

python3 - "$EXT_DIR" <<'PY'
import json
import sys
from pathlib import Path

ext = Path(sys.argv[1])
for rel in ("package.json", "language-configuration.json", "syntaxes/prim.tmLanguage.json", "syntaxes/pri.tmLanguage.json"):
    path = ext / rel
    with path.open(encoding="utf-8") as handle:
        json.load(handle)
    print(f"ok: {rel}")

pkg = json.loads((ext / "package.json").read_text(encoding="utf-8"))
languages = {item["id"]: item for item in pkg["contributes"]["languages"]}
grammars = {item["language"]: item for item in pkg["contributes"]["grammars"]}
for name, extension in (("prim", ".pr"), ("prim-interface", ".pri")):
    language, grammar = languages[name], grammars[name]
    assert extension in language["extensions"]
    assert grammar["scopeName"] == f"source.{name}", grammar["scopeName"]
    assert (ext / language["configuration"]).is_file()
    assert (ext / grammar["path"]).is_file()
    for variant in ("light", "dark"):
        assert (ext / language["icon"][variant]).is_file(), language["icon"][variant]
assert (ext / pkg["icon"]).is_file(), pkg["icon"]
print("ok: contribution paths")
PY
