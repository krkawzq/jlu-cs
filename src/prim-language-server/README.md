# Prim Language for VS Code

A VS Code extension for Prim Core 1 with `.pr` source and `.pri` type-interface support, TextMate highlighting, a standalone Node.js language server, basic editor configuration, and file icons. Requires VS Code 1.82 or later; after installation it needs no Rust, LLVM, or Prim executable.

## Language Service

| Feature | Scope |
| --- | --- |
| Go to definition `F12` | Local variables, parameters, explicit captures, and statically recognizable object fields |
| Go to type definition | From an inferable object instance or alias to its constructor |
| Find references `Shift+F12` | Match by binding identity, distinguishing same-name variables and different object fields |
| Rename `F2` | Update declarations, capture chains, and resolved references together; reparse the edited source and reject renames that change name resolution |
| Hover | Signatures, reference parameters, approximate types, known fields, and standard library documentation |
| Completion | Currently visible bindings, keywords, `std.`, and fields of inferable objects; capture lists and decorators filtered by context |
| Snippets | `closure`, `object`, `ifelse`, `loop`, `let`, with Tab moving through placeholders |
| Signature help | User closures, `@jit` closures, and standard-library calls, showing the active argument and `&` parameters |
| Inlay hints | Approximate variable types, call parameter names, and reference modes; constructor type hints are navigable |
| Quick fixes `Ctrl+.` | Add missing captures or semicolons, or prefix unused bindings with `_` |
| Symbols and folding | Hierarchical outline, read/write highlights for the current symbol, code-block and block-comment folding, and progressive selection expansion |
| Semantic highlighting | Separate categories for type constructors, functions, parameters, variables, fields, and namespaces |
| `.pri` type interfaces | Parameter, return, and field types; union types; type aliases; same-name script association; and cross-file definition navigation |

Core 1 has no module imports; runtime name resolution does not treat same-name variables in other files as global declarations. A closure can see only explicit captures, parameters, local bindings, and reserved names. Type interfaces do not create runtime names either.

## `.pri` Type Interfaces

`.pri` is similar to `.pyi`: it describes interfaces only and does not execute code. Basic syntax:

```prim-interface
type Point = @{ let x: float; let y: float; };
let length_squared: $[](point: Point) -> float;
let optional_text: str | unit;
```

A sibling `foo.pri` provides hints for the actual top-level bindings of `foo.pr`. Types in the interface are used for parameter completion inside function bodies, signatures, hover, inlay hints, and type-definition navigation. `builtin/types.pri`, `std.pri`, and `jit.pri` in the repository root define the standard library interfaces; the build validates them and copies them into the extension. Editing an interface re-analyzes related open documents.

See [type-hints.md](../../docs/type-hints.md) for the full syntax, loading precedence, validation, and limits. Runnable paired examples are `examples/type_hints.pr` and `type_hints.pri`. Cross-interface rename is not automatic yet; when changing a binding name in both an interface and its implementation, update both sides.

While typing, an unclosed call keeps its signature and argument positions, and an unclosed function body still offers parameter completion at trailing whitespace. Completion does not appear inside strings or comments; completing in the middle of a word replaces the whole identifier instead of leaving a suffix.

## Quick Fixes and Inlay Hints

When a capture is missing, `Ctrl+.` offers both "capture value `x`" and "share slot `&x`" actions and does not choose the semantics for the user. The semicolon fix inserts `;` before a trailing line comment. The unused-binding fix performs a rename, preserving the initializer and its side effects rather than deleting the statement. Every fix carries the document version.

Type and parameter hints appear only in the editor and never insert type annotations that Prim does not support. They can be disabled separately:

```json
{
  "prim.inlayHints.types": false,
  "prim.inlayHints.parameters": false
}
```

The editor's own `editor.inlayHints.enabled` setting still applies. Clients that support the hint-refresh protocol refresh after configuration changes without restarting the server.

## Type Colors

