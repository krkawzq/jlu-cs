use prim::{Config, Runtime, Value, prepare};

fn run(source: &str, jit: bool, level: u8) -> (Runtime, Result<Value, prim::Error>) {
    let mut rt = prepare(
        source,
        Config {
            jit_enabled: jit,
            opt_level: level,
            gc_threshold: 1,
            ..Config::default()
        },
    )
    .unwrap();
    let result = rt.run();
    (rt, result)
}
fn compare(source: &str) {
    let (reference, result) = run(source, false, 0);
    for level in 0..=2 {
        let (rt, actual) = run(source, true, level);
        match (&result, &actual) {
            (Ok(a), Ok(b)) => assert!(a.equals(b), "{source}: {a:?} != {b:?}"),
            (Err(a), Err(b)) => {
                assert_eq!(a.kind, b.kind, "{source}");
                assert_eq!(a.message, b.message, "{source}");
                assert_eq!(a.span, b.span, "{source}");
                assert_eq!(a.trace, b.trace, "{source}");
            }
            _ => panic!("{source}: {result:?} != {actual:?}"),
        }
        assert_eq!(reference.output, rt.output);
        assert_eq!(reference.stats.steps, rt.stats.steps, "{source}, O{level}");
    }
}

#[test]
fn scalar_loops_branches_and_checked_arithmetic() {
    for n in [-2, 0, 1, 35, 2000] {
        compare(&format!(
            "let f = @jit $[](n) {{ let i=0; let s=0; loop {{ if i>=n {{ break s; }}; i=i+1; if i%3==0 {{ continue; }}; let x=i*2; s=s+x; }} }}; f({n})"
        ));
    }
    for body in [
        "if n > 0 { return 7; }; 9",
        "n / -3",
        "n % -3",
        "-n",
        "!(n == 3) && (n != 4 || false)",
        "if n < 0 { () } else { n }",
        "let x = n; x = false; x == false",
        "let x=n; x=true; x+1",
        "let x=n; x=(); x==()",
        "if n { 1 } else { 2 }",
        "n + 9223372036854775807",
        "n * 9223372036854775807",
        "n / 0",
        "n % 0",
        "let x=0; loop { x=x+1; if x==3000 { break n/0; }; }",
    ] {
        compare(&format!("let f=@jit $[](n) {{ {body} }}; f(3)"));
    }
    for body in ["n / -1", "n % -1", "-n", "n - 1"] {
        compare(&format!(
            "let f=@jit $[](n) {{ {body} }}; f(-9223372036854775808)"
        ));
    }
}

#[test]
fn dynamic_arguments_aliases_captures_and_resumption() {
    for arg in ["2.5", "true", "()", "\"hi\""] {
        compare(&format!("let f=@jit $[](n) {{ n+n }}; f({arg})"));
    }
    compare("let x=1; let f=@jit $[&x](n) { x=x+n; x }; f(2); f(3)");
    compare("let f=@jit $[](n) { let alias=&n; alias=9; n }; f(2)");
    compare("let f=@jit $[](&n) { n=n+1; n }; let x=2; f(&x); x");
    compare(
        "let f=@jit $[](n) { let i=0; loop { i=i+1; if i==30 { break; }; }; let x=false; x+1 }; f(2)",
    );
    let (rt, value) = run("let f=@jit $[](n) { let x=n; x=false; x+1 }; f(2)", true, 2);
    assert!(value.is_err());
    assert_eq!(rt.stats.scalar_calls, 1);
    assert_eq!(rt.stats.scalar_fallbacks, 1);
}

#[test]
fn scalar_path_reduces_helpers_and_preserves_fuel() {
    let source = "let f=@jit $[](n) { let i=0; let s=0; loop { if i==n { break s; }; s=s+i; i=i+1; } }; f(5000)";
    let (generic, a) = run(source, true, 0);
    let (scalar, b) = run(source, true, 2);
    assert!(a.unwrap().equals(&b.unwrap()));
    assert_eq!(scalar.stats.scalar_calls, 1);
    assert_eq!(scalar.stats.steps, generic.stats.steps);
    assert!(scalar.stats.native_helper_calls * 50 < generic.stats.native_helper_calls);
    for fuel in [0, 10, 30, 1000, 90000] {
        let mut results = vec![];
        for jit in [false, true] {
            let mut rt = prepare(
                source,
                Config {
                    jit_enabled: jit,
                    max_steps: Some(fuel),
                    ..Config::default()
                },
            )
            .unwrap();
            let result = rt.run();
            results.push((
                result.map(|v| v.display()).map_err(|e| (e.kind, e.span)),
                rt.stats.steps,
            ));
            assert_eq!(rt.stats.scalar_calls, 0);
        }
        assert_eq!(results[0], results[1]);
    }
}

#[test]
fn optimized_artifacts_describe_actual_compilation() {
    for level in 0..=2 {
        let source = format!(
            r#"
let f=@jit $[](n) {{ let i=0; loop {{ if i>=n {{ break i; }}; i=i+1; }} }};
std.inspect.compile(f);
let info=std.inspect.jit(f);
std.assert(info.opt_level=={level});
std.assert(info.lowering=="{}");
std.assert(info.optimized_ir_bytes==std.len(std.inspect.optimized_ir(f)));
std.assert(std.text.contains(std.inspect.optimized_ir(f), "define i32 @prim_function_"));
std.assert(std.inspect.native(f).size>0);
f(20)
"#,
            if level == 0 {
                "generic"
            } else {
                "local-scalars"
            }
        );
        let (_, result) = run(&source, true, level);
        assert_eq!(result.unwrap().display(), "20");
    }
}
