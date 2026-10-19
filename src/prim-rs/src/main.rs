use prim::{Config, Error, Runtime, Session, Value, ir, session, syntax};
use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    path::PathBuf,
    process::ExitCode,
    time::Instant,
};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Run,
    Check,
    Tokens,
    Ast,
    Ir,
    Repl,
    Builtins,
    Version,
    Help,
}
struct Options {
    mode: Mode,
    config: Config,
    file: Option<String>,
    evals: Vec<String>,
    interactive: bool,
    result: bool,
    stats: bool,
    time: bool,
    quiet: bool,
    no_prompt: bool,
    json: bool,
    color: String,
    max_source: usize,
}
struct Failure {
    code: u8,
    text: String,
}
type CliResult<T> = std::result::Result<T, Failure>;
fn usage(text: impl Into<String>) -> Failure {
    Failure {
        code: 2,
        text: text.into(),
    }
}
fn execution(text: String) -> Failure {
    Failure { code: 1, text }
}

const HELP: &str = r#"Prim — closure language and explicit LLVM ORC JIT

Usage:
  prim [OPTIONS] FILE.pr [-- SCRIPT_ARGS...]
  prim [OPTIONS] -e CODE [-e CODE ...] [-- SCRIPT_ARGS...]
  prim [OPTIONS] -
  prim [OPTIONS]                       REPL on a terminal; otherwise stdin
  prim <run|check|lex|ast|ir|repl> [OPTIONS] [FILE.pr]
  prim <builtins|version|help>

Source and interactive modes:
  -e, -c, --eval CODE       Evaluate a source unit; repeat to share a session
  -i, --interactive        Enter the REPL after the file/eval units
  --repl                   Start a REPL (also accepts piped command lines)
  --check                  Parse and resolve without executing
  --tokens                 Print tokens without executing
  --ast                    Print AST without executing
  --ir                     Print core IR and root maps without executing
  --result                 Print the last value of each executed input unit
  -q, --quiet              Suppress banner and informational notices
  --no-prompt              Suppress REPL prompts

Execution:
  --jit=on|off              Respect manual @jit (on, default), or interpret all
  --no-jit                 Alias for --jit=off; does not change source semantics
  --jit-log                Log actual native compilation
  -O0, -O1, -O2            JIT optimization level (default -O2)
  --opt-level N            Same as -ON; 0=generic, 1/2=scalar + LLVM O1/O2
  --emit-llvm DIR          Save IR for functions actually JIT compiled
  --gc-threshold N         Positive allocation interval (default 1024)
  --fuel N                 Runtime operation budget per input unit
  --stack-limit N          Positive maximum active frame depth (default 256)
  --max-source-bytes N     Input/session source limit (default 16777216)
  --stats                  Print cumulative runtime/JIT/GC counters
  --time                   Print wall time for each input unit

Diagnostics and discovery:
  --diagnostic-format text|json   Error output format (default text)
  --color auto|always|never       Text diagnostic color (default auto)
  --builtins               List builtin names and exact argument counts
  -V, --version            Print interpreter and LLVM versions
  -h, --help               Show this help

REPL commands:
  :help :quit :reset :vars :del NAME :load FILE :builtins :stats :gc :cancel

Statements retain Prim's semicolon rules. Multiline blocks use continuation
prompts. REPL variables and closures persist; history is never re-executed.
Use -- to pass script arguments. JIT is only selected by @jit/jit(closure).
Exit codes: 0 success, 1 language/runtime error, 2 usage or input-file error.
"#;