Prim `.pr` has no class or type-declaration syntax, while type declarations and type references in `.pri` use the type color. For `.pr` constructors such as `Point` and `Counter`—**uppercase names that are statically recognizable as returning data closures**—declarations and references also use the standard LSP `type` token, mapped to `entity.name.type.prim`, and the current theme decides the actual type color. Object instances still use `variable`, while parameters and fields use `parameter` and `property`. Ordinary uppercase numeric variables are not mislabeled as types.

```prim
let Point = $[](x, y) {
    @{ let x = x; let y = y; }
};
let p = Point(1, 2);
std.print(p.x);
```

Semantic highlighting is enabled for Prim by default. User settings can override this default; if type colors are missing, check `editor.semanticHighlighting.enabled` and use VS Code's "Developer: Inspect Editor Tokens and Scopes" to verify that `Point` is `type`. Colors follow the theme and are not hard-coded.

## Errors and Warnings

- **Error**: illegal characters, numbers, or escapes; unterminated strings or comments; syntax errors; missing semicolons; chained comparisons; undefined names; missing captures; duplicate bindings; shadowed reserved names; invalid assignment or reference places; invalid `self`; control transfers across boundaries; and numeric literals out of range.
- **Warning**: unread local bindings and parameters, shadowed outer names, unreachable code after a direct `return`/`break`/`continue`, and argument-count, reference-mode, and field-name issues when the signature or field set is known. Public object fields and capture fields are not treated as unused locals; a `_` prefix suppresses the unused warning.

Set `"prim.diagnostics.warnings": false` to disable warnings while errors remain visible. Diagnostics update after a short debounce while typing and are cleared when the document closes.

The server performs static analysis only and **does not execute programs, decorators, or JIT code**. Dynamic types, fields of unknown objects, dynamic call-argument matching, arithmetic, and runtime errors remain the Prim runtime's responsibility. Assignment or reference can change a binding's value, so the server conservatively abandons related type and field inference by declaration identity without affecting same-name bindings in other scopes. Arbitrary user decorators can change signatures and cannot reuse the original signature the way the built-in `jit` does. There is no complete cross-function side-effect analysis, so unknown argument passing and dynamic mutation can invalidate static type and signature approximations. Field rename covers only statically resolved accesses and cannot track objects passed through unknown parameters; use the rename preview to inspect such dynamic accesses.

Editor syntax and static semantics follow the current `src/prim-rs/src/syntax.rs` and `ir.rs`, with additional error recovery needed for typing. The interpreter's field-slot permission check is a runtime check that the editor surfaces as a `ReadOnlySlot` warning; illegal writes to reserved names themselves remain static errors. When the editor's inference budget is exhausted it also emits only a warning and does not mark a valid program as a syntax error.

`$name(args)`, parameter and return type annotations, list literals, bracket indexing, and labeled loops are not part of the current `prim-rs` syntax. Named functions are written `let name = $[](args) { ... };`; arrays use `std.array`, `std.get`, and `std.set`. The root `examples/` directory already uses these forms.

## Alignment with prim-rs

Standard library names and arities are generated automatically from `registry!` in `src/prim-rs/src/builtins.rs`, covering the current 167 functions and the `text`, `math`, `bits`, `seq`, `random`, `time`, `env`, `process`, `fs`, `term`, `sys`, `ast`, `inspect`, and `gc` modules. Types and descriptions come from `builtin/*.pri` in the repository root; the build script validates names, arities, and value-parameter modes and syncs the bundled copy. After adding a standard library function and updating `.pri`, run:

```bash
node scripts/prim-language-server/sync-builtins.js
```

Syntax parity tests compile and invoke the current Rust lexer, parser, and IR compiler directly; they do not duplicate the Rust grammar and do not execute the program under test. Tests compare acceptance and AST structure across real examples, source from Rust tests, and operator combinations. Source hashes are retained to detect source changes during verification.

Run from the repository's absolute path on a valid CPU worker:

```bash
bash scripts/prim-language-server/check-parity.sh
PRIM_FRONTEND_PROBE="$PWD/build/vscode/frontend-parity/frontend-probe" \
  npm test --prefix src/prim-language-server
```

