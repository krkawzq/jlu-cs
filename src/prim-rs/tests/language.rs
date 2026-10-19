use prim::{Config, Runtime, Value, prepare};

fn execute(source: &str, jit: bool) -> Runtime {
    let config = Config {
        jit_enabled: jit,
        gc_threshold: 1,
        max_steps: Some(1_000_000),
        ..Config::default()
    };
    let mut rt = prepare(source, config).unwrap_or_else(|e| panic!("{}", e.render("test", source)));
    rt.run()
        .unwrap_or_else(|e| panic!("{}", e.render("test", source)));
    rt
}
fn both(source: &str) {
    let a = execute(source, false);
    let b = execute(source, true);
    assert_eq!(a.output, b.output);
}

#[test]
fn values_aliases_and_objects() {
    both(
        r#"
let a = 1; let b = a; let c = &a; a = 2;
std.assert(b == 1); std.assert(c == 2); c = 3; std.assert(a == 3);
let p = @{ let x = 1; }; let q = p; let alias = &p;
q.x = 7; std.assert(p.x == 7); p = @{ let x = 9; };
std.assert(q.x == 7); std.assert(alias.x == 9); std.assert(p != q);
std.assert({} == ()); std.assert(@{} != @{});
"#,
    );
}

#[test]
fn closure_snapshots_and_shared_state() {
    both(
        r#"
let x = 10;
let snapshot = @jit $[x]() { x };
let shared = @jit $[&x]() { x };
x = 20; std.assert(snapshot() == 10); std.assert(shared() == 20);
let n = 0; let next = @jit $[n]() { n = n + 1; n };
std.assert(next() == 1); std.assert(next() == 2); std.assert(n == 0);
next.n = 10; std.assert(next() == 11);
"#,
    );
}

#[test]
fn stateful_counter_and_native_code_sharing() {
    let source = r#"
let Counter = @jit $[](initial) {
    @{ let value = initial;
       let next = @jit $[&value](step) { value = value + step; value };
       let read = $[&value]() { value };
    }
};
let a = Counter(0); let b = Counter(100); let next = a.next;
std.assert(next(2) == 2); std.assert(a.read() == 2);
std.assert(b.next(3) == 103); std.assert(a.next.value == 2);
a.value = 40; std.assert(next(1) == 41); std.assert(b.read() == 103);
"#;
    let rt = execute(source, true);
    assert_eq!(rt.stats.compiled_functions, 2);
    assert_eq!(rt.stats.native_calls, 5);
    execute(source, false);
}

#[test]
fn recursion_and_mixed_execution() {
    both(
        r#"
let factorial = @jit $[](n) { if n == 0 { 1 } else { n * self(n - 1) } };
std.assert(factorial(10) == 3628800);
let add = $[](x) { x + 1 };
let apply = @jit $[add](n) { add(n) };
std.assert(apply(4) == 5);
let call = $[](f, x) { f(x) };
std.assert(call(factorial, 5) == 120);
"#,
    );
}

#[test]
fn all_control_flow_paths() {
    both(
        r#"
let sum = @jit $[](n) {
    let i = 0; let total = 0;
    loop {
        if i == n { break total; };
        i = i + 1;
        if i == 3 { continue; };
        total = total + i;
    }
};
std.assert(sum(5) == 12);
let early = @jit $[](n) { { if n > 0 { return n; }; }; 0 };
std.assert(early(3) == 3); std.assert(early(-1) == 0);
let nested = @jit $[]() { loop { let n = loop { break 3; }; break n + 2; } };
std.assert(nested() == 5);
"#,
    );
}

#[test]
fn short_circuit_preserves_side_effects() {
    both(
        r#"
let n = 0; let effect = $[&n]() { n = n + 1; true };
let run = @jit $[effect]() {
    std.assert(!(false && effect())); std.assert(true || effect());
    std.assert(true && effect()); std.assert(false || effect());
};
run(); std.assert(n == 2);
"#,
    );
}

