# Prim Rust Implementation

A single-threaded dynamic language interpreter with an explicit LLVM ORC JIT. The source lives in this directory; the legacy C++ frontend is kept in `../../src.bak/`. It needs no re2c/Bison, uses no third-party Rust crates, and does not build LLVM from source.

Prim source files use the `.pr` extension, and the executable is named `prim`.

## Build and Run

Requires Rust 1.91+, Cargo, the system LLVM 14 shared library, and `llvm-config`. The project is verified on a CPU pod with Linux x86_64, Rust 1.95, and LLVM 14.0.0. The build script accepts LLVM 14–18 and selects the pointer IR dialect by version; other versions are untested and not covered by the delivery guarantee.

Run on a CPU worker with the environment above:

```bash
cd /mnt/shared-storage-user/dnacoding/wangzhongqi/Code/Project/Prim
scripts/prim-rs/build.sh
build/prim-rs/prim --version
build/prim-rs/prim --jit-log --stats src/prim-rs/examples/jit.pr
scripts/prim-rs/demo.sh
```

`build.sh` places Cargo intermediates in `/tmp/prim-rs-target-$USER` on the worker and copies the release binary to `build/prim-rs/prim`. You can also use Cargo directly:

```bash
cd src/prim-rs
CARGO_TARGET_DIR=/tmp/prim-rs-target cargo build --release --offline --locked
CARGO_TARGET_DIR=/tmp/prim-rs-target cargo test --release --offline --locked
```

Select LLVM with `LLVM_CONFIG=/path/to/llvm-config`. The binary links against `libLLVM-<version>.so` rather than a static LLVM library and does not spawn an external Clang compilation subprocess. The runtime machine needs the matching shared library; check with `ldd build/prim-rs/prim`.

In the cluster, builds, full tests, and demos all run on a valid CPU pod. Rediscover the address with `list-worker.py --ssh`; do not write expiring SSH addresses into scripts. The project has no downloaded dependencies, so the current build is fully offline. If network dependencies are added later, reverse-proxy the network from the development box as the cluster rules require.

## Minimal Example

```prim
let Counter = $[](initial) {
    @{
        let value = initial;
        let next = @jit $[&value](step) {
            value = value + step;
            value
        };
    }
};
let counter = Counter(0);
let next = counter.next;
std.print(next(2));
counter.value = 40;
std.print(next(1));
```

It prints `2` and `41`. After `next` is extracted from the object it still holds the shared slot; the first call triggers compilation through `@jit`, and later calls reuse the machine code.

## Implemented

- UTF-8 strings, Unicode escapes, decimal/binary/octal/hex integers, floats, booleans, and unit.
- Lexical scoping, explicit value and slot captures, aliases, field reads and writes, and fixed field sets.
- Data closures, callable closures, higher-order functions, `self` recursion, reference parameters, and capture escape.
- Block tail values, strict bool conditions, short-circuiting, loops, break/continue/return.
- General decorators, manual `jit(f)` / `@jit`, with no hot-spot detection.
- 168 built-in interfaces covering stdout/stderr and line or whole-stream input, fixed arrays and sequences, time and UTC calendar, random numbers, text and math, bit operations, files and directories, process and system information, terminal drawing/keys/colors, AST parsing, runtime inspection, and GC.
- Persistent REPL, repeated `-e`, post-execution interaction, structure inspection subcommands, JSON diagnostics, and resource limits.
- A complete terminal Snake game plus verification of real PTY key input and terminal restoration.
- Source locations, call stacks, static name resolution, dynamic errors, and execution resource limits.
- Non-moving mark-sweep GC for slots and closures that reclaims cyclic graphs.
- LLVM IR parsing and verification, ORC native code generation, in-process function caching, and interpreter/JIT interop.
- JIT generation of branches/loops and overflow-checked integer add, subtract, and multiply; when dynamic types miss the fast path, the same runtime operations preserve dynamic semantics.

Not provided: list, dictionary, and tuple literals; classes; type annotations; operator overloading; and module imports. Fixed arrays are provided by built-in data closures.

## CLI

See [docs/cli.md](../../docs/cli.md) for the full manual and [prim-builtin.md](../../docs/prim-builtin.md) for built-in interfaces.

```bash
prim --help
prim program.pr -- script_arguments
prim -e 'let x = 1;' -e 'std.print(x + 1);'
prim --result -e '21 * 2'
prim repl
prim -i program.pr
prim check program.pr
prim lex program.pr
prim ast program.pr
prim ir program.pr
prim --diagnostic-format=json -e '1 / 0'
prim --jit-log --stats --time --emit-llvm build/ir program.pr
prim --no-jit --stats program.pr
prim --fuel 1000000 --stack-limit 128 --gc-threshold 128 program.pr
cat program.pr | prim -
```

