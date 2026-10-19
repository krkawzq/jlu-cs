use std::{
    io::Write,
    process::{Command, Stdio},
};
fn invoke(args: &[&str], input: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prim"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn text(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn discovery_and_usage_exit_codes() {
    for args in [
        &["--help"][..],
        &["help"],
        &["-V"],
        &["version"],
        &["builtins"],
    ] {
        let out = invoke(args, "");
        assert!(out.status.success());
        assert!(!out.stdout.is_empty());
    }
    for args in [
        &["--unknown"][..],
        &["--fuel"],
        &["--gc-threshold=0"],
        &["--jit=auto"],
        &["--opt-level=3"],
        &["-O-1"],
        &["--color=bad"],
        &["missing-file.pr"],
    ] {
        assert_eq!(invoke(args, "").status.code(), Some(2));
    }
}
#[test]
fn eval_stdin_and_argument_modes() {
    assert_eq!(text(&invoke(&["--result", "-e", "21 * 2"], "")), "42\n");
    assert_eq!(text(&invoke(&["--result", "-"], "6 * 7")), "42\n");
    assert_eq!(text(&invoke(&[], "std.print(42);")), "42\n");
    let out = invoke(&["-e", "std.print(std.env.arg(0));", "--", "--example"], "");
    assert!(out.status.success());
    assert_eq!(text(&out), "--example\n");
    let out = invoke(
        &[
            "-e",
            "let x = 1; std.print(\"once\");",
            "-e",
            "x = 2; std.print(x);",
        ],
        "",
    );
    assert!(out.status.success());
    assert_eq!(text(&out), "once\n2\n");
}
#[test]
fn extended_stdio_assertions_and_process_exit() {
    let out = invoke(
        &[
            "-e",
            "std.eprint(\"warn\"); std.ewrite(\"!\"); std.eflush(); std.print(\"out\");",
        ],
        "",
    );
    assert!(out.status.success());
    assert_eq!(text(&out), "out\n");
    assert_eq!(String::from_utf8_lossy(&out.stderr), "warn\n!");

    let out = invoke(&["-e", "std.print(std.read_all());"], "hello\nworld");
    assert_eq!(text(&out), "hello\nworld\n");
    let out = invoke(&["-e", "std.print(std.len(std.read_all()));"], "éé");
    assert_eq!(text(&out), "2\n");

    let out = invoke(&["-e", "std.assert_eq(1, 2);"], "");
    assert_eq!(out.status.code(), Some(1));
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.contains("AssertionFailed"));
    assert!(error.contains("expected 2, got 1"));

    let out = invoke(&["-e", "std.fail(\"boom\");"], "");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("UserError"));

    let out = invoke(&["-e", "std.print(\"bye\"); std.process.exit(7);"], "");
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(text(&out), "bye\n");
}

#[test]
fn inspection_never_executes_source() {
    for mode in ["check", "ast", "ir", "lex"] {
        let out = invoke(&[mode, "-e", "std.print(\"DO_NOT_RUN\");"], "");
        assert!(out.status.success());
        assert!(!text(&out).lines().any(|l| l == "DO_NOT_RUN"));
    }
    let out = invoke(&["--ast", "--check", "-e", "1"], "");
    assert!(out.status.success());
    assert!(text(&out).contains("Int"));
}
#[test]
fn persistent_repl_and_interactive_after_eval() {
    let out = invoke(
        &["repl", "-q", "--no-prompt"],
        "let x = 2;\nlet f = @jit $[&x]() { x * x };\nx = 5;\nf()\n:vars\n:quit\n",
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text(&out).contains("25\n"));
    assert!(text(&out).contains("x = 5"));
    let out = invoke(&["-q", "-i", "-e", "let x = 40;"], "x + 2\n:quit\n");
    assert!(out.status.success());
    assert_eq!(text(&out), "42\n");
}
#[test]
fn repl_multiline_reset_and_error_recovery() {
    let out = invoke(
        &["repl", "-q"],
        "let f = $[](x) {\nx + 1\n};\nf(4)\n1 / 0\nf(5)\n:reset\n:vars\n:quit\n",
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out), "5\n6\n");
    assert!(String::from_utf8_lossy(&out.stderr).contains("DivisionByZero"));
}
#[test]
fn json_diagnostics_and_limits() {
    let out = invoke(&["--diagnostic-format=json", "-e", "1 / 0"], "");
    assert_eq!(out.status.code(), Some(1));
    let error = String::from_utf8_lossy(&out.stderr);
    assert!(error.starts_with('{'));
    assert!(error.contains("\"kind\":\"DivisionByZero\""));
    assert!(error.contains("\"file\":\"<eval:1>\""));
    let out = invoke(&["--fuel=20", "-e", "loop {};"], "");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("ExecutionLimit"));
    assert_eq!(
        invoke(&["--max-source-bytes=4", "-e", "12345"], "")
            .status
            .code(),
        Some(2)
    );
}
#[test]
fn explicit_jit_control_and_statistics() {
    let source = "let f = @jit $[](x) { x + 1 }; std.print(f(2));";
    let out = invoke(&["--stats", "--time", "--jit=off", "-q", "-e", source], "");
    assert!(out.status.success());
    assert_eq!(text(&out), "3\n");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("native_calls=0"));
    assert!(err.contains("[time]"));
    let out = invoke(&["--stats", "-e", source], "");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("native_calls=1"));
}

#[test]
fn optimization_flags_and_ir_artifacts() {
    for (flag, level) in [("-O0", 0), ("-O1", 1), ("-O2", 2), ("--opt-level=1", 1)] {
        let source = format!("let f=@jit $[](x) {{ x+1 }}; std.inspect.compile(f); std.assert(std.inspect.jit(f).opt_level=={level}); std.print(f(2));");
        let path = std::env::temp_dir().join(format!("prim-optimization-{}-{level}", std::process::id()));
        let out = invoke(&[flag, "--emit-llvm", path.to_str().unwrap(), "-e", &source], "");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(text(&out), "3\n");
        for file in ["function_1.ll", "function_1.opt.ll"] {
            assert!(std::fs::read_to_string(path.join(file)).unwrap().contains("prim_function_1"));
        }
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn repl_load_file_and_colored_error() {
    let path = std::env::temp_dir().join(format!("prim-repl-load-{}.pr", std::process::id()));
    std::fs::write(&path, "std.print(\"loaded once\"); let loaded = 40;").unwrap();
    let input = format!(":load {}\nloaded + 2\n:quit\n", path.display());
    let out = invoke(&["repl", "-q"], &input);
    std::fs::remove_file(path).unwrap();
    assert!(out.status.success());
    assert_eq!(text(&out), "loaded once\n42\n");
    let out = invoke(&["--color=always", "-e", "1 / 0"], "");
    assert!(out.stderr.starts_with(b"\x1b[31m"));
}

#[test]
fn initial_interactive_error_is_reported_in_piped_mode() {
    let out = invoke(
        &["-i", "-q", "--diagnostic-format=json", "-e", "1 / 0"],
        ":quit\n",
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&out.stderr).lines().count(), 1);
}
