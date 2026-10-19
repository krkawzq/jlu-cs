use prim::{Config, Session, Value};
fn integer(value: Value) -> i64 {
    if let Value::Int(n) = value {
        n
    } else {
        panic!("expected int")
    }
}

#[test]
fn fragments_preserve_bindings_and_never_replay_effects() {
    let mut s = Session::new(Config::default());
    s.evaluate(
        "one.pr",
        "std.print(\"once\"); let x = 1; let f = @jit $[&x]() { x }; ",
    )
    .unwrap();
    assert_eq!(integer(s.evaluate("two.pr", "x = 7; f()").unwrap()), 7);
    assert_eq!(integer(s.evaluate("three.pr", "f()").unwrap()), 7);
    assert_eq!(s.runtime.output, "once\n");
    assert_eq!(s.runtime.stats.compiled_functions, 1);
}

#[test]
fn shadowing_keeps_existing_capture_slots_alive() {
    let mut s = Session::new(Config {
        gc_threshold: 1,
        ..Config::default()
    });
    s.evaluate("one", "let x = 1; let f = $[&x]() { std.gc.collect(); x };")
        .unwrap();
    s.evaluate("two", "let x = 9;").unwrap();
    assert_eq!(
        integer(s.evaluate("three", "std.gc.collect(); x + f()").unwrap()),
        10
    );
    assert!(s.runtime.forget_global("x"));
    assert_eq!(integer(s.evaluate("four", "f()").unwrap()), 1);
}

#[test]
fn failed_input_keeps_completed_effects_without_inventing_bindings() {
    let mut s = Session::new(Config {
        gc_threshold: 1,
        ..Config::default()
    });
    s.evaluate("one", "let x = 1;").unwrap();
    let error = s
        .evaluate("two", "x = 2; let y = 3; 1 / 0; let missing = 4;")
        .unwrap_err();
    assert_eq!(error.kind, "DivisionByZero");
    assert_eq!(integer(s.evaluate("three", "x + y").unwrap()), 5);
    assert_eq!(
        s.evaluate("four", "missing").unwrap_err().kind,
        "UndefinedName"
    );
    s.evaluate("five", "let missing = 8;").unwrap();
    assert_eq!(integer(s.evaluate("six", "missing").unwrap()), 8);
}

#[test]
fn compile_failure_has_no_effect_and_old_functions_keep_source_identity() {
    let mut s = Session::new(Config::default());
    s.evaluate("definition.pr", "let f = @jit $[]() { 1 / 0 };")
        .unwrap();
    let before = s.runtime.program.functions.len();
    assert!(
        s.evaluate("bad.pr", "let discarded = $[]() { unknown };")
            .is_err()
    );
    assert_eq!(s.runtime.program.functions.len(), before);
    let error = s.evaluate("caller.pr", "f()").unwrap_err();
    let text = s.render_error(&error);
    assert!(text.contains("definition.pr"));
    assert!(text.contains("caller.pr"));
}

#[test]
fn reset_reclaims_session_and_budget_is_per_input() {
    let mut s = Session::new(Config {
        max_steps: Some(20),
        ..Config::default()
    });
    for _ in 0..20 {
        s.evaluate("input", "let x = 3;").unwrap();
    }
    assert_eq!(s.runtime.global_values().len(), 1);
    s.reset();
    assert!(s.runtime.global_values().is_empty());
    assert!(s.runtime.program.functions.is_empty());
    s.max_source_bytes = 8;
    assert!(s.evaluate("too-large", "123456789").is_err());
}
