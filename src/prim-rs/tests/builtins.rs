use prim::{Config, Runtime, Value, prepare};
fn run(source: &str, jit: bool) -> Runtime {
    let mut runtime = prepare(
        source,
        Config {
            jit_enabled: jit,
            gc_threshold: 1,
            max_steps: Some(1_000_000),
            ..Config::default()
        },
    )
    .unwrap_or_else(|e| panic!("{}", e.render("builtin-test.pr", source)));
    runtime
        .run()
        .unwrap_or_else(|e| panic!("{}", e.render("builtin-test.pr", source)));
    runtime
}
fn both(source: &str) {
    let a = run(source, false);
    let b = run(source, true);
    assert_eq!(a.output, b.output);
}

#[test]
fn arrays_preserve_identity_and_share_normal_gc_slots() {
    both(
        r#"
let check = @jit $[]() {
    let a = std.array(4, 0);
    std.assert(std.type(a) == "closure"); std.assert(std.len(a) == 4);
    std.set(a, 2, 9); std.assert(std.get(a, 2) == 9);
    std.assert(std.get(a, "2") == 9); std.assert(std.has(a, 3)); std.assert(!std.has(a, 4));
    let object = @{ let x = 1; };
    let shared = std.array(2, object);
    std.get(shared, 0).x = 4; std.assert(std.get(shared, 1).x == 4);
    std.set(object, "x", 8); std.assert(object.x == 8);
    std.set(a, 0, a); std.assert(std.get(a, 0) == a);
    std.assert(std.len(std.array(0, ())) == 0);
}; check();
"#,
    );
}

#[test]
fn cyclic_arrays_are_collected() {
    for jit in [false, true] {
        let baseline = run("", jit);
        let rt = run(
            "let make = @jit $[]() { let a = std.array(20, ()); std.set(a, 0, a); }; let i = 0; loop { if i == 50 { break; }; make(); i = i + 1; };",
            jit,
        );
        assert_eq!(rt.stats.live_slots, baseline.stats.live_slots);
        assert_eq!(rt.stats.live_closures, baseline.stats.live_closures);
    }
}

#[test]
fn unicode_text_operations_and_parsing() {
    both(
        r#"
let check = @jit $[]() {
    std.assert(std.text.at("aé😀", 2) == "😀");
    std.assert(std.text.slice("aé😀", 1, 3) == "é😀");
    std.assert(std.text.slice("abc", 3, 3) == "");
    std.assert(std.text.repeat("é", 3) == "ééé");
    std.assert(std.text.trim(" \t hi\n") == "hi");
    std.assert(std.text.contains("aé", "é"));
    std.assert(std.text.starts_with("hello", "he")); std.assert(std.text.ends_with("hello", "lo"));
    std.assert(std.text.replace("a-b-b", "b", "é") == "a-é-é");
    std.assert(std.text.replace("ab", "", "-") == "-a-b-");
    std.assert(std.parse_int(" -42 ") == -42); std.assert(std.parse_int("no") == ());
    std.assert(std.parse_int("9223372036854775808") == ());
    std.assert(std.parse_float("2.5e2") == 250.0); std.assert(std.parse_float("NaN") == ());
    std.assert(std.parse_float("inf") == ()); std.assert(std.parse_float("1e999") == ());
}; check();
"#,
    );
}

#[test]
fn numeric_utilities_keep_strict_kinds() {
    both(
        r#"
let check = @jit $[]() {
    std.assert(std.math.abs(-4) == 4); std.assert(std.math.abs(-1.5) == 1.5);
    std.assert(std.math.min(4, 2) == 2); std.assert(std.math.max(4.0, 2.0) == 4.0);
    std.assert(std.math.clamp(9, 0, 4) == 4); std.assert(std.math.clamp(-1, 0, 4) == 0);
    std.assert(std.math.clamp(2, 0, 4) == 2);
    std.assert(std.math.floor(-1.2) == -2.0); std.assert(std.math.ceil(-1.2) == -1.0);
    std.assert(std.math.round(-1.5) == -2.0); std.assert(std.math.sqrt(9.0) == 3.0);
    std.assert(std.math.pow(2.0, 3.0) == 8.0); std.assert(std.math.sin(0.0) == 0.0);
    std.assert(std.math.cos(0.0) == 1.0); std.assert(std.math.sign(-0.0) == 0);
    std.assert(std.math.sign(-4) == -1); std.assert(std.math.sign(2.0) == 1);
}; check();
"#,
    );
}

