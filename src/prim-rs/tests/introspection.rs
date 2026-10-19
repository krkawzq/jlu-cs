use prim::{Config, Session, Value, prepare_named};
fn check(source: &str, jit: bool) -> prim::Runtime {
    let mut runtime = prepare_named(
        "inspection.pr",
        source,
        Config {
            jit_enabled: jit,
            gc_threshold: 1,
            max_steps: Some(2_000_000),
            ..Config::default()
        },
    )
    .unwrap_or_else(|e| panic!("{}", e.render("inspection.pr", source)));
    runtime
        .run()
        .unwrap_or_else(|e| panic!("{}", e.render("inspection.pr", source)));
    runtime
}
#[test]
fn ast_parse_walk_dump_and_source_segments() {
    let source = r#"
let source = "std.print(\"never\"); let f = @jit $[x](&a) { if a > 0 { a } else { 0 } }; f(2);";
let tree = std.ast.parse(source);
std.assert(tree.kind == "Program"); std.assert(std.ast.is_node(tree)); std.assert(!std.ast.is_node(42));
std.assert(std.len(std.ast.find(tree, "Closure")) == 1);
std.assert(std.len(std.ast.find(tree, "Capture")) == 1);
std.assert(std.len(std.ast.find(tree, "Parameter")) == 1);
std.assert(std.len(std.ast.find(tree, "Call")) == 2);
let node = std.get(std.ast.find(tree, "Closure"), 0);
std.assert(std.ast.source_segment(source, node) == "$[x](&a) { if a > 0 { a } else { 0 } }");
std.assert(std.text.contains(std.ast.dump(tree), "operator=\">\""));
std.assert(std.get(std.ast.walk(tree), 0) == tree);
let tokens = std.ast.tokens("let x = \"你好\\n\";");
std.assert(std.get(tokens, 0).kind == "Keyword");
std.assert(std.get(tokens, 3).value == "你好\n");
std.assert(std.get(tokens, 5).kind == "Eof");
"#;
    for jit in [false, true] {
        let rt = check(source, jit);
        assert_eq!(rt.output, "");
        assert_eq!(rt.stats.compiled_functions, 0);
    }
}
#[test]
fn function_signature_capture_and_identity() {
    for jit in [false, true] {
        check(
            r#"
let x = 2; let y = 3;
let f = $[x, &y](a, &b) { a + b + x + y };
std.assert(std.inspect.is_callable(f)); std.assert(std.inspect.is_function(f));
std.assert(!std.inspect.is_builtin(f)); std.assert(!std.inspect.is_jit(f));
std.assert(std.inspect.is_builtin(std.print)); std.assert(!std.inspect.is_callable(@{}));
std.assert(std.inspect.describe(f).kind == "function");
std.assert(std.inspect.describe(1).type == "int");
let signature = std.inspect.signature(f);
std.assert(signature.arity == 2); std.assert(std.get(signature.parameters, 1).by_ref);
std.assert(std.get(signature.parameters, 0).name == "a");
std.assert(std.inspect.signature(std.array).arity == 2);
let captures = std.inspect.captures(f);
std.assert(std.get(captures, 0).mode == "value"); std.assert(std.get(captures, 1).mode == "slot");
y = 9; std.assert(std.get(std.inspect.captures(f), 1).value == 9);
std.assert(std.get(captures, 1).value == 3);
let g = jit(f);
std.assert(std.get(std.inspect.captures(f), 1).slot_id == std.get(std.inspect.captures(g), 1).slot_id);
std.assert(std.inspect.describe(f).code_id == std.inspect.describe(g).code_id);
std.assert(std.inspect.describe(f).closure_id != std.inspect.describe(g).closure_id);
std.assert(std.get(std.inspect.members(f), 0).name == "x");
std.assert(std.get(std.inspect.members(std), 0).writable == false);
std.assert(std.inspect.source(f) == "$[x, &y](a, &b) { a + b + x + y }");
std.assert(std.inspect.location(f).file == "inspection.pr");
std.assert(std.len(std.ast.find(std.inspect.ast(f), "Parameter")) == 2);
std.assert(std.text.contains(std.inspect.core_ir(f), "Binary"));
std.assert(std.inspect.source(std.print) == ());
std.assert(std.len(std.inspect.builtins()) > 140);
"#,
            jit,
        );
    }
}
#[test]
fn compile_without_execution_then_inspect_real_native_bytes() {
    let rt = check(
        r#"
let effect = 0;
let f = @jit $[&effect](x) { effect = effect + 1; x * 2 };
let cold = std.inspect.jit(f);
std.assert(cold.state == "uncompiled"); std.assert(cold.address == ());
std.assert(std.inspect.compile(f) == f); std.assert(effect == 0);
let ready = std.inspect.jit(f);
std.assert(ready.compiled); std.assert(ready.will_use_native); std.assert(ready.native_calls == 0);
std.assert(ready.code_size > 0); std.assert(ready.metadata_error == ());
std.assert(ready.arity == 1); std.assert(ready.capture_count == 1);
std.assert(ready.registers > 0); std.assert(ready.instruction_count > 0);
std.assert(ready.ir_bytes == std.len(std.inspect.llvm_ir(f))); std.assert(ready.data_layout != ());
let native = std.inspect.native(f);
std.assert(native.origin == "loaded-memory"); std.assert(native.size == ready.code_size);
std.assert(std.len(std.text.split(native.hex, " ")) == native.size);
std.assert(native.hex == std.inspect.machine_code(f));
std.assert(native.assembly == std.inspect.disassemble(f));
std.assert(std.text.contains(native.assembly, "ret"));
std.assert(std.text.contains(std.inspect.llvm_ir(f), "llvm.smul.with.overflow"));
std.assert(std.text.contains(std.inspect.llvm_ir(f), "define i32"));
std.assert(f(5) == 10); std.assert(effect == 1);
std.assert(ready.native_calls == 0); std.assert(std.inspect.jit(f).native_calls == 1);
std.assert(std.inspect.jit(f).native_integer_ops == 2);
"#,
        true,
    );
    assert_eq!(rt.stats.compiled_functions, 1);
    assert_eq!(rt.stats.native_calls, 1);
}
#[test]
fn code_is_shared_but_environments_and_counters_are_explicit() {
    check(
        r#"
let make = $[](n) { @jit $[n](x) { x + n } };
let a = make(2); let b = make(10);
std.inspect.compile(a);
std.assert(std.inspect.jit(b).compiled);
std.assert(std.inspect.jit(a).address == std.inspect.jit(b).address);
std.assert(a(1) == 3); std.assert(b(1) == 11);
std.assert(std.inspect.jit(a).native_calls == 2);
std.assert(std.get(std.inspect.captures(a), 0).value == 2);
std.assert(std.get(std.inspect.captures(b), 0).value == 10);
"#,
        true,
    );
}
#[test]
fn stacks_include_current_mode_and_source() {
    for jit in [false, true] {
        let source = format!(
            r#"
let f = @jit $[](n) {{ if n == 0 {{ std.inspect.stack() }} else {{ self(n - 1) }} }};
let frames = f(2); std.assert(std.len(frames) == 4);
let last = std.get(frames, 3); std.assert(last.mode == "{}");
std.assert(last.location.file == "inspection.pr"); std.assert(last.depth == 3);
"#,
            if jit { "native" } else { "interpreted" }
        );
        check(&source, jit);
    }
}
#[test]
fn inspection_has_predictable_errors_and_cannot_mutate_snapshots() {
    for (source, kind) in [
        ("std.ast.parse(\"let x;\")", "AstError"),
        ("std.ast.tokens(\"0x\")", "AstError"),
        ("std.ast.walk(@{})", "AstError"),
        ("std.inspect.signature(1)", "NotCallable"),
        ("std.inspect.jit(std.print)", "InspectError"),
        (
            "let f = @jit $[]() {}; std.inspect.machine_code(f)",
            "JitNotCompiled",
        ),
        ("let f = $[]() {}; std.inspect.compile(f)", "JitTargetError"),
        (
            "let tree = std.ast.parse(\"1\"); tree.kind = \"bad\";",
            "ReadOnlySlot",
        ),
        (
            "let tree = std.ast.parse(\"1\"); std.seq.fill(tree.children, ());",
            "ReadOnlySlot",
        ),
        (
            "std.ast.source_segment(\"\",std.ast.parse(\"1\"))",
            "AstError",
        ),
    ] {
        let mut rt = prim::prepare(source, Config::default()).unwrap();
        assert_eq!(rt.run().unwrap_err().kind, kind, "{source}");
    }
    let mut rt = prim::prepare(
        "let f = @jit $[]() {}; std.inspect.compile(f);",
        Config {
            jit_enabled: false,
            ..Config::default()
        },
    )
    .unwrap();
    assert_eq!(rt.run().unwrap_err().kind, "JitDisabled");
}
#[test]
fn malformed_cycles_and_deep_inputs_are_bounded() {
    let source = r#"let children = std.array(1, ()); let n = @{ let kind = "Fake"; let span = @{}; let attributes = @{}; let children = children; }; std.set(children, 0, n); std.ast.walk(n);"#;
    let mut rt = prim::prepare(source, Config::default()).unwrap();
    assert_eq!(rt.run().unwrap_err().kind, "AstError");
    for source in [
        "@a ".repeat(500) + "$[]() {}",
        "if true {} else ".repeat(500) + "{}",
    ] {
        let error = prim::syntax::parse(prim::syntax::lex(&source).unwrap()).unwrap_err();
        assert_eq!(error.kind, "SyntaxError");
    }
}
#[test]
fn source_is_retained_once_and_locations_survive_repl_inputs() {
    let mut s = Session::new(Config {
        gc_threshold: 1,
        ..Config::default()
    });
    s.evaluate("original.pr", "let f = @jit $[](x) { x + 1 };")
        .unwrap();
    s.evaluate(
        "caller.pr",
        "std.assert(std.inspect.location(f).file == \"original.pr\"); std.inspect.compile(f);",
    )
    .unwrap();
    let value = s.evaluate("query.pr", "std.inspect.source(f)").unwrap();
    let Value::Str(source) = value else { panic!() };
    assert_eq!(source.as_str(), "$[](x) { x + 1 }");
    assert_eq!(s.runtime.program.sources.len(), 3);
    s.reset();
    assert!(s.runtime.program.sources.is_empty());
}
#[test]
fn metadata_snapshots_are_collectable_under_pressure() {
    check(
        r#"
let f = @jit $[](x) { x * x }; std.inspect.compile(f);
let baseline = std.gc.stats().slots;
let i = 0;
loop {
    if i == 30 { break; };
    { let tree = std.inspect.ast(f); let dump = std.ast.dump(tree); let assembly = std.inspect.disassemble(f); let captures = std.inspect.captures(f); std.assert(std.len(dump) > 0); std.assert(std.len(assembly) > 0); };
    std.gc.collect(); std.assert(std.gc.stats().slots - baseline < 32);
    i = i + 1;
};
"#,
        true,
    );
}

#[test]
fn failed_compilation_state_remains_queryable_in_a_session() {
    let path = std::env::temp_dir().join(format!("prim-inspect-failure-{}", std::process::id()));
    std::fs::write(&path, "file").unwrap();
    let mut s = Session::new(Config {
        ir_dir: Some(path.clone()),
        ..Config::default()
    });
    s.evaluate(
        "definition.pr",
        "let f = @jit $[]() { std.print(\"body\"); };",
    )
    .unwrap();
    assert_eq!(
        s.evaluate("compile.pr", "std.inspect.compile(f)")
            .unwrap_err()
            .kind,
        "JitCompileError"
    );
    s.evaluate("query.pr","let info = std.inspect.jit(f); std.assert(info.state == \"failed\"); std.assert(!info.compiled); std.assert(info.error != ());").unwrap();
    assert!(s.runtime.output.is_empty());
    std::fs::remove_file(path).unwrap();
}