#[test]
fn reference_parameters_escape() {
    both(
        r#"
let make = @jit $[]() {
    let n = 1;
    let capture = @jit $[](&x) { @jit $[&x]() { x = x + 1; x } };
    capture(&n)
};
let f = make(); std.assert(f() == 2); std.assert(f() == 3);
let g = @jit $[](&target, delta) { target = target + delta; };
let obj = @{ let n = 3; }; g(&obj.n, 4); std.assert(obj.n == 7);
"#,
    );
}

#[test]
fn decorators_and_jit_identity() {
    both(
        r#"
let traced = $[](f) { $[f](x) { std.print("call"); f(x) } };
let a = @jit @traced $[](x) { x * 2 };
let b = @traced @jit $[](x) { x * 3 };
std.assert(a(3) == 6); std.assert(b(3) == 9);
let n = 0; let original = $[n]() { n = n + 1; n };
let compiled = jit(original);
std.assert(original != compiled); std.assert(jit(compiled) == compiled);
std.assert(compiled() == 1); std.assert(original() == 2); std.assert(compiled.n == 2);
let identity = @jit $[]() { self }; std.assert(identity() == identity);
"#,
    );
}

#[test]
fn decoration_order_and_callee_snapshot() {
    both(
        r#"
let state = @{ let text = ""; };
let a = $[state](f) { state.text = state.text + "a"; f };
let b = $[state](f) { state.text = state.text + "b"; f };
let f = @a @b $[]() { 1 };
std.assert(state.text == "ba"); std.assert(f() == 1);
let original = $[](x) { 1 }; let target = original;
let change = $[&target]() { target = $[](x) { 2 }; 0 };
std.assert(target(change()) == 1); std.assert(target(0) == 2);
"#,
    );
}

#[test]
fn assignment_resolves_target_before_rhs() {
    both(
        r#"
let first = @{ let x = 0; }; let p = first;
let change = $[&p]() { p = @{ let x = 20; }; 8 };
let run = @jit $[&p,change]() { p.x = change(); };
run(); std.assert(first.x == 8); std.assert(p.x == 20);
"#,
    );
}

#[test]
fn literals_and_standard_library() {
    both(
        r#"
let f = @jit $[]() {
    std.assert(0xFF == 255); std.assert(0o755 == 493); std.assert(0b10_10 == 10);
    std.assert(1_000 == 1000); std.assert(007 == 7);
    std.assert(2.5e-3 == 0.0025); std.assert(1e1_0 == 10000000000.0);
    std.assert(-9223372036854775808 == -(0x8000000000000000));
    std.assert(-7 / 3 == -2); std.assert(-7 % 3 == -1);
    std.assert(std.len("aé\u{1F600}\0") == 4);
    std.assert(std.str(()) == "()"); std.assert(std.str(-0.0) == "-0.0");
    std.assert(std.type(1) == "int"); std.assert(std.type(1.0) == "float");
    std.assert(std.type(@{}) == "closure"); std.assert(std.str(@{}) == "<closure>");
    std.assert(std.int(-3.9) == -3); std.assert(std.float(3) == 3.0);
    std.assert(1 != 1.0); std.assert(true != 1);
    std.assert("a" + "b" == "ab");
}; f();
"#,
    );
}

#[test]
fn lexical_scopes_and_empty_bodies() {
    both(
        r#"
let x = 10; let y = { let x = x + 1; x * 2 };
std.assert(x == 10); std.assert(y == 22);
let empty = @jit $[]() {}; std.assert(empty() == ());
let point = @jit $[](x, y) { @{ let x = x; let y = y; } };
std.assert(point(3, 4).y == 4);
let f = @jit $[](x) { { let x = x + 1; std.assert(x == 6); }; x };
std.assert(f(5) == 5);
"#,
    );
}