#[test]
fn randomness_is_reproducible_and_bounded() {
    both(
        r#"
let check = @jit $[]() {
    std.random.seed(0); let first = std.random.int(-100, 100); let second = std.random.float();
    std.random.seed(0); std.assert(std.random.int(-100, 100) == first); std.assert(std.random.float() == second);
    let i = 0;
    loop {
        if i == 500 { break; };
        let n = std.random.int(-7, 9); std.assert(n >= -7 && n < 9);
        let f = std.random.float(); std.assert(f >= 0.0 && f < 1.0);
        let wide = std.random.int(-9223372036854775808, 9223372036854775807);
        std.assert(std.type(wide) == "int");
        i = i + 1;
    };
    std.assert(std.random.int(4, 5) == 4);
}; check();
"#,
    );
}

#[test]
fn monotonic_clock_and_output_capture() {
    let rt = run(
        "let f = @jit $[]() { let before = std.time.now_ms(); std.time.sleep(2); std.assert(std.time.now_ms() >= before); std.assert(std.time.unix_ms() > 0); std.write(\"hello\"); std.write(3); std.print(\"!\"); std.flush(); }; f();",
        true,
    );
    assert_eq!(rt.output, "hello3!\n");
    let mut rt = prepare(
        "std.print(1);",
        Config {
            capture_output: false,
            ..Config::default()
        },
    )
    .unwrap();
    rt.run().unwrap();
    assert!(rt.output.is_empty());
}