With no arguments, Prim enters the REPL on a terminal and executes stdin when piped. The REPL keeps variables and closures and does not replay history; it provides `:help/:vars/:del/:load/:reset/:stats/:gc/:quit`.

`check/lex/ast/ir` do not execute user code. `--jit=off` only disables manual compilation explicitly; by default there is no fallback after a failure. `--stats` reports cumulative statistics, and `--time` reports wall-clock time for each input. Exit codes: 0 success, 1 language error, 2 usage or input-file error.

## Presentation Demos

| Program | What it shows |
| --- | --- |
| `examples/hello.pr` | Minimal language and strings |
| `examples/closures.pr` | Unified objects and functions, extracted methods, shared slots, shared code with independent environments |
| `examples/decorators.pr` | Decorator composition order and manual compilation |
| `examples/jit.pr` | Loop sum, recursion, compile once and execute many times |
| `examples/primes.pr` | Complete prime program with 25 primes below 100 |
| `examples/error.pr` | Native execution error location with side effects not replayed; expected failure |
| `examples/interactive.pr` | Prompts, line input, and text parsing |
| `examples/snake.pr` | Playable terminal Snake; `-- --self-test` verifies the logic |
| `examples/memory.pr` | Reclamation after scope exit and 500 rounds of loop allocation |

`../../scripts/prim-rs/demo.sh` runs these programs automatically, prints statistics, saves LLVM IR, and treats the final error example as an expected failure.

Optimization levels `-O0`, `-O1`, and `-O2` (default) select generic lowering or local scalar lowering plus LLVM optimization passes. Non-escaping scalar loops can keep locals in SSA; shared captures and calls retain the generic runtime path. `--fuel` uses the instruction-checked generic path. See [optimization and CPU measurements](../../docs/optimization.md).

`std.inspect.optimized_ir(f)` exposes the exact module submitted to ORC; `std.inspect.llvm_ir(f)` exposes the pre-optimization text. `--emit-llvm` saves both `.ll` and `.opt.ll`.

The generated IR shows `br`, `switch`, `llvm.sadd.with.overflow.i64`, `llvm.ssub.with.overflow.i64`, and `llvm.smul.with.overflow.i64`. `native_integer_ops` records the native arithmetic fast paths actually taken; an undecorated function call is never compiled no matter how many times it runs.

The first call includes compilation cost. The current version prioritizes semantic completeness, with many runtime helpers and type checks, and does not claim to be faster than the interpreter. When presenting performance, report compilation cost and subsequent execution time separately.

## Implementation Structure

| File | Responsibility |
| --- | --- |
| `src/syntax.rs` | Lexer, AST, Pratt parser, source diagnostics |
| `src/ir.rs` | Name resolution, capture layout, place and control-target checks, register IR |
| `src/runtime.rs` | Dynamic values, environments, slots, GC, standard library, interpreter, call dispatch |
| `src/jit.rs` | LLVM C API, resource ownership, IR generation, ORC compilation, native callbacks |
| `src/main.rs` | CLI, modes and flags, output, statistics, execution limits |
| `src/session.rs` | Incremental REPL, multi-input sessions, cross-input diagnostics |
| `src/builtins.rs` / `src/terminal.rs` | Standard library, termios and key input, terminal restoration |
| `src/liveness.rs` | Control-flow live-register root analysis |
| `tests/language.rs` | Interpreter/JIT differential and lifetime tests |

Closures and slots are connected by integer handles, and machine code never holds internal addresses of Rust containers. The live registers at the current PC of the active Frame, the current closure, and the return value are GC roots; interactive sessions also retain explicit global bindings. JIT callback boundaries catch Rust panics, language errors use explicit status codes, and unwinding cannot cross FFI. The ORC engine owns machine code until the Runtime is destroyed.

## Current Engineering Boundaries

- The frontend stops at the first error; the multi-error recovery described in the spec for IDEs is not implemented.
- GC uses CFG live roots, clears dead registers, and shrinks trailing arena holes. REPL source, code, and global bindings are retained per session; `:del/:reset` release the corresponding state. See [memory-management.md](../../docs/memory-management.md) for the detailed audit.
- JIT provides only an in-process cache; there is no persistent cache, cross-process machine-code format, or dynamic unloading.
- Numeric fast paths currently cover integer add, subtract, and multiply; floats, division and modulo, and object operations share runtime helpers.
- Verified environment: Linux x86_64 + LLVM 14; other platforms are untested.

## AST / inspect / native code

详见 [内省与机器码展示](../../docs/introspection.md)。汇报示例：

```bash
prim src/prim-rs/examples/inspect.pr
```

`std.ast` 返回结构化只读语法树，`std.inspect` 查看签名、源码、捕获槽、调用栈、JIT 状态、原始 LLVM IR 与已加载机器码。查询不隐式编译；`std.inspect.compile` 预编译已标记的函数而不执行函数体。
