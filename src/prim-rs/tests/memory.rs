use prim::{Config, Runtime, Value, prepare};
use std::rc::Rc;

fn checked(source: &str, jit: bool) -> Runtime {
    let mut rt = prepare(
        source,
        Config {
            jit_enabled: jit,
            gc_threshold: 1,
            max_steps: Some(2_000_000),
            ..Config::default()
        },
    )
    .unwrap();
    rt.run()
        .unwrap_or_else(|e| panic!("{}", e.render("memory.pr", source)));
    rt
}

#[test]
fn dead_block_values_are_reclaimed_before_function_return() {
    let source = r#"
let check = @jit $[]() {
    let baseline = std.gc.stats().slots;
    { let large = std.array(20000, "payload"); std.assert(std.len(large) == 20000); };
    std.gc.collect();
    let after = std.gc.stats();
    std.assert(after.slots - baseline < 16);
    std.assert(after.slot_capacity < 512);
}; check();
"#;
    for jit in [false, true] {
        checked(source, jit);
    }
}

#[test]
fn large_dead_strings_release_their_rc_owners() {
    for jit in [false, true] {
        checked(
            r#"
let check = @jit $[]() {
    { let text = std.text.repeat("x", 4000000); std.assert(std.len(text) == 4000000); };
    std.gc.collect();
    std.assert(std.gc.stats().string_bytes < 1024);
}; check();
"#,
            jit,
        );
    }
}

#[test]
fn roots_survive_calls_branches_aliasing_and_backedges() {
    for jit in [false, true] {
        checked(
            r#"
let roundtrip = @jit $[](value) { std.gc.collect(); value };
let make = @jit $[](n) { let object = @{ let n = n; }; let result = $[object]() { std.gc.collect(); object.n }; std.gc.collect(); result };
let i = 0;
loop {
    if i == 100 { break; };
    let f = make(i);
    let a = @{ let x = i; };
    let alias = &a.x;
    std.assert(roundtrip(a).x == i);
    if i % 2 == 0 { std.gc.collect(); std.assert(f() == i); }
    else { let extra = std.array(100, f); std.gc.collect(); std.assert(std.get(extra, 99)() == i); };
    alias = i + 1; std.gc.collect(); std.assert(a.x == i + 1);
    i = i + 1;
};
"#,
            jit,
        );
    }
}

#[test]
fn assignment_target_and_earlier_arguments_are_gc_roots() {
    for jit in [false, true] {
        checked(
            r#"
let original = @{ let x = 3; };
let current = original;
let effect = @jit $[&current]() { current = @{}; std.gc.collect(); 9 };
let first = @jit $[](a, b) { std.gc.collect(); a };
current.x = effect(); std.assert(original.x == 9);
let value = first(@{ let answer = 42; }, effect()); std.assert(value.answer == 42);
let capture = @jit $[](&slot) { std.gc.collect(); @jit $[&slot]() { std.gc.collect(); slot } };
let escaped = { let x = 12; capture(&x) };
std.gc.collect(); std.assert(escaped() == 12);
"#,
            jit,
        );
    }
}

#[test]
fn error_exit_releases_heap_and_frames() {
    let mut baseline = prepare("", Config::default()).unwrap();
    baseline.run().unwrap();
    for jit in [false, true] {
        let mut rt=prepare("let f = @jit $[]() { let a = std.array(5000, ()); std.set(a, 0, a); std.assert(false); }; f();",Config{jit_enabled:jit,gc_threshold:1,..Config::default()}).unwrap();
        assert_eq!(rt.run().unwrap_err().kind, "AssertionFailed");
        assert_eq!(rt.stats.live_slots, baseline.stats.live_slots);
        assert_eq!(rt.stats.live_closures, baseline.stats.live_closures);
        // A second run reuses the runtime and compiled code without retaining the failed frame.
        assert_eq!(rt.run().unwrap_err().kind, "AssertionFailed");
        assert_eq!(rt.stats.live_slots, baseline.stats.live_slots);
    }
}

#[test]
fn output_capture_is_explicit_and_can_be_drained() {
    let mut rt = prepare(
        "std.write(std.text.repeat(\"a\", 100000));",
        Config::default(),
    )
    .unwrap();
    rt.run().unwrap();
    let output = rt.take_output();
    assert_eq!(output.len(), 100000);
    assert!(rt.output.is_empty());
    assert_eq!(rt.output.capacity(), 0);
    let rt = checked("std.print(1);", false);
    assert_eq!(rt.output, "1\n");
    let mut silent = prepare(
        "let i = 0; loop { if i == 1000 { break; }; std.write(\"frame\"); i = i + 1; };",
        Config {
            capture_output: false,
            ..Config::default()
        },
    )
    .unwrap();
    silent.run().unwrap();
    assert_eq!(silent.output.capacity(), 0);
}

#[test]
fn returned_string_lifetime_is_owned_and_released() {
    let mut rt = prepare("std.text.repeat(\"x\", 100000)", Config::default()).unwrap();
    let Value::Str(s) = rt.run().unwrap() else {
        panic!("string result")
    };
    let weak = Rc::downgrade(&s);
    drop(s);
    assert!(weak.upgrade().is_some());
    drop(rt);
    assert!(weak.upgrade().is_none());
}

#[test]
fn repeated_native_runtime_lifecycles_drop_the_engine() {
    for _ in 0..30 {
        let rt = checked(
            "let f = @jit $[](n) { let cycle = std.array(2, ()); std.set(cycle, 0, cycle); n * n }; std.assert(f(8) == 64);",
            true,
        );
        assert_eq!(rt.stats.compiled_functions, 1);
    }
}

#[test]
fn long_lived_native_loop_has_bounded_managed_heap() {
    for jit in [false, true] {
        checked(
            r#"
let churn = @jit $[]() {
    let baseline = std.gc.stats().slots;
    let i = 0;
    loop {
        if i == 500 { break; };
        { let a = std.array(256, ()); std.set(a, 0, a); };
        std.gc.collect();
        let stats = std.gc.stats();
        std.assert(stats.slots - baseline < 32);
        std.assert(stats.slot_capacity < 2048);
        i = i + 1;
    };
}; churn();
"#,
            jit,
        );
    }
}