fn main() -> ExitCode {
    match parse_options().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if !error.text.is_empty() {
                eprintln!("{}", error.text);
            }
            ExitCode::from(error.code)
        }
    }
}
fn parse_options() -> CliResult<Options> {
    let mut args = env::args_os()
        .skip(1)
        .map(|a| {
            a.into_string()
                .map_err(|_| usage("arguments must be valid UTF-8"))
        })
        .collect::<CliResult<Vec<_>>>()?
        .into_iter()
        .peekable();
    let mut options = Options {
        mode: Mode::Run,
        config: Config {
            echo_output: true,
            capture_output: false,
            ..Config::default()
        },
        file: None,
        evals: vec![],
        interactive: false,
        result: false,
        stats: false,
        time: false,
        quiet: false,
        no_prompt: false,
        json: false,
        color: "auto".into(),
        max_source: 16 * 1024 * 1024,
    };
    if let Some(first) = args.peek() {
        let mode = match first.as_str() {
            "run" => Some(Mode::Run),
            "check" => Some(Mode::Check),
            "lex" | "tokens" => Some(Mode::Tokens),
            "ast" | "parse" => Some(Mode::Ast),
            "ir" => Some(Mode::Ir),
            "repl" => Some(Mode::Repl),
            "builtins" => Some(Mode::Builtins),
            "version" => Some(Mode::Version),
            "help" => Some(Mode::Help),
            _ => None,
        };
        if let Some(mode) = mode {
            options.mode = mode;
            args.next();
        }
    }
    while let Some(arg) = args.next() {
        if arg == "--" {
            if options.file.is_none()
                && options.evals.is_empty()
                && options.mode != Mode::Repl
                && !options.interactive
            {
                options.file = args.next();
            }
            options.config.args.extend(args);
            break;
        }
        let (key, attached) = if arg.starts_with("--") {
            arg.split_once('=')
                .map_or((arg.as_str(), None), |(k, v)| (k, Some(v.to_owned())))
        } else {
            (arg.as_str(), None)
        };
        let mut value = || {
            attached
                .clone()
                .or_else(|| args.next())
                .ok_or_else(|| usage(format!("{key} requires a value")))
        };
        match key {
            "-h" | "--help" => options.mode = Mode::Help,
            "-V" | "--version" => options.mode = Mode::Version,
            "-e" | "-c" | "--eval" => options.evals.push(value()?),
            "-i" | "--interactive" => options.interactive = true,
            "--repl" => options.mode = Mode::Repl,
            "--check" => {
                if !matches!(options.mode, Mode::Tokens | Mode::Ast | Mode::Ir) {
                    options.mode = Mode::Check;
                }
            }
            "--tokens" | "--lexer-only" => options.mode = Mode::Tokens,
            "--ast" => options.mode = Mode::Ast,
            "--ir" => options.mode = Mode::Ir,
            "--result" => options.result = true,
            "--stats" => options.stats = true,
            "--time" => options.time = true,
            "-q" | "--quiet" => options.quiet = true,
            "--no-prompt" => options.no_prompt = true,
            "--builtins" => options.mode = Mode::Builtins,
            "--jit-log" => options.config.jit_log = true,
            "-O0" => options.config.opt_level = 0,
            "-O1" => options.config.opt_level = 1,
            "-O2" => options.config.opt_level = 2,
            "-O" | "--opt-level" => {
                options.config.opt_level = match value()?.as_str() {
                    "0" => 0, "1" => 1, "2" => 2,
                    _ => return Err(usage("--opt-level must be 0, 1 or 2")),
                };
            }
            "--no-jit" => options.config.jit_enabled = false,
            "--jit" => {
                options.config.jit_enabled = match value()?.as_str() {
                    "on" => true,
                    "off" => false,
                    _ => return Err(usage("--jit must be on or off")),
                }
            }
            "--emit-llvm" => options.config.ir_dir = Some(PathBuf::from(value()?)),
            "--gc-threshold" => options.config.gc_threshold = positive(&value()?, key)?,
            "--stack-limit" | "--max-depth" => {
                options.config.max_call_depth = positive(&value()?, key)?
            }
            "--fuel" => {
                options.config.max_steps = Some(
                    value()?
                        .parse()
                        .map_err(|_| usage("--fuel requires a nonnegative integer"))?,
                )
            }
            "--max-source-bytes" => options.max_source = positive(&value()?, key)?,
            "--diagnostic-format" => {
                options.json = match value()?.as_str() {
                    "text" => false,
                    "json" => true,
                    _ => return Err(usage("diagnostic format must be text or json")),
                }
            }
            "--color" => {
                let color = value()?;
                if !matches!(color.as_str(), "auto" | "always" | "never") {
                    return Err(usage("color must be auto, always or never"));
                }
                options.color = color;
            }
            "-" => {
                if options.file.replace(arg).is_some() {
                    return Err(usage("only one input file; use -- for script arguments"));
                }
            }
            a if a.starts_with("-e") && !a.starts_with("--") && a.len() > 2 => {
                options.evals.push(a[2..].into())
            }
            a if a.starts_with('-') => {
                return Err(usage(format!("unknown option {a}; use --help")));
            }
            _ => {
                if options.file.replace(arg).is_some() {
                    return Err(usage("only one input file; use -- for script arguments"));
                }
            }
        }
    }
    if matches!(options.mode, Mode::Help | Mode::Version | Mode::Builtins) {
        return Ok(options);
    }
    if options.file.is_some() && !options.evals.is_empty() {
        return Err(usage("choose a file or -e inputs"));
    }
    if !matches!(options.mode, Mode::Run | Mode::Repl)
        && (options.interactive || options.evals.len() > 1)
    {
        return Err(usage(
            "inspection modes accept one source and cannot start a REPL",
        ));
    }
    if options.file.is_none() && options.evals.is_empty() && options.mode == Mode::Run {
        if options.interactive || io::stdin().is_terminal() {
            options.mode = Mode::Repl;
        } else {
            options.file = Some("-".into());
        }
    }
    Ok(options)
}
fn positive(value: &str, name: &str) -> CliResult<usize> {
    value
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| usage(format!("{name} requires a positive integer")))
}
fn read_source(path: &str, limit: usize) -> CliResult<String> {
    let mut bytes = vec![];
    if path == "-" {
        io::stdin()
            .take(limit.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| usage(format!("stdin: {e}")))?;
    } else {
        fs::File::open(path)
            .map_err(|e| usage(format!("{path}: {e}")))?
            .take(limit.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| usage(format!("{path}: {e}")))?;
    }
    if bytes.len() > limit {
        return Err(usage(format!("source exceeds {limit} bytes")));
    }
    String::from_utf8(bytes).map_err(|_| usage(format!("{path}: source is not valid UTF-8")))
}
fn run(options: Options) -> CliResult<()> {
    match options.mode {
        Mode::Help => {
            print!("{HELP}");
            return Ok(());
        }
        Mode::Version => {
            println!(
                "prim {} (LLVM {}, ORC LLJIT)",
                env!("CARGO_PKG_VERSION"),
                env!("PRIM_LLVM_VERSION")
            );
            return Ok(());
        }
        Mode::Builtins => {
            print_builtins();
            return Ok(());
        }
        _ => {}
    }
    if !options.config.jit_enabled && !options.quiet {
        eprintln!("[jit] explicitly disabled; marked closures will be interpreted");
    }
    let mut inputs = vec![];
    if let Some(path) = &options.file {
        inputs.push((
            if path == "-" {
                "<stdin>".into()
            } else {
                path.clone()
            },
            read_source(path, options.max_source)?,
        ));
    }
    for (index, source) in options.evals.iter().enumerate() {
        if source.len() > options.max_source {
            return Err(usage("eval source exceeds --max-source-bytes"));
        }
        inputs.push((format!("<eval:{}>", index + 1), source.clone()));
    }
    if !matches!(options.mode, Mode::Run | Mode::Repl) {
        if inputs.is_empty() {
            inputs.push(("<stdin>".into(), read_source("-", options.max_source)?));
        }
        let (name, source) = &inputs[0];
        let start = Instant::now();
        let inspected = inspect(source, options.mode);
        if options.time {
            print_time(start);
        }
        return inspected.map_err(|e| execution(format_error(&options, name, source, &e)));
    }
    let interactive = options.mode == Mode::Repl || options.interactive;
    if interactive || !options.evals.is_empty() {
        let mut session = Session::new(options.config.clone());
        session.max_source_bytes = options.max_source;
        let mut initial_error = false;
        for (name, source) in inputs {
            let start = Instant::now();
            let result = session.evaluate(&name, &source);
            if options.time {
                print_time(start);
            }
            match result {
                Ok(value) => {
                    if options.result {
                        println!("{}", value.display());
                    }
                }
                Err(error) => {
                    let text = session_error(&options, &session, &error);
                    if interactive {
                        initial_error = true;
                        eprintln!("{text}");
                    } else {
                        if options.stats {
                            print_stats(&session.runtime);
                        }
                        return Err(execution(text));
                    }
                }
            }
        }
        let result = if interactive {
            repl(&mut session, &options)
        } else {
            Ok(())
        };
        if options.stats {
            print_stats(&session.runtime);
        }
        if initial_error && !io::stdin().is_terminal() && result.is_ok() {
            return Err(execution(String::new()));
        }
        return result;
    }
    let (name, source) = inputs.pop().ok_or_else(|| usage("no source supplied"))?;
    let start = Instant::now();
    let mut runtime = prim::prepare_named(&name, &source, options.config.clone())
        .map_err(|e| execution(format_error(&options, &name, &source, &e)))?;
    let result = runtime.run();
    if options.time {
        print_time(start);
    }
    if options.stats {
        print_stats(&runtime);
    }
    let value = result.map_err(|e| execution(format_error(&options, &name, &source, &e)))?;
    if options.result {
        println!("{}", value.display());
    }
    Ok(())
}
fn inspect(source: &str, mode: Mode) -> syntax::Result<()> {
    let tokens = syntax::lex(source)?;
    if mode == Mode::Tokens {
        println!("{tokens:#?}");
        return Ok(());
    }
    let body = syntax::parse(tokens)?;
    if mode == Mode::Ast {
        println!("{body:#?}");
    }
    let program = ir::compile(&body)?;
    if mode == Mode::Ir {
        println!("{program:#?}");
    }
    Ok(())
}
fn print_builtins() {
    for (name, arity) in Runtime::builtin_signatures() {
        println!("{name}/{arity}");
    }
}
fn print_time(start: Instant) {
    eprintln!(
        "[time] elapsed_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
}
fn print_stats(runtime: &Runtime) {
    let s = &runtime.stats;
    eprintln!(
        "[stats] compiled={} native_calls={} native_integer_ops={} interpreted_calls={} compile_ms={:.3} gc_collections={} live_slots={} live_closures={} live_strings={} string_bytes={} steps={} helper_calls={} scalar_calls={} scalar_fallbacks={} slot_allocations={} closure_allocations={} gc_ms={:.3} native_code_bytes={}",
        s.compiled_functions,
        s.native_calls,
        s.native_integer_ops,
        s.interpreted_calls,
        s.compile_ms,
        s.gc_collections,
        s.live_slots,
        s.live_closures,
        s.live_strings,
        s.live_string_bytes,
        s.steps, s.native_helper_calls, s.scalar_calls, s.scalar_fallbacks,
        s.slot_allocations, s.closure_allocations, s.gc_ms, s.native_code_bytes
    );
}
fn quote(s: &str) -> String {
    let mut result = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c < ' ' => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}
fn format_error(options: &Options, name: &str, source: &str, error: &Error) -> String {
    if options.json {
        let (line, column) = syntax::location(source, error.span.start);
        format!(
            "{{\"kind\":{},\"message\":{},\"file\":{},\"line\":{line},\"column\":{column},\"start\":{},\"end\":{}}}",
            quote(&error.kind),
            quote(&error.message),
            quote(name),
            error.span.start,
            error.span.end
        )
    } else {
        colorize(options, error.render(name, source))
    }
}
fn colorize(options: &Options, text: String) -> String {
    let color = options.color == "always"
        || options.color == "auto"
            && io::stderr().is_terminal()
            && env::var_os("NO_COLOR").is_none();
    if color {
        if let Some((header, rest)) = text.split_once('\n') {
            format!("\x1b[31m{header}\x1b[0m\n{rest}")
        } else {
            format!("\x1b[31m{text}\x1b[0m")
        }
    } else {
        text
    }
}
fn session_error(options: &Options, session: &Session, error: &Error) -> String {
    if options.json {
        let (name, source, local) = session.error_source(error);
        format_error(options, name, source, &local)
    } else {
        colorize(options, session.render_error(error))
    }
}
fn repl(session: &mut Session, options: &Options) -> CliResult<()> {
    let tty = io::stdin().is_terminal();
    let prompts = tty && io::stdout().is_terminal() && !options.no_prompt;
    if tty && !options.quiet {
        println!(
            "Prim {} / LLVM {} — :help for commands",
            env!("CARGO_PKG_VERSION"),
            env!("PRIM_LLVM_VERSION")
        );
    }
    let mut buffer = String::new();
    let mut unit = 0usize;
    let mut failed = false;
    loop {
        if prompts {
            print!("{}", if buffer.is_empty() { ">>> " } else { "... " });
            io::stdout().flush().map_err(|e| usage(e.to_string()))?;
        }
        let line = session::read_console_line().map_err(|e| usage(e.to_string()))?;
        let Some(line) = line else {
            if !buffer.trim().is_empty() {
                unit += 1;
                evaluate_repl(session, options, &buffer, unit, &mut failed);
            }
            break;
        };
        let trimmed = line.trim();
        if trimmed == ":quit" || trimmed == ":q" {
            break;
        }
        if trimmed == ":cancel" {
            buffer.clear();
            continue;
        }
        if buffer.is_empty() && trimmed.starts_with(':') {
            let (command, argument) = trimmed
                .split_once(char::is_whitespace)
                .map_or((trimmed, ""), |(a, b)| (a, b.trim()));
            match command {
                ":help" => println!(
                    ":help :quit :reset :vars :del NAME :load FILE :builtins :stats :gc :cancel\nUse semicolons for statements. Blocks may span lines. Results are printed automatically."
                ),
                ":reset" => {
                    session.reset();
                    if !options.quiet {
                        println!("Session reset.");
                    }
                }
                ":vars" => {
                    for (name, value) in session.runtime.global_values() {
                        println!("{name} = {}", value.display());
                    }
                }
                ":del" => {
                    if !session.runtime.forget_global(argument) {
                        eprintln!("No such global: {argument}");
                        failed = true;
                    }
                }
                ":load" => match read_source(argument, options.max_source) {
                    Ok(source) => {
                        unit += 1;
                        let start = Instant::now();
                        match session.evaluate(argument, &source) {
                            Ok(v) => {
                                if !matches!(v, Value::Unit) {
                                    println!("{}", v.display());
                                }
                            }
                            Err(e) => {
                                eprintln!("{}", session_error(options, session, &e));
                                failed = true;
                            }
                        }
                        if options.time {
                            print_time(start);
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", e.text);
                        failed = true;
                    }
                },
                ":builtins" => print_builtins(),
                ":stats" => print_stats(&session.runtime),
                ":gc" => {
                    session.runtime.collect();
                    print_stats(&session.runtime);
                }
                _ => {
                    eprintln!("Unknown REPL command: {command}; use :help");
                    failed = true;
                }
            }
            continue;
        }
        if buffer.is_empty() && trimmed.is_empty() {
            continue;
        }
        if buffer.len().saturating_add(line.len() + 1) > options.max_source {
            buffer.clear();
            failed = true;
            eprintln!("SourceLimit: input exceeds --max-source-bytes");
            continue;
        }
        buffer.push_str(&line);
        buffer.push('\n');
        if session::needs_more_input(&buffer) {
            continue;
        }
        unit += 1;
        evaluate_repl(session, options, &buffer, unit, &mut failed);
        buffer.clear();
    }
    if failed && !tty {
        Err(execution(String::new()))
    } else {
        Ok(())
    }
}
fn evaluate_repl(
    session: &mut Session,
    options: &Options,
    source: &str,
    index: usize,
    failed: &mut bool,
) {
    let start = Instant::now();
    match session.evaluate(&format!("<repl:{index}>"), source) {
        Ok(value) => {
            if !matches!(value, Value::Unit) {
                println!("{}", value.display());
            }
        }
        Err(error) => {
            eprintln!("{}", session_error(options, session, &error));
            *failed = true;
        }
    }
    if options.time {
        print_time(start);
    }
}