#[test]
fn file_io_and_script_arguments() {
    let path = std::env::temp_dir().join(format!("prim-builtins-test-{}.txt", std::process::id()));
    let source = r#"
let check = @jit $[]() {
    std.assert(std.env.argc() == 2); std.assert(std.env.arg(1) == "café"); std.assert(std.env.arg(2) == ());
    std.assert(std.type(std.env.cwd()) == "str");
    std.assert(std.env.get("PRIM_TEST_MISSING_46E3F1C1") == ());
    let path = std.env.arg(0);
    std.fs.write_text(path, "hello\n"); std.fs.append_text(path, "café\0");
    std.assert(std.fs.exists(path)); std.assert(std.fs.read_text(path) == "hello\ncafé\0");
    std.fs.write_text(path, "new"); std.assert(std.fs.read_text(path) == "new");
}; check();
"#;
    for jit in [false, true] {
        let mut rt = prepare(
            source,
            Config {
                jit_enabled: jit,
                args: vec![path.to_string_lossy().into_owned(), "café".into()],
                ..Config::default()
            },
        )
        .unwrap();
        rt.run().unwrap();
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn builtin_validation_and_namespace_readonly() {
    for (expr, kind) in [
        ("std.array(-1, 0)", "IndexError"),
        ("std.array(1000001, 0)", "ResourceLimit"),
        ("std.get(std.array(2, 0), 2)", "IndexError"),
        ("std.get(@{}, \"missing\")", "MissingField"),
        ("std.set(std.math, \"abs\", 1)", "ReadOnlySlot"),
        ("std.has(@{}, -1)", "IndexError"),
        ("std.math.abs(-9223372036854775808)", "IntegerOverflow"),
        ("std.math.min(1, 1.0)", "TypeError"),
        ("std.math.clamp(1, 3, 2)", "ValueError"),
        ("std.math.sqrt(-1.0)", "FloatRangeError"),
        ("std.math.pow(1e300, 2.0)", "FloatRangeError"),
        ("std.random.int(4, 4)", "ValueError"),
        ("std.time.sleep(-1)", "ValueError"),
        ("std.text.at(\"hi\", 2)", "IndexError"),
        ("std.text.slice(\"hi\", 2, 1)", "IndexError"),
        ("std.text.repeat(\"abc\", 100000000)", "ResourceLimit"),
        ("std.parse_int(2)", "TypeError"),
        ("std.text.fixed(1.0, 18)", "ValueError"),
        ("std.text.pad_start(\"x\", 2, \"ab\")", "ValueError"),
        ("std.text.from_code(55296)", "ValueError"),
        ("std.text.parse_int_radix(\"10\", 1)", "ValueError"),
        ("std.math.asin(2.0)", "FloatRangeError"),
        ("std.math.log(1.0, 1.0)", "FloatRangeError"),
        ("std.bits.shl(1, 64)", "ValueError"),
        ("std.random.float_range(1.0, 1.0)", "ValueError"),
        ("std.random.choice(std.array(0, ()))", "ValueError"),
        ("std.seq.slice(std.array(2, 0), 2, 1)", "IndexError"),
        ("std.time.format_utc(9223372036854775807)", "ValueError"),
        ("std.term.enter()", "TerminalError"),
        ("std.term.home()", "TerminalError"),
        ("std.term.title(\"bad\\u{7}\")", "ValueError"),
        ("std.term.poll_key(60001)", "ValueError"),
    ] {
        for jit in [false, true] {
            let source = format!("let f = @jit $[]() {{ {expr} }}; f();");
            let mut rt = prepare(
                &source,
                Config {
                    jit_enabled: jit,
                    ..Config::default()
                },
            )
            .unwrap();
            assert_eq!(rt.run().unwrap_err().kind, kind, "{expr}");
        }
    }
    let mut rt = prepare("std.time.now_ms(std.print(1));", Config::default()).unwrap();
    assert_eq!(rt.run().unwrap_err().kind, "ArityError");
    assert!(rt.output.is_empty());
}

#[test]
fn terminal_size_and_unentered_leave() {
    both(
        "let f = @jit $[]() { std.term.leave(); let s = std.term.size(); std.assert(s.width > 0 && s.height > 0); std.assert(std.type(std.term.is_tty()) == \"bool\"); }; f();",
    );
}

#[test]
fn extended_text_search_split_and_formatting() {
    both(
        r#"
let check = @jit $[]() {
    std.assert_eq(std.text.index_of("aé😀é", "é"), 1);
    std.assert_eq(std.text.last_index_of("aé😀é", "é"), 3);
    std.assert(std.text.index_of("abc", "z") == ());
    std.assert_eq(std.text.index_of("abc", ""), 0);
    std.assert_eq(std.text.last_index_of("abc", ""), 3);

    let parts = std.text.split("a,b,,c", ",");
    std.assert_eq(std.len(parts), 4);
    std.assert_eq(std.get(parts, 0), "a");
    std.assert_eq(std.get(parts, 2), "");
    std.assert_eq(std.get(parts, 3), "c");
    let characters = std.text.split("aé", "");
    std.assert_eq(std.len(characters), 2);
    std.assert_eq(std.get(characters, 1), "é");
    let lines = std.text.lines("a\r\nb\n");
    std.assert_eq(std.len(lines), 2);
    std.assert_eq(std.get(lines, 1), "b");
    std.assert_eq(std.text.join(parts, "-"), "a-b--c");

    std.assert_eq(std.text.upper("hello"), "HELLO");
    std.assert_eq(std.text.lower("WORLD"), "world");
    std.assert_eq(std.text.pad_start("7", 3, "0"), "007");
    std.assert_eq(std.text.pad_end("ab", 4, "·"), "ab··");
    std.assert_eq(std.text.pad_start("long", 2, "0"), "long");
    std.assert_eq(std.text.code_at("A😀", 1), 128512);
    std.assert_eq(std.text.from_code(128512), "😀");
    std.assert_eq(std.text.fixed(3.14159, 2), "3.14");
    std.assert_eq(std.text.fixed(-1.5, 0), "-2");
    std.assert_eq(std.text.parse_int_radix(" -ff ", 16), -255);
    std.assert_eq(std.text.parse_int_radix("+101", 2), 5);
    std.assert(std.text.parse_int_radix("2", 2) == ());
}; check();
"#,
    );
}

#[test]
fn extended_math_and_bit_operations() {
    both(
        r#"
let check = @jit $[]() {
    std.assert(std.math.abs(std.math.tan(0.0)) < 1e-12);
    std.assert(std.math.abs(std.math.asin(1.0) - std.math.pi() / 2.0) < 1e-12);
    std.assert(std.math.abs(std.math.acos(1.0)) < 1e-12);
    std.assert(std.math.abs(std.math.atan(1.0) - std.math.pi() / 4.0) < 1e-12);
    std.assert(std.math.abs(std.math.atan2(1.0, 0.0) - std.math.pi() / 2.0) < 1e-12);
    std.assert(std.math.abs(std.math.exp(0.0) - 1.0) < 1e-12);
    std.assert(std.math.abs(std.math.ln(std.math.exp(2.0)) - 2.0) < 1e-12);
    std.assert(std.math.abs(std.math.log(8.0, 2.0) - 3.0) < 1e-12);
    std.assert(std.math.abs(std.math.log2(8.0) - 3.0) < 1e-12);
    std.assert(std.math.abs(std.math.log10(1000.0) - 3.0) < 1e-12);
    std.assert_eq(std.math.trunc(-1.9), -1.0);
    std.assert_eq(std.math.hypot(3.0, 4.0), 5.0);

    std.assert_eq(std.bits.and(6, 3), 2);
    std.assert_eq(std.bits.or(6, 3), 7);
    std.assert_eq(std.bits.xor(6, 3), 5);
    std.assert_eq(std.bits.not(0), -1);
    std.assert_eq(std.bits.shl(1, 4), 16);
    std.assert_eq(std.bits.shr(-8, 1), -4);
    std.assert_eq(std.bits.count_ones(255), 8);
    std.assert_eq(std.bits.leading_zeros(1), 63);
    std.assert_eq(std.bits.trailing_zeros(8), 3);
}; check();
"#,
    );
}

#[test]
fn sequence_helpers_mutate_arrays_and_copy_values() {
    both(
        r#"
let check = @jit $[]() {
    let values = std.array(5, 0);
    std.set(values, 0, 3); std.set(values, 1, 1); std.set(values, 2, 4);
    std.set(values, 3, 1); std.set(values, 4, 5);

    let slice = std.seq.slice(values, 1, 4);
    std.assert_eq(std.len(slice), 3);
    std.assert_eq(std.get(slice, 0), 1);
    std.set(slice, 0, 9);
    std.assert_eq(std.get(values, 1), 1);

    let copied = std.seq.copy(values);
    std.set(copied, 0, 8);
    std.assert_eq(std.get(values, 0), 3);

    std.seq.sort(values);
    std.assert_eq(std.get(values, 0), 1);
    std.assert_eq(std.get(values, 4), 5);
    std.seq.reverse(values);
    std.assert_eq(std.get(values, 0), 5);
    std.assert_eq(std.get(values, 4), 1);
    std.seq.fill(values, 7);
    std.assert_eq(std.get(values, 2), 7);
    std.assert_eq(std.seq.index_of(values, 7), 0);
    std.assert(std.seq.index_of(values, 8) == ());

    let words = std.array(3, "");
    std.set(words, 0, "pear"); std.set(words, 1, "apple"); std.set(words, 2, "fig");
    std.seq.sort(words);
    std.assert_eq(std.get(words, 0), "apple");
    std.assert_eq(std.get(words, 2), "pear");

    let combined = std.seq.concat(slice, words);
    std.assert_eq(std.len(combined), 6);
    std.assert_eq(std.get(combined, 3), "apple");
}; check();
"#,
    );
}

#[test]
fn extended_random_generators_are_reproducible() {
    both(
        r#"
let check = @jit $[]() {
    std.random.seed(42);
    let first_bool = std.random.bool();
    let first_normal = std.random.normal();
    let first_range = std.random.float_range(-2.0, 3.0);
    std.random.seed(42);
    std.assert_eq(std.random.bool(), first_bool);
    std.assert_eq(std.random.normal(), first_normal);
    std.assert_eq(std.random.float_range(-2.0, 3.0), first_range);
    std.assert(first_range >= -2.0 && first_range < 3.0);

    let items = std.array(4, 0);
    std.set(items, 0, 10); std.set(items, 1, 20); std.set(items, 2, 30); std.set(items, 3, 40);
    std.random.seed(7);
    std.random.shuffle(items);
    let seen = std.array(4, false);
    let index = 0;
    loop {
        if index == 4 { break; };
        let value = std.get(items, index);
        std.assert(value == 10 || value == 20 || value == 30 || value == 40);
        std.set(seen, (value / 10) - 1, true);
        index = index + 1;
    };
    let check_index = 0;
    loop {
        if check_index == 4 { break; };
        std.assert(std.get(seen, check_index));
        check_index = check_index + 1;
    };

    let picked = std.random.choice(items);
    std.assert(picked == 10 || picked == 20 || picked == 30 || picked == 40);
    let bytes = std.random.bytes(32);
    std.assert_eq(std.len(bytes), 32);
    let byte_index = 0;
    loop {
        if byte_index == 32 { break; };
        let byte = std.get(bytes, byte_index);
        std.assert(byte >= 0 && byte < 256);
        byte_index = byte_index + 1;
    };
}; check();
"#,
    );
}

#[test]
fn filesystem_calendar_and_system_builtins() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prim-builtins-extended-{}-{stamp}",
        std::process::id()
    ));
    let source = r#"