Plain `npm test` does not compile Rust; without `PRIM_FRONTEND_PROBE` it skips only the real Rust parity items while all other feature tests still run.

## Covered Syntax

- Keywords: `let`, `if`, `else`, `loop`, `break`, `continue`, `return`, `true`, `false`, `self`
- Callable closures: `$[captures](parameters) { body }`
- Data closures: `@{ ... }`
- Decorators: `@name`, `@jit`
- Explicit captures and references: `$[x]`, `$[&x]`, `$[](x, &y)`
- Field access: `obj.field`, `std.print(...)`
- Numbers: decimal, `0x`, `0o`, `0b`, underscore grouping, and floating-point exponents
- Strings: double quotes and escapes such as `\n` and `\u{...}`
- Comments: `//` and `/* ... */`
- File icons: the same mark as the extension logo, with light and dark SVGs

## Logo

The logo is a `P` assembled from the closure model: the ring is the environment, the diamond inside the ring is a value in a slot (slot and value are separate, and the diamond does not touch the ring), and the vertical stroke is the code entry, shaped like the slash in a terminal `/` with a gap from the ring because code is the optional part of a closure. The ring alone is an object; the ring plus the stroke is a function. The amber color comes from vintage amber-phosphor terminals: code and values are amber, and the environment is warm white.

`logo.png`, `logo.svg`, `prim-light.svg`, and `prim-dark.svg` are generated into the repository root `icons/` by `scripts/fig_logo.py`. The build script regenerates them every time and copies them over the extension's `icons/`; modify the script rather than the copies inside the extension. To preview them alone, run from the repository root:

```bash
uv run scripts/fig_logo.py
```

## Build

Requires Node.js, npm, and uv (for icon generation). Run from the repository root:

```bash
scripts/prim-language-server/build.sh
```

The script generates and copies icons, syncs standard library signatures and `.pri`, installs dependencies from the lockfile, runs frontend and feature tests, the real stdio/IPC LSP lifecycle tests, and the tokenizer smoke test, and writes a VSIX with runtime dependencies to `build/vscode/prim-0.4.0.vsix`. The default build does not compile or execute Prim programs.

To run only the lightweight tests:

```bash
cd src/prim-language-server
npm test
```

Other LSP-capable editors can start the server over stdio:

```bash
node /absolute/path/to/prim-language-server/server/server.js --stdio
```

## Local Installation

```bash
scripts/prim-language-server/install.sh
```

The script builds the VSIX and tries to install it with the `code`, `codium`, or `code-server` CLI. If none of these CLIs is available, it prints the VSIX path; in VS Code, use "Extensions → ... → Install from VSIX".

## Directory

```text
prim-language-server/
├── package.json
├── extension.js                  # VS Code LSP client
├── server/
│   ├── server.js                 # LSP connection, document versions, and diagnostic publishing
│   ├── syntax.js                 # Recoverable lexer and parser
│   ├── analysis.js               # Scopes, bindings, static diagnostics, and approximate types
│   ├── builtins.js               # Standard library modules, signatures, and docs
│   ├── interfaces.js             # .pri parsing and type resolution
│   ├── hints.js                  # Same-name interfaces, workspace interfaces, and unsaved content
│   ├── builtin-signatures.json   # Generated from prim-rs registry!
│   └── features.js               # Navigation, completion, rename, and semantic tokens
├── language-configuration.json
├── builtin/                     # Bundled copy of the root builtin/
├── icons/
│   ├── logo.png / logo.svg      # Extension icon (overwritten from the repository icons/ during build)
│   ├── prim-light.svg
│   └── prim-dark.svg
├── syntaxes/prim.tmLanguage.json
├── syntaxes/pri.tmLanguage.json
└── test/
    ├── sample.pr
    ├── analysis.test.js
    ├── alignment.test.js
    ├── editing.test.js
    ├── interfaces.test.js
    ├── parity.test.js
    ├── frontend-probe.rs
    ├── protocol.test.js
    └── tokenize.js
```