#[test]
fn gc_reclaims_cycles_and_retains_escaped_slots() {
    let source = r#"
let factory = @jit $[]() {
    let n = @{};
    let a = @{ let other = &n; };
    n = a;
};
let i = 0; loop { if i == 200 { break; }; factory(); i = i + 1; };
"#;
    for jit in [false, true] {
        let baseline = execute("", jit);
        let rt = execute(source, jit);
        assert!(rt.stats.gc_collections > 100);
        assert_eq!(rt.stats.live_closures, baseline.stats.live_closures);
        assert_eq!(rt.stats.live_slots, baseline.stats.live_slots);
    }
}

#[test]
fn jit_is_only_explicit_and_lazy() {
    let rt = execute(
        "let f = $[](x) { x + 1 }; let i = 0; loop { if i == 100 { break; }; f(i); i = i + 1; }; let unused = @jit $[]() { 1 };",
        true,
    );
    assert_eq!(rt.stats.compiled_functions, 0);
    assert_eq!(rt.stats.native_calls, 0);
    assert_eq!(rt.stats.interpreted_calls, 100);
    let rt = execute("let f = @jit $[](x) { x + 1 }; f(1); f(2);", true);
    assert_eq!(rt.stats.compiled_functions, 1);
    assert_eq!(rt.stats.native_calls, 2);
}

#[test]
fn dynamic_types_are_not_frozen_by_first_call() {
    both(
        r#"
let add = @jit $[](x, y) { x + y };
std.assert(add(1, 2) == 3); std.assert(add(1.5, 2.0) == 3.5); std.assert(add("a", "b") == "ab");
let x = 1; let f = @jit $[&x]() { x + x }; std.assert(f() == 2); x = "q"; std.assert(f() == "qq");
"#,
    );
}

#[test]
fn runtime_errors_match_and_effects_are_not_replayed() {
    for (expression, kind) in [
        ("1 / 0", "DivisionByZero"),
        ("-9223372036854775808 % -1", "IntegerOverflow"),
        ("9223372036854775807 + 1", "IntegerOverflow"),
        ("--9223372036854775808", "IntegerOverflow"),
        ("1.0 / -0.0", "DivisionByZero"),
        ("1e308 * 1e308", "FloatRangeError"),
        ("1 + true", "TypeError"),
        ("if 1 { 2 }", "TypeError"),
        ("true && 1", "TypeError"),
        ("@{}.missing", "MissingField"),
        ("std.assert(false)", "AssertionFailed"),
    ] {
        let source = format!("let f = @jit $[]() {{ std.print(\"once\"); {expression} }}; f();");
        let mut spans = vec![];
        for jit in [false, true] {
            let mut rt = prepare(
                &source,
                Config {
                    jit_enabled: jit,
                    gc_threshold: 1,
                    ..Config::default()
                },
            )
            .unwrap();
            let error = rt.run().unwrap_err();
            assert_eq!(error.kind, kind, "{expression}");
            assert_eq!(rt.output, "once\n");
            spans.push(error.span);
        }
        assert_eq!(spans[0], spans[1]);
    }
}

#[test]
fn argument_validation_precedes_argument_effects() {
    for source in [
        "let f = $[]() {}; f(std.print(1));",
        "let f = $[](&x) {}; f(std.print(1));",
        "let f = @{}; f(std.print(1));",
    ] {
        let mut rt = prepare(source, Config::default()).unwrap();
        assert!(rt.run().is_err());
        assert_eq!(rt.output, "");
    }
}

#[test]
fn readonly_fields_and_bad_jit_targets() {
    for (source, kind) in [
        ("std.print = 1;", "ReadOnlySlot"),
        ("let s = std; s.print = 1;", "ReadOnlySlot"),
        ("jit(@{});", "JitTargetError"),
        ("jit(std.print);", "JitTargetError"),
        ("jit(1);", "JitTargetError"),
        (
            "let bad = $[](f) { 1 }; let f = @bad $[]() {};",
            "DecoratorResultError",
        ),
    ] {
        let mut rt = prepare(source, Config::default()).unwrap();
        assert_eq!(rt.run().unwrap_err().kind, kind);
    }
}