let check = @jit $[]() {
    let root = std.env.arg(0);
    let nested = root + "/a/b";
    std.fs.mkdir_all(nested);
    std.assert(std.fs.is_dir(nested));
    std.assert(!std.fs.is_dir(root + "/missing"));

    let path = nested + "/note.txt";
    std.fs.write_text(path, "hello");
    std.assert(std.fs.is_file(path));
    std.assert(!std.fs.is_dir(path));
    std.assert_eq(std.fs.file_size(path), 5);

    let copied = nested + "/copy.txt";
    std.fs.copy_file(path, copied);
    std.assert_eq(std.fs.read_text(copied), "hello");
    let names = std.fs.list_dir(nested);
    std.assert_eq(std.len(names), 2);
    std.assert_eq(std.get(names, 0), "copy.txt");
    std.assert_eq(std.get(names, 1), "note.txt");

    let moved = nested + "/moved.txt";
    std.fs.rename(copied, moved);
    std.assert(std.fs.exists(moved));
    std.assert(!std.fs.exists(copied));
    std.fs.remove_file(moved);
    std.assert(!std.fs.exists(moved));

    let empty = root + "/empty";
    std.fs.mkdir_all(empty);
    std.fs.remove_dir(empty);
    std.assert(!std.fs.exists(empty));

    std.assert_eq(std.time.format_utc(0), "1970-01-01 00:00:00");
    std.assert_eq(std.time.format_utc(1700000000000), "2023-11-14 22:13:20");
    let parts = std.time.parts(1700000000000);
    std.assert_eq(parts.year, 2023);
    std.assert_eq(parts.month, 11);
    std.assert_eq(parts.day, 14);
    std.assert_eq(parts.hour, 22);
    std.assert_eq(parts.minute, 13);
    std.assert_eq(parts.second, 20);
    std.assert_eq(parts.millisecond, 0);
    std.assert_eq(parts.weekday, 2);

    let args = std.env.args();
    std.assert_eq(std.len(args), 1);
    std.assert_eq(std.get(args, 0), root);
    std.assert(std.env.home() == () || std.type(std.env.home()) == "str");
    std.assert_eq(std.type(std.env.temp_dir()), "str");
    std.assert(std.process.pid() > 0);
    std.assert_eq(std.sys.os(), "linux");
    std.assert_eq(std.type(std.sys.arch()), "str");
}; check();
"#;
    for jit in [false, true] {
        let mut runtime = prepare(
            source,
            Config {
                jit_enabled: jit,
                args: vec![root.to_string_lossy().into_owned()],
                ..Config::default()
            },
        )
        .unwrap();
        runtime
            .run()
            .unwrap_or_else(|e| panic!("{}", e.render("builtin-test.pr", source)));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn snake_logic_in_interpreter_and_jit() {
    let source = include_str!("../examples/snake.pr");
    for jit in [false, true] {
        let mut rt = prepare(
            source,
            Config {
                jit_enabled: jit,
                args: vec!["--self-test".into()],
                gc_threshold: 32,
                max_steps: Some(1_000_000),
                ..Config::default()
            },
        )
        .unwrap();
        assert!(matches!(
            rt.run()
                .unwrap_or_else(|e| panic!("{}", e.render("snake.pr", source))),
            Value::Unit
        ));
        assert!(rt.output.contains("snake self-test: ok"));
        if jit {
            assert_eq!(rt.stats.compiled_functions, 1);
            assert!(rt.stats.native_calls > 10);
        }
    }
}