#[test]
fn invalid_frontend_programs_are_rejected() {
    for source in [
        "0b102",
        "1__0",
        "0x",
        "1e+",
        "123abc",
        "\"\\u{D800}\"",
        "\"\\q\"",
        "/* open",
        "let x;",
        "let x = 1",
        "a = b = 1;",
        "@{ let x = 1; x }",
        "@jit f",
        "$() {}",
        "1 < 2 < 3",
        "(1, 2)",
        "[1, 2]",
        "return &x;",
        "let x = 1; let f = $[]() { x };",
        "let x = 1; let f = $[x, &x]() {};",
        "let f = $[](x) { let x = 2; };",
        "let x = 1; let r = &(x + 1);",
        "let jit = 1;",
        "return 1;",
        "loop { let f = $[]() { break; }; };",
        "loop { let x = @{ break; }; };",
        "9223372036854775808",
        "0 - 9223372036854775808",
        "let f = $[]() { @{ return; } };",
        "self",
        "let f = $[]() {}; f(&std);",
        "let f = $[std]() {};",
        ";",
    ] {
        assert!(
            prepare(source, Config::default()).is_err(),
            "unexpected acceptance: {source}"
        );
    }
}

#[test]
fn execution_and_recursion_limits() {
    for source in ["loop {};", "let f = @jit $[]() { loop {} }; f();"] {
        let mut rt = prepare(
            source,
            Config {
                max_steps: Some(100),
                ..Config::default()
            },
        )
        .unwrap();
        assert_eq!(rt.run().unwrap_err().kind, "ExecutionLimit");
    }
    let mut rt = prepare(
        "let f = @jit $[]() { self() }; f();",
        Config {
            max_call_depth: 12,
            ..Config::default()
        },
    )
    .unwrap();
    assert_eq!(rt.run().unwrap_err().kind, "RecursionLimit");
}

#[test]
fn result_value_and_nested_initializer_control() {
    let mut rt = prepare(
        "let f = @jit $[]() { let x = @{ let result = loop { break 7; }; }; x.result }; f()",
        Config::default(),
    )
    .unwrap();
    assert!(matches!(rt.run().unwrap(), Value::Int(7)));
}

#[test]
fn postfix_applies_to_complete_if_expression() {
    both(
        r#"
let f = $[](x) { x + 1 };
let value = if true { f } else if false { f } else { f }(2);
std.assert(value == 3);
"#,
    );
}

#[test]
fn jit_emits_and_executes_checked_integer_arithmetic() {
    let rt = execute(
        "let f = @jit $[](a, b) { (a + b) * (a - b) }; std.assert(f(7, 2) == 45);",
        true,
    );
    assert_eq!(rt.stats.native_integer_ops, 3);
}

#[test]
fn diagnostic_locations_handle_all_newlines() {
    for newline in ["\n", "\r\n", "\r"] {
        let source = format!("let s = \"café\";{newline}\tunknown");
        let error = match prepare(&source, Config::default()) {
            Ok(_) => panic!("expected undefined name"),
            Err(e) => e,
        };
        assert!(error.render("input.pr", &source).contains("input.pr:2:5"));
    }
}

#[test]
fn jit_infrastructure_failure_never_runs_the_body() {
    let path = std::env::temp_dir().join(format!("prim-jit-blocked-{}", std::process::id()));
    std::fs::write(&path, "not a directory").unwrap();
    let source = "let f = @jit $[](x) { std.print(\"body\"); }; f(std.print(\"argument\"));";
    let mut rt = prepare(
        source,
        Config {
            ir_dir: Some(path.clone()),
            ..Config::default()
        },
    )
    .unwrap();
    let error = rt.run().unwrap_err();
    std::fs::remove_file(path).unwrap();
    assert_eq!(error.kind, "JitCompileError");
    assert_eq!(rt.output, "argument\n");
    assert_eq!(rt.stats.native_calls, 0);
}
