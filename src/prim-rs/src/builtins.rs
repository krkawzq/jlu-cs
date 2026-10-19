use super::{Cell, Closure, Code, Runtime, Value, finite, overflow};
use crate::{
    syntax::{Error, Result, Span},
    terminal::Terminal,
};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::Path,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

macro_rules! registry {
    ($($variant:ident => ($name:literal,$arity:literal)),* $(,)?) => {
        #[derive(Clone,Copy,Debug)] pub(super) enum Builtin {$($variant),*}
        impl Builtin {
            pub fn name(self)->&'static str {match self {$(Self::$variant=>$name),*}}
            pub fn arity(self)->usize {match self {$(Self::$variant=>$arity),*}}
        }
        const ALL:&[Builtin]=&[$(Builtin::$variant),*];
    }
}
registry! {
    // Source, syntax, function, and native-code inspection.
    AstTokens=>("std.ast.tokens",1),
    AstParse=>("std.ast.parse",1),
    AstDump=>("std.ast.dump",1),
    AstWalk=>("std.ast.walk",1),
    AstFind=>("std.ast.find",2),
    AstSegment=>("std.ast.source_segment",2),
    AstIsNode=>("std.ast.is_node",1),
    InspectIsCallable=>("std.inspect.is_callable",1),
    InspectIsFunction=>("std.inspect.is_function",1),
    InspectIsBuiltin=>("std.inspect.is_builtin",1),
    InspectIsJit=>("std.inspect.is_jit",1),
    InspectDescribe=>("std.inspect.describe",1),
    InspectMembers=>("std.inspect.members",1),
    InspectSignature=>("std.inspect.signature",1),
    InspectCaptures=>("std.inspect.captures",1),
    InspectSource=>("std.inspect.source",1),
    InspectLocation=>("std.inspect.location",1),
    InspectAst=>("std.inspect.ast",1),
    InspectCoreIr=>("std.inspect.core_ir",1),
    InspectStack=>("std.inspect.stack",0),
    InspectBuiltins=>("std.inspect.builtins",0),
    InspectJit=>("std.inspect.jit",1),
    InspectCompile=>("std.inspect.compile",1),
    InspectOptimizedIr=>("std.inspect.optimized_ir",1),
    InspectLlvmIr=>("std.inspect.llvm_ir",1),
    InspectMachineCode=>("std.inspect.machine_code",1),
    InspectDisassemble=>("std.inspect.disassemble",1),
    InspectNative=>("std.inspect.native",1),

    // Output and input.
    Print=>("std.print",1), Write=>("std.write",1), Flush=>("std.flush",0), ReadLine=>("std.read_line",0), Input=>("std.input",1),
    EPrint=>("std.eprint",1), EWrite=>("std.ewrite",1), EFlush=>("std.eflush",0), ReadAll=>("std.read_all",0),
    // Assertions, conversions, and array access.
    Assert=>("std.assert",1), AssertEq=>("std.assert_eq",2), Fail=>("std.fail",1),
    Type=>("std.type",1), Str=>("std.str",1), Int=>("std.int",1), Float=>("std.float",1),
    ParseInt=>("std.parse_int",1), ParseFloat=>("std.parse_float",1), Len=>("std.len",1),
    Array=>("std.array",2), Get=>("std.get",2), Set=>("std.set",3), Has=>("std.has",2), Jit=>("jit",1),
    // Memory diagnostics.
    GcCollect=>("std.gc.collect",0), GcStats=>("std.gc.stats",0),
    // Time and calendar.
    NowMs=>("std.time.now_ms",0), UnixMs=>("std.time.unix_ms",0), Sleep=>("std.time.sleep",1),
    FormatUtc=>("std.time.format_utc",1), TimeParts=>("std.time.parts",1),
    // Random numbers.
    Seed=>("std.random.seed",1), RandomInt=>("std.random.int",2), RandomFloat=>("std.random.float",0),
    RandomBool=>("std.random.bool",0), RandomFloatRange=>("std.random.float_range",2),
    RandomChoice=>("std.random.choice",1), RandomShuffle=>("std.random.shuffle",1),
    RandomBytes=>("std.random.bytes",1), RandomNormal=>("std.random.normal",0),
    // Text.
    TextAt=>("std.text.at",2), Slice=>("std.text.slice",3), Repeat=>("std.text.repeat",2), Trim=>("std.text.trim",1),
    Contains=>("std.text.contains",2), StartsWith=>("std.text.starts_with",2), EndsWith=>("std.text.ends_with",2), Replace=>("std.text.replace",3),
    IndexOf=>("std.text.index_of",2), LastIndexOf=>("std.text.last_index_of",2), Split=>("std.text.split",2),
    Lines=>("std.text.lines",1), Join=>("std.text.join",2), Upper=>("std.text.upper",1), Lower=>("std.text.lower",1),
    PadStart=>("std.text.pad_start",3), PadEnd=>("std.text.pad_end",3), CodeAt=>("std.text.code_at",2),
    FromCode=>("std.text.from_code",1), Fixed=>("std.text.fixed",2), ParseIntRadix=>("std.text.parse_int_radix",2),
    // Math.
    Abs=>("std.math.abs",1), Min=>("std.math.min",2), Max=>("std.math.max",2), Clamp=>("std.math.clamp",3),
    Floor=>("std.math.floor",1), Ceil=>("std.math.ceil",1), Round=>("std.math.round",1), Sqrt=>("std.math.sqrt",1),
    Pow=>("std.math.pow",2), Sin=>("std.math.sin",1), Cos=>("std.math.cos",1), Tan=>("std.math.tan",1),
    Asin=>("std.math.asin",1), Acos=>("std.math.acos",1), Atan=>("std.math.atan",1), Atan2=>("std.math.atan2",2),
    Exp=>("std.math.exp",1), Ln=>("std.math.ln",1), Log=>("std.math.log",2), Log2=>("std.math.log2",1),
    Log10=>("std.math.log10",1), Trunc=>("std.math.trunc",1), Hypot=>("std.math.hypot",2), Pi=>("std.math.pi",0),
    Sign=>("std.math.sign",1),
    // Bit operations.
    BitAnd=>("std.bits.and",2), BitOr=>("std.bits.or",2), BitXor=>("std.bits.xor",2), BitNot=>("std.bits.not",1),
    Shl=>("std.bits.shl",2), Shr=>("std.bits.shr",2), CountOnes=>("std.bits.count_ones",1),
    LeadingZeros=>("std.bits.leading_zeros",1), TrailingZeros=>("std.bits.trailing_zeros",1),
    // Closure sequences.
    SeqSlice=>("std.seq.slice",3), SeqReverse=>("std.seq.reverse",1), SeqSort=>("std.seq.sort",1),
    SeqFill=>("std.seq.fill",2), SeqIndexOf=>("std.seq.index_of",2), SeqCopy=>("std.seq.copy",1),
    SeqConcat=>("std.seq.concat",2),
    // Script environment and process.
    Argc=>("std.env.argc",0), Arg=>("std.env.arg",1), EnvGet=>("std.env.get",1), Cwd=>("std.env.cwd",0),
    EnvArgs=>("std.env.args",0), EnvHome=>("std.env.home",0), EnvTempDir=>("std.env.temp_dir",0),
    ProcessPid=>("std.process.pid",0), ProcessExit=>("std.process.exit",1),
    // Files.
    ReadText=>("std.fs.read_text",1), WriteText=>("std.fs.write_text",2), AppendText=>("std.fs.append_text",2), Exists=>("std.fs.exists",1),
    ListDir=>("std.fs.list_dir",1), MkdirAll=>("std.fs.mkdir_all",1), RemoveFile=>("std.fs.remove_file",1),
    RemoveDir=>("std.fs.remove_dir",1), Rename=>("std.fs.rename",2), CopyFile=>("std.fs.copy_file",2),
    FileSize=>("std.fs.file_size",1), IsFile=>("std.fs.is_file",1), IsDir=>("std.fs.is_dir",1),
    // Terminal.
    TermEnter=>("std.term.enter",0), TermLeave=>("std.term.leave",0), IsTty=>("std.term.is_tty",0), TermSize=>("std.term.size",0),
    Clear=>("std.term.clear",0), Home=>("std.term.home",0), Move=>("std.term.move",2), Cursor=>("std.term.cursor",1),
    Color=>("std.term.color",2), Reset=>("std.term.reset",0), PollKey=>("std.term.poll_key",1),
    ColorRgb=>("std.term.color_rgb",2), Bell=>("std.term.bell",0), Title=>("std.term.title",1),
    // Host information.
    SysOs=>("std.sys.os",0), SysArch=>("std.sys.arch",0),
}
const MAX_TEXT: usize = 16 * 1024 * 1024;
const MAX_ARRAY: usize = 1_000_000;
fn string(s: impl Into<String>) -> Value {
    Value::Str(Rc::new(s.into()))
}
fn io_error(e: std::io::Error, span: Span) -> Error {
    Error::new(
        if e.kind() == std::io::ErrorKind::Interrupted {
            "Interrupted"
        } else {
            "IoError"
        },
        e.to_string(),
        span,
    )
}
struct Args<'a> {
    values: &'a [Value],
    span: Span,
}
impl<'a> Args<'a> {
    fn int(&self, i: usize) -> Result<i64> {
        if let Value::Int(n) = self.values[i] {
            Ok(n)
        } else {
            Err(Error::new(
                "TypeError",
                format!("argument {} requires int", i + 1),
                self.span,
            ))
        }
    }
    fn float(&self, i: usize) -> Result<f64> {
        if let Value::Float(n) = self.values[i] {
            Ok(n)
        } else {
            Err(Error::new(
                "TypeError",
                format!("argument {} requires float", i + 1),
                self.span,
            ))
        }
    }
    fn text(&self, i: usize) -> Result<&'a str> {
        if let Value::Str(s) = &self.values[i] {
            Ok(s)
        } else {
            Err(Error::new(
                "TypeError",
                format!("argument {} requires str", i + 1),
                self.span,
            ))
        }
    }
    fn boolean(&self, i: usize) -> Result<bool> {
        if let Value::Bool(b) = self.values[i] {
            Ok(b)
        } else {
            Err(Error::new(
                "TypeError",
                format!("argument {} requires bool", i + 1),
                self.span,
            ))
        }
    }
    fn closure(&self, i: usize) -> Result<usize> {
        if let Value::Closure(id) = self.values[i] {
            Ok(id)
        } else {
            Err(Error::new(
                "TypeError",
                format!("argument {} requires closure", i + 1),
                self.span,
            ))
        }
    }
    fn index(&self, i: usize) -> Result<usize> {
        usize::try_from(self.int(i)?)
            .map_err(|_| Error::new("IndexError", "index must be nonnegative", self.span))
    }
}
impl Runtime {
    pub fn builtin_signatures() -> Vec<(&'static str, usize)> {
        ALL.iter().map(|b| (b.name(), b.arity())).collect()
    }
    pub(super) fn install_builtins(&mut self) {
        let mut fields = vec![];
        let mut modules: BTreeMap<&str, Vec<(String, usize)>> = BTreeMap::new();
        for &builtin in ALL {
            let closure = self.new_closure(Closure {
                fields: vec![],
                code: Code::Builtin(builtin),
                jit: false,
                marked: Span::default(),
            });
            let slot = self.new_slot(Value::Closure(closure), false);
            if builtin.name() == "jit" {
                self.prelude[1] = slot;
                continue;
            }
            let name = builtin.name().strip_prefix("std.").unwrap();
            if let Some((module, method)) = name.split_once('.') {
                modules
                    .entry(module)
                    .or_default()
                    .push((method.into(), slot));
            } else {
                fields.push((name.into(), slot));
            }
        }
        for (module, methods) in modules {
            let c = self.new_closure(Closure {
                fields: methods,
                code: Code::Data,
                jit: false,
                marked: Span::default(),
            });
            let s = self.new_slot(Value::Closure(c), false);
            fields.push((module.into(), s));
        }
        let std = self.new_closure(Closure {
            fields,
            code: Code::Data,
            jit: false,
            marked: Span::default(),
        });
        self.prelude[0] = self.new_slot(Value::Closure(std), false);
    }
    fn emit_output(&mut self, text: &str, span: Span) -> Result<()> {
        if self.config.echo_output {
            std::io::stdout()
                .lock()
                .write_all(text.as_bytes())
                .map_err(|e| io_error(e, span))?;
        }
        if self.config.capture_output {
            self.output.push_str(text);
        }
        Ok(())
    }
    fn flush_output(&self, span: Span) -> Result<()> {
        if self.config.echo_output {
            std::io::stdout().flush().map_err(|e| io_error(e, span))?;
        }
        Ok(())
    }
    fn record(&mut self, fields: Vec<(&str, Value)>) -> Value {
        let fields = fields
            .into_iter()
            .map(|(name, value)| (name.into(), self.new_slot(value, true)))
            .collect();
        Value::Closure(self.new_closure(Closure {
            fields,
            code: Code::Data,
            jit: false,
            marked: Span::default(),
        }))
    }
    fn array_value(&mut self, values: Vec<Value>, span: Span) -> Value {
        let fields = values
            .into_iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), self.new_slot(value, true)))
            .collect();
        Value::Closure(self.new_closure(Closure {
            fields,
            code: Code::Data,
            jit: false,
            marked: span,
        }))
    }
    fn closure_values(&self, id: usize) -> Vec<Value> {
        self.closure(id)
            .fields
            .iter()
            .map(|(_, slot)| self.slot(*slot).value.clone())
            .collect()
    }
    fn writable_slots(&self, id: usize, span: Span) -> Result<Vec<usize>> {
        self.closure(id)
            .fields
            .iter()
            .map(|(name, slot)| {
                if self.slot(*slot).writable {
                    Ok(*slot)
                } else {
                    Err(Error::new(
                        "ReadOnlySlot",
                        format!("field '{name}' is read-only"),
                        span,
                    ))
                }
            })
            .collect()
    }
    fn stderr_output(&self, text: &str, span: Span) -> Result<()> {
        std::io::stderr()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|e| io_error(e, span))
    }
    fn random_below(&mut self, limit: usize) -> usize {
        let range = limit as u64;
        let threshold = range.wrapping_neg() % range;
        loop {
            let word = self.random_word();
            if word >= threshold {
                return (word % range) as usize;
            }
        }
    }
    fn swap_slots(&mut self, left: usize, right: usize) {
        if left == right {
            return;
        }
        let (low, high) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        let (before, after) = self.slots.split_at_mut(high);
        std::mem::swap(
            &mut before[low].as_mut().unwrap().value,
            &mut after[0].as_mut().unwrap().value,
        );
    }
    fn field_key(&self, id: usize, key: &Value, span: Span) -> Result<(String, Option<usize>)> {
        let name = match key {
            Value::Str(s) => (**s).clone(),
            Value::Int(n) if *n >= 0 => n.to_string(),
            Value::Int(_) => {
                return Err(Error::new("IndexError", "index must be nonnegative", span));
            }
            _ => {
                return Err(Error::new(
                    "TypeError",
                    "field key requires str or nonnegative int",
                    span,
                ));
            }
        };
        let fields = &self.closure(id).fields;
        let direct = if let Value::Int(n) = key {
            usize::try_from(*n)
                .ok()
                .and_then(|i| fields.get(i))
                .filter(|(k, _)| k == &name)
                .map(|(_, slot)| *slot)
        } else {
            None
        };
        let slot = direct.or_else(|| fields.iter().find(|(k, _)| k == &name).map(|(_, s)| *s));
        Ok((name, slot))
    }
    fn random_word(&mut self) -> u64 {
        // SplitMix64 keeps all seed bit patterns valid and gives reproducible per-runtime streams.
        self.random_state = self.random_state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.random_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn terminal_control(&mut self, text: &str, span: Span) -> Result<Value> {
        if !self.terminal.active() {
            return Err(Error::new(
                "TerminalError",
                "call std.term.enter() first",
                span,
            ));
        }
        self.emit_output(text, span)?;
        Ok(Value::Unit)
    }
    pub(super) fn builtin(&mut self, b: Builtin, args: Vec<Cell>, span: Span) -> Result<Value> {
        let values = args
            .into_iter()
            .map(|a| match a {
                Cell::Value(v) => Ok(v),
                _ => Err(Error::new(
                    "ArgumentModeError",
                    "builtins take value arguments",
                    span,
                )),
            })
            .collect::<Result<Vec<_>>>()?;
        let a = Args {
            values: &values,
            span,
        };
        let result = match b {
            Builtin::AstTokens
            | Builtin::AstParse
            | Builtin::AstDump
            | Builtin::AstWalk
            | Builtin::AstFind
            | Builtin::AstSegment
            | Builtin::AstIsNode
            | Builtin::InspectIsCallable
            | Builtin::InspectIsFunction
            | Builtin::InspectIsBuiltin
            | Builtin::InspectIsJit
            | Builtin::InspectDescribe
            | Builtin::InspectMembers
            | Builtin::InspectSignature
            | Builtin::InspectCaptures
            | Builtin::InspectSource
            | Builtin::InspectLocation
            | Builtin::InspectAst
            | Builtin::InspectCoreIr
            | Builtin::InspectStack
            | Builtin::InspectBuiltins
            | Builtin::InspectJit
            | Builtin::InspectCompile
            | Builtin::InspectOptimizedIr
            | Builtin::InspectLlvmIr
            | Builtin::InspectMachineCode
            | Builtin::InspectDisassemble
            | Builtin::InspectNative => return self.inspect_builtin(b, values, span),
            Builtin::Print | Builtin::Write => {
                let mut text = values[0].display();
                if matches!(b, Builtin::Print) {
                    text.push('\n');
                }
                self.emit_output(&text, span)?;
                Value::Unit
            }
            Builtin::Flush => {
                self.flush_output(span)?;
                Value::Unit
            }
            Builtin::ReadLine | Builtin::Input => {
                if matches!(b, Builtin::Input) {
                    let prompt = a.text(0)?;
                    self.emit_output(prompt, span)?;
                }
                self.flush_output(span)?;
                self.terminal
                    .read_line()
                    .map_err(|e| io_error(e, span))?
                    .map_or(Value::Unit, string)
            }
            Builtin::ReadAll => {
                if self.terminal.active() {
                    return Err(Error::new(
                        "TerminalError",
                        "leave terminal mode before reading stdin",
                        span,
                    ));
                }
                let mut bytes = vec![];
                std::io::stdin()
                    .lock()
                    .take(MAX_TEXT as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| io_error(e, span))?;
                if bytes.len() > MAX_TEXT {
                    return Err(Error::new("ResourceLimit", "stdin exceeds 16 MiB", span));
                }
                string(
                    String::from_utf8(bytes)
                        .map_err(|_| Error::new("IoError", "stdin is not valid UTF-8", span))?,
                )
            }
            Builtin::EPrint | Builtin::EWrite => {
                let mut text = values[0].display();
                if matches!(b, Builtin::EPrint) {
                    text.push('\n');
                }
                self.stderr_output(&text, span)?;
                Value::Unit
            }
            Builtin::EFlush => {
                std::io::stderr().flush().map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::Assert => {
                if !a.boolean(0)? {
                    return Err(Error::new("AssertionFailed", "assertion is false", span));
                }
                Value::Unit
            }
            Builtin::AssertEq => {
                if !values[0].equals(&values[1]) {
                    return Err(Error::new(
                        "AssertionFailed",
                        format!(
                            "assert_eq failed: expected {}, got {}",
                            values[1].display(),
                            values[0].display()
                        ),
                        span,
                    ));
                }
                Value::Unit
            }
            Builtin::Fail => {
                return Err(Error::new("UserError", a.text(0)?.to_string(), span));
            }
            Builtin::Type => string(values[0].kind()),
            Builtin::Str => string(values[0].display()),
            Builtin::Int => match values[0] {
                Value::Int(n) => Value::Int(n),
                Value::Float(n) => {
                    let n = n.trunc();
                    if n < i64::MIN as f64 || n >= 9223372036854775808.0 {
                        return Err(overflow(span));
                    }
                    Value::Int(n as i64)
                }
                _ => return Err(self.type_error("int conversion requires a number", span)),
            },
            Builtin::Float => match values[0] {
                Value::Int(n) => Value::Float(n as f64),
                Value::Float(n) => Value::Float(n),
                _ => return Err(self.type_error("float conversion requires a number", span)),
            },
            Builtin::ParseInt => a
                .text(0)?
                .trim()
                .parse::<i64>()
                .ok()
                .map_or(Value::Unit, Value::Int),
            Builtin::ParseFloat => a
                .text(0)?
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map_or(Value::Unit, Value::Float),
            Builtin::Len => match &values[0] {
                Value::Str(s) => Value::Int(s.chars().count() as i64),
                Value::Closure(id) => Value::Int(self.closure(*id).fields.len() as i64),
                _ => return Err(self.type_error("len requires str or closure", span)),
            },
            Builtin::Array => {
                let n = a.index(0)?;
                if n > MAX_ARRAY {
                    return Err(Error::new(
                        "ResourceLimit",
                        "array length exceeds 1000000",
                        span,
                    ));
                }
                let fields = (0..n)
                    .map(|i| (i.to_string(), self.new_slot(values[1].clone(), true)))
                    .collect();
                Value::Closure(self.new_closure(Closure {
                    fields,
                    code: Code::Data,
                    jit: false,
                    marked: span,
                }))
            }
            Builtin::Get | Builtin::Set | Builtin::Has => {
                let id = a.closure(0)?;
                let (name, slot) = self.field_key(id, &values[1], span)?;
                if matches!(b, Builtin::Has) {
                    Value::Bool(slot.is_some())
                } else {
                    let slot = slot.ok_or_else(|| {
                        Error::new(
                            if matches!(values[1], Value::Int(_)) {
                                "IndexError"
                            } else {
                                "MissingField"
                            },
                            format!("field '{name}' does not exist"),
                            span,
                        )
                    })?;
                    if matches!(b, Builtin::Get) {
                        self.slot(slot).value.clone()
                    } else {
                        if !self.slot(slot).writable {
                            return Err(Error::new(
                                "ReadOnlySlot",
                                format!("field '{name}' is read-only"),
                                span,
                            ));
                        }
                        self.slots[slot].as_mut().unwrap().value = values[2].clone();
                        Value::Unit
                    }
                }
            }
            Builtin::Jit => {
                let Value::Closure(id) = values[0] else {
                    return Err(Error::new(
                        "JitTargetError",
                        "jit requires a user callable closure",
                        span,
                    ));
                };
                let mut c = self.closure(id).clone();
                if !matches!(c.code, Code::User(_)) {
                    return Err(Error::new(
                        "JitTargetError",
                        "jit requires a user callable closure",
                        span,
                    ));
                }
                if c.jit {
                    Value::Closure(id)
                } else {
                    c.jit = true;
                    c.marked = span;
                    Value::Closure(self.new_closure(c))
                }
            }
            Builtin::GcCollect => {
                self.collect();
                Value::Unit
            }
            Builtin::GcStats => {
                self.collect();
                self.record(vec![
                    ("slots", Value::Int(self.stats.live_slots as i64)),
                    ("closures", Value::Int(self.stats.live_closures as i64)),
                    ("slot_capacity", Value::Int(self.slots.capacity() as i64)),
                    (
                        "closure_capacity",
                        Value::Int(self.closures.capacity() as i64),
                    ),
                    ("collections", Value::Int(self.stats.gc_collections as i64)),
                    ("output_bytes", Value::Int(self.output.len() as i64)),
                    ("strings", Value::Int(self.stats.live_strings as i64)),
                    (
                        "string_bytes",
                        Value::Int(self.stats.live_string_bytes as i64),
                    ),
                ])
            }
            Builtin::NowMs => {
                Value::Int(self.started.elapsed().as_millis().min(i64::MAX as u128) as i64)
            }
            Builtin::UnixMs => Value::Int(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| Error::new("ClockError", e.to_string(), span))?
                    .as_millis()
                    .min(i64::MAX as u128) as i64,
            ),
            Builtin::Sleep => {
                let ms = a.int(0)?;
                if !(0..=86400000).contains(&ms) {
                    return Err(Error::new(
                        "ValueError",
                        "sleep requires 0..86400000 milliseconds",
                        span,
                    ));
                }
                self.terminal
                    .sleep(ms as u64)
                    .map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::FormatUtc => {
                let (year, month, day, hour, minute, second, _, _) = unix_parts(a.int(0)?);
                if !(0..=9999).contains(&year) {
                    return Err(Error::new(
                        "ValueError",
                        "UTC year must be in 0..9999",
                        span,
                    ));
                }
                string(format!(
                    "{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"
                ))
            }
            Builtin::TimeParts => {
                let (year, month, day, hour, minute, second, millisecond, weekday) =
                    unix_parts(a.int(0)?);
                self.record(vec![
                    ("year", Value::Int(year)),
                    ("month", Value::Int(i64::from(month))),
                    ("day", Value::Int(i64::from(day))),
                    ("hour", Value::Int(hour)),
                    ("minute", Value::Int(minute)),
                    ("second", Value::Int(second)),
                    ("millisecond", Value::Int(millisecond)),
                    ("weekday", Value::Int(i64::from(weekday))),
                ])
            }
            Builtin::Seed => {
                self.random_state = a.int(0)? as u64;
                Value::Unit
            }
            Builtin::RandomInt => {
                let (lo, hi) = (a.int(0)?, a.int(1)?);
                if lo >= hi {
                    return Err(Error::new(
                        "ValueError",
                        "random.int requires lo < hi",
                        span,
                    ));
                }
                let range = (i128::from(hi) - i128::from(lo)) as u64;
                let threshold = range.wrapping_neg() % range;
                let word = loop {
                    let x = self.random_word();
                    if x >= threshold {
                        break x;
                    }
                };
                Value::Int((i128::from(lo) + i128::from(word % range)) as i64)
            }
            Builtin::RandomFloat => {
                Value::Float((self.random_word() >> 11) as f64 / 9007199254740992.0)
            }
            Builtin::RandomBool => Value::Bool(self.random_word() & 1 != 0),
            Builtin::RandomFloatRange => {
                let (lo, hi) = (a.float(0)?, a.float(1)?);
                if lo >= hi || !lo.is_finite() || !hi.is_finite() {
                    return Err(Error::new(
                        "ValueError",
                        "random.float_range requires finite lo < hi",
                        span,
                    ));
                }
                let fraction = (self.random_word() >> 11) as f64 / 9007199254740992.0;
                finite(lo + (hi - lo) * fraction, span)?
            }
            Builtin::RandomChoice => {
                let id = a.closure(0)?;
                let length = self.closure(id).fields.len();
                if length == 0 {
                    return Err(Error::new(
                        "ValueError",
                        "random.choice requires a non-empty closure",
                        span,
                    ));
                }
                let index = self.random_below(length);
                let slot = self.closure(id).fields[index].1;
                self.slot(slot).value.clone()
            }
            Builtin::RandomShuffle => {
                let id = a.closure(0)?;
                let slots = self.writable_slots(id, span)?;
                for index in (1..slots.len()).rev() {
                    let picked = self.random_below(index + 1);
                    self.swap_slots(slots[index], slots[picked]);
                }
                Value::Unit
            }
            Builtin::RandomBytes => {
                let count = a.index(0)?;
                if count > MAX_ARRAY {
                    return Err(Error::new(
                        "ResourceLimit",
                        "random.bytes length exceeds 1000000",
                        span,
                    ));
                }
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(Value::Int((self.random_word() & 0xFF) as i64));
                }
                self.array_value(values, span)
            }
            Builtin::RandomNormal => {
                let first = loop {
                    let candidate = (self.random_word() >> 11) as f64 / 9007199254740992.0;
                    if candidate > 0.0 {
                        break candidate;
                    }
                };
                let second = (self.random_word() >> 11) as f64 / 9007199254740992.0;
                finite(
                    (-2.0 * first.ln()).sqrt() * (std::f64::consts::TAU * second).cos(),
                    span,
                )?
            }
            Builtin::TextAt => {
                let s = a.text(0)?;
                let i = a.index(1)?;
                string(
                    s.chars()
                        .nth(i)
                        .ok_or_else(|| Error::new("IndexError", "string index out of range", span))?
                        .to_string(),
                )
            }
            Builtin::Slice => {
                let s = a.text(0)?;
                let (start, end) = (a.index(1)?, a.index(2)?);
                let count = s.chars().count();
                if start > end || end > count {
                    return Err(Error::new(
                        "IndexError",
                        "slice requires 0 <= start <= end <= length",
                        span,
                    ));
                }
                string(s.chars().skip(start).take(end - start).collect::<String>())
            }
            Builtin::Repeat => {
                let s = a.text(0)?;
                let n = a.index(1)?;
                if s.len().checked_mul(n).is_none_or(|size| size > MAX_TEXT) {
                    return Err(Error::new(
                        "ResourceLimit",
                        "repeated string exceeds 16 MiB",
                        span,
                    ));
                }
                string(s.repeat(n))
            }
            Builtin::Trim => string(a.text(0)?.trim()),
            Builtin::Contains => Value::Bool(a.text(0)?.contains(a.text(1)?)),
            Builtin::StartsWith => Value::Bool(a.text(0)?.starts_with(a.text(1)?)),
            Builtin::EndsWith => Value::Bool(a.text(0)?.ends_with(a.text(1)?)),
            Builtin::Replace => {
                let (s, from, to) = (a.text(0)?, a.text(1)?, a.text(2)?);
                let count = if from.is_empty() {
                    s.chars().count() + 1
                } else {
                    s.matches(from).count()
                };
                let size = (s.len() as u128) - (count as u128) * (from.len() as u128)
                    + (count as u128) * (to.len() as u128);
                if size > MAX_TEXT as u128 {
                    return Err(Error::new(
                        "ResourceLimit",
                        "replacement output exceeds 16 MiB allocation budget",
                        span,
                    ));
                }
                string(s.replace(from, to))
            }
            Builtin::IndexOf | Builtin::LastIndexOf => {
                let text = a.text(0)?;
                let needle = a.text(1)?;
                let position = if needle.is_empty() {
                    Some(if matches!(b, Builtin::IndexOf) {
                        0
                    } else {
                        text.chars().count()
                    })
                } else if matches!(b, Builtin::IndexOf) {
                    text.find(needle).map(|byte| text[..byte].chars().count())
                } else {
                    text.rfind(needle).map(|byte| text[..byte].chars().count())
                };
                position.map_or(Value::Unit, |index| Value::Int(index as i64))
            }
            Builtin::Split => {
                let text = a.text(0)?;
                let separator = a.text(1)?;
                let mut parts = vec![];
                let mut size = 0usize;
                if separator.is_empty() {
                    for character in text.chars() {
                        size = size.saturating_add(character.len_utf8());
                        parts.push(string(character.to_string()));
                    }
                } else {
                    for part in text.split(separator) {
                        size = size
                            .saturating_add(part.len())
                            .saturating_add(separator.len());
                        parts.push(string(part));
                    }
                }
                if parts.len() > MAX_ARRAY || size > MAX_TEXT {
                    return Err(Error::new(
                        "ResourceLimit",
                        "split result exceeds array or text limits",
                        span,
                    ));
                }
                self.array_value(parts, span)
            }
            Builtin::Lines => {
                let text = a.text(0)?;
                let lines = text.lines().map(string).collect::<Vec<_>>();
                if lines.len() > MAX_ARRAY {
                    return Err(Error::new(
                        "ResourceLimit",
                        "line count exceeds 1000000",
                        span,
                    ));
                }
                self.array_value(lines, span)
            }
            Builtin::Join => {
                let id = a.closure(0)?;
                let separator = a.text(1)?;
                let slots = self
                    .closure(id)
                    .fields
                    .iter()
                    .map(|(_, slot)| *slot)
                    .collect::<Vec<_>>();
                let mut output = String::new();
                for (index, slot) in slots.iter().enumerate() {
                    let Value::Str(text) = &self.slot(*slot).value else {
                        return Err(
                            self.type_error(format!("join part {index} requires str"), span)
                        );
                    };
                    if index > 0 {
                        output.push_str(separator);
                    }
                    output.push_str(text);
                    if output.len() > MAX_TEXT {
                        return Err(Error::new(
                            "ResourceLimit",
                            "joined text exceeds 16 MiB",
                            span,
                        ));
                    }
                }
                string(output)
            }
            Builtin::Upper | Builtin::Lower => {
                let text = if matches!(b, Builtin::Upper) {
                    a.text(0)?.to_uppercase()
                } else {
                    a.text(0)?.to_lowercase()
                };
                if text.len() > MAX_TEXT {
                    return Err(Error::new(
                        "ResourceLimit",
                        "case conversion exceeds 16 MiB",
                        span,
                    ));
                }
                string(text)
            }
            Builtin::PadStart | Builtin::PadEnd => {
                let text = a.text(0)?;
                let width = a.index(1)?;
                let fill = a.text(2)?;
                let mut characters = fill.chars();
                let Some(character) = characters.next() else {
                    return Err(Error::new(
                        "ValueError",
                        "padding fill requires exactly one Unicode scalar",
                        span,
                    ));
                };
                if characters.next().is_some() {
                    return Err(Error::new(
                        "ValueError",
                        "padding fill requires exactly one Unicode scalar",
                        span,
                    ));
                }
                if width > MAX_TEXT {
                    return Err(Error::new(
                        "ResourceLimit",
                        "padding width exceeds 16 MiB",
                        span,
                    ));
                }
                let length = text.chars().count();
                if length >= width {
                    string(text)
                } else {
                    let padding_length = width - length;
                    if (padding_length as u64) * (character.len_utf8() as u64) > MAX_TEXT as u64 {
                        return Err(Error::new(
                            "ResourceLimit",
                            "padded text exceeds 16 MiB",
                            span,
                        ));
                    }
                    let padding = character.to_string().repeat(padding_length);
                    let output = if matches!(b, Builtin::PadStart) {
                        format!("{padding}{text}")
                    } else {
                        format!("{text}{padding}")
                    };
                    string(output)
                }
            }
            Builtin::CodeAt => {
                let text = a.text(0)?;
                let index = a.index(1)?;
                let character = text
                    .chars()
                    .nth(index)
                    .ok_or_else(|| Error::new("IndexError", "string index out of range", span))?;
                Value::Int(i64::from(character as u32))
            }
            Builtin::FromCode => {
                let code = a.int(0)?;
                let character = u32::try_from(code)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| {
                        Error::new(
                            "ValueError",
                            "from_code requires a Unicode scalar value",
                            span,
                        )
                    })?;
                string(character.to_string())
            }
            Builtin::Fixed => {
                let value = a.float(0)?;
                let digits = a.index(1)?;
                if digits > 17 {
                    return Err(Error::new(
                        "ValueError",
                        "fixed requires 0..17 decimal digits",
                        span,
                    ));
                }
                string(format!("{value:.digits$}"))
            }
            Builtin::ParseIntRadix => {
                let text = a.text(0)?.trim();
                let base = a.int(1)?;
                if !(2..=36).contains(&base) {
                    return Err(Error::new(
                        "ValueError",
                        "parse_int_radix requires base 2..36",
                        span,
                    ));
                }
                let digits = text.strip_prefix('+').unwrap_or(text);
                i64::from_str_radix(digits, base as u32)
                    .ok()
                    .map_or(Value::Unit, Value::Int)
            }
            Builtin::Abs => match values[0] {
                Value::Int(n) => Value::Int(n.checked_abs().ok_or_else(|| overflow(span))?),
                Value::Float(n) => Value::Float(n.abs()),
                _ => return Err(self.type_error("abs requires a number", span)),
            },
            Builtin::Sign => match values[0] {
                Value::Int(n) => Value::Int(n.signum()),
                Value::Float(n) => Value::Int(if n == 0.0 {
                    0
                } else if n < 0.0 {
                    -1
                } else {
                    1
                }),
                _ => return Err(self.type_error("sign requires a number", span)),
            },
            Builtin::Min | Builtin::Max => {
                let order = compare(&values[0], &values[1], span)?;
                let first = if matches!(b, Builtin::Min) {
                    order.is_le()
                } else {
                    order.is_ge()
                };
                values[if first { 0 } else { 1 }].clone()
            }
            Builtin::Clamp => {
                if compare(&values[1], &values[2], span)?.is_gt() {
                    return Err(Error::new("ValueError", "clamp requires lo <= hi", span));
                }
                let low = compare(&values[0], &values[1], span)?;
                let high = compare(&values[0], &values[2], span)?;
                values[if low.is_lt() {
                    1
                } else if high.is_gt() {
                    2
                } else {
                    0
                }]
                .clone()
            }
            Builtin::Floor => finite(a.float(0)?.floor(), span)?,
            Builtin::Ceil => finite(a.float(0)?.ceil(), span)?,
            Builtin::Round => finite(a.float(0)?.round(), span)?,
            Builtin::Sqrt => finite(a.float(0)?.sqrt(), span)?,
            Builtin::Pow => finite(a.float(0)?.powf(a.float(1)?), span)?,
            Builtin::Sin => finite(a.float(0)?.sin(), span)?,
            Builtin::Cos => finite(a.float(0)?.cos(), span)?,
            Builtin::Tan => finite(a.float(0)?.tan(), span)?,
            Builtin::Asin => finite(a.float(0)?.asin(), span)?,
            Builtin::Acos => finite(a.float(0)?.acos(), span)?,
            Builtin::Atan => finite(a.float(0)?.atan(), span)?,
            Builtin::Atan2 => finite(a.float(0)?.atan2(a.float(1)?), span)?,
            Builtin::Exp => finite(a.float(0)?.exp(), span)?,
            Builtin::Ln => finite(a.float(0)?.ln(), span)?,
            Builtin::Log => finite(a.float(0)?.log(a.float(1)?), span)?,
            Builtin::Log2 => finite(a.float(0)?.log2(), span)?,
            Builtin::Log10 => finite(a.float(0)?.log10(), span)?,
            Builtin::Trunc => finite(a.float(0)?.trunc(), span)?,
            Builtin::Hypot => finite(a.float(0)?.hypot(a.float(1)?), span)?,
            Builtin::Pi => Value::Float(std::f64::consts::PI),
            Builtin::BitAnd => Value::Int(a.int(0)? & a.int(1)?),
            Builtin::BitOr => Value::Int(a.int(0)? | a.int(1)?),
            Builtin::BitXor => Value::Int(a.int(0)? ^ a.int(1)?),
            Builtin::BitNot => Value::Int(!a.int(0)?),
            Builtin::Shl | Builtin::Shr => {
                let value = a.int(0)?;
                let bits = a.int(1)?;
                if !(0..=63).contains(&bits) {
                    return Err(Error::new("ValueError", "shift requires 0..63 bits", span));
                }
                Value::Int(if matches!(b, Builtin::Shl) {
                    value.wrapping_shl(bits as u32)
                } else {
                    value >> bits
                })
            }
            Builtin::CountOnes => Value::Int(i64::from(a.int(0)?.count_ones())),
            Builtin::LeadingZeros => Value::Int(i64::from(a.int(0)?.leading_zeros())),
            Builtin::TrailingZeros => Value::Int(i64::from(a.int(0)?.trailing_zeros())),
            Builtin::SeqSlice => {
                let id = a.closure(0)?;
                let (start, end) = (a.index(1)?, a.index(2)?);
                let items = self.closure_values(id);
                if start > end || end > items.len() {
                    return Err(Error::new(
                        "IndexError",
                        "sequence slice requires 0 <= start <= end <= length",
                        span,
                    ));
                }
                self.array_value(items[start..end].to_vec(), span)
            }
            Builtin::SeqReverse => {
                let id = a.closure(0)?;
                let slots = self.writable_slots(id, span)?;
                let length = slots.len();
                for index in 0..length / 2 {
                    self.swap_slots(slots[index], slots[length - 1 - index]);
                }
                Value::Unit
            }
            Builtin::SeqSort => {
                let id = a.closure(0)?;
                let slots = self.writable_slots(id, span)?;
                let mut items = self.closure_values(id);
                if let Some(first) = items.first() {
                    match first {
                        Value::Int(_) => {
                            if !items.iter().all(|item| matches!(item, Value::Int(_))) {
                                return Err(self.type_error(
                                    "sort requires homogeneous int, float, or str fields",
                                    span,
                                ));
                            }
                            items.sort_by(|left, right| match (left, right) {
                                (Value::Int(left), Value::Int(right)) => left.cmp(right),
                                _ => unreachable!(),
                            });
                        }
                        Value::Float(_) => {
                            if !items.iter().all(|item| matches!(item, Value::Float(_))) {
                                return Err(self.type_error(
                                    "sort requires homogeneous int, float, or str fields",
                                    span,
                                ));
                            }
                            items.sort_by(|left, right| match (left, right) {
                                (Value::Float(left), Value::Float(right)) => {
                                    left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
                                }
                                _ => unreachable!(),
                            });
                        }
                        Value::Str(_) => {
                            if !items.iter().all(|item| matches!(item, Value::Str(_))) {
                                return Err(self.type_error(
                                    "sort requires homogeneous int, float, or str fields",
                                    span,
                                ));
                            }
                            items.sort_by(|left, right| match (left, right) {
                                (Value::Str(left), Value::Str(right)) => left.cmp(right),
                                _ => unreachable!(),
                            });
                        }
                        _ => {
                            return Err(self.type_error(
                                "sort requires homogeneous int, float, or str fields",
                                span,
                            ));
                        }
                    }
                }
                for (slot, item) in slots.into_iter().zip(items) {
                    self.slots[slot].as_mut().unwrap().value = item;
                }
                Value::Unit
            }
            Builtin::SeqFill => {
                let id = a.closure(0)?;
                let slots = self.writable_slots(id, span)?;
                for slot in slots {
                    self.slots[slot].as_mut().unwrap().value = values[1].clone();
                }
                Value::Unit
            }
            Builtin::SeqIndexOf => {
                let id = a.closure(0)?;
                let needle = &values[1];
                self.closure_values(id)
                    .iter()
                    .position(|item| item.equals(needle))
                    .map_or(Value::Unit, |index| Value::Int(index as i64))
            }
            Builtin::SeqCopy => {
                let id = a.closure(0)?;
                self.array_value(self.closure_values(id), span)
            }
            Builtin::SeqConcat => {
                let left = a.closure(0)?;
                let right = a.closure(1)?;
                let mut items = self.closure_values(left);
                items.extend(self.closure_values(right));
                if items.len() > MAX_ARRAY {
                    return Err(Error::new(
                        "ResourceLimit",
                        "concatenated sequence exceeds 1000000 fields",
                        span,
                    ));
                }
                self.array_value(items, span)
            }
            Builtin::Argc => Value::Int(self.config.args.len() as i64),
            Builtin::Arg => self
                .config
                .args
                .get(a.index(0)?)
                .map_or(Value::Unit, |v| string(v.clone())),
            Builtin::EnvGet => {
                let name = a.text(0)?;
                if name.is_empty() || name.contains(['=', '\0']) {
                    return Err(Error::new(
                        "ValueError",
                        "invalid environment variable name",
                        span,
                    ));
                }
                match std::env::var(name) {
                    Ok(value) => string(value),
                    Err(std::env::VarError::NotPresent) => Value::Unit,
                    Err(e) => return Err(Error::new("IoError", e.to_string(), span)),
                }
            }
            Builtin::Cwd => string(
                std::env::current_dir()
                    .map_err(|e| io_error(e, span))?
                    .to_string_lossy()
                    .into_owned(),
            ),
            Builtin::EnvArgs => {
                let args = self
                    .config
                    .args
                    .iter()
                    .cloned()
                    .map(string)
                    .collect::<Vec<_>>();
                self.array_value(args, span)
            }
            Builtin::EnvHome => std::env::var("HOME").ok().map_or(Value::Unit, string),
            Builtin::EnvTempDir => string(std::env::temp_dir().to_string_lossy().into_owned()),
            Builtin::ProcessPid => Value::Int(i64::from(std::process::id())),
            Builtin::ProcessExit => {
                let code = a.int(0)?;
                if !(0..=255).contains(&code) {
                    return Err(Error::new(
                        "ValueError",
                        "process.exit requires code 0..255",
                        span,
                    ));
                }
                self.flush_output(span)?;
                let _ = std::io::stderr().flush();
                let _ = self.terminal.leave();
                std::process::exit(code as i32);
            }
            Builtin::ReadText => {
                let file = std::fs::File::open(a.text(0)?).map_err(|e| io_error(e, span))?;
                let mut bytes = vec![];
                file.take(MAX_TEXT as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| io_error(e, span))?;
                if bytes.len() > MAX_TEXT {
                    return Err(Error::new(
                        "ResourceLimit",
                        "text file exceeds 16 MiB",
                        span,
                    ));
                }
                string(
                    String::from_utf8(bytes)
                        .map_err(|_| Error::new("IoError", "file is not valid UTF-8", span))?,
                )
            }
            Builtin::WriteText | Builtin::AppendText => {
                let (path, text) = (a.text(0)?, a.text(1)?);
                if text.len() > MAX_TEXT {
                    return Err(Error::new(
                        "ResourceLimit",
                        "text write exceeds 16 MiB",
                        span,
                    ));
                }
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(matches!(b, Builtin::WriteText))
                    .append(matches!(b, Builtin::AppendText))
                    .open(path)
                    .map_err(|e| io_error(e, span))?;
                file.write_all(text.as_bytes())
                    .map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::Exists => Value::Bool(
                Path::new(a.text(0)?)
                    .try_exists()
                    .map_err(|e| io_error(e, span))?,
            ),
            Builtin::ListDir => {
                let path = a.text(0)?;
                let mut names = vec![];
                for entry in std::fs::read_dir(path).map_err(|e| io_error(e, span))? {
                    let entry = entry.map_err(|e| io_error(e, span))?;
                    let name = entry.file_name().into_string().map_err(|_| {
                        Error::new("IoError", "directory entry name is not valid UTF-8", span)
                    })?;
                    names.push(name);
                    if names.len() > MAX_ARRAY {
                        return Err(Error::new(
                            "ResourceLimit",
                            "directory contains more than 1000000 entries",
                            span,
                        ));
                    }
                }
                names.sort();
                let values = names.into_iter().map(string).collect::<Vec<_>>();
                self.array_value(values, span)
            }
            Builtin::MkdirAll => {
                std::fs::create_dir_all(a.text(0)?).map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::RemoveFile => {
                std::fs::remove_file(a.text(0)?).map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::RemoveDir => {
                std::fs::remove_dir(a.text(0)?).map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::Rename => {
                std::fs::rename(a.text(0)?, a.text(1)?).map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::CopyFile => {
                std::fs::copy(a.text(0)?, a.text(1)?).map_err(|e| io_error(e, span))?;
                Value::Unit
            }
            Builtin::FileSize => {
                let size = std::fs::metadata(a.text(0)?)
                    .map_err(|e| io_error(e, span))?
                    .len();
                Value::Int(i64::try_from(size).map_err(|_| {
                    Error::new(
                        "ResourceLimit",
                        "file size exceeds signed 64-bit range",
                        span,
                    )
                })?)
            }
            Builtin::IsFile => Value::Bool(Path::new(a.text(0)?).is_file()),
            Builtin::IsDir => Value::Bool(Path::new(a.text(0)?).is_dir()),
            Builtin::TermEnter => {
                if !self.config.echo_output {
                    return Err(Error::new(
                        "TerminalError",
                        "terminal mode requires echo_output",
                        span,
                    ));
                }
                self.terminal
                    .enter()
                    .map_err(|e| Error::new("TerminalError", e.to_string(), span))?;
                Value::Unit
            }
            Builtin::TermLeave => {
                self.terminal
                    .leave()
                    .map_err(|e| Error::new("TerminalError", e.to_string(), span))?;
                Value::Unit
            }
            Builtin::IsTty => Value::Bool(Terminal::is_tty()),
            Builtin::TermSize => {
                let (w, h) = Terminal::size();
                self.record(vec![("width", Value::Int(w)), ("height", Value::Int(h))])
            }
            Builtin::Clear => return self.terminal_control("\x1b[2J\x1b[H", span),
            Builtin::Home => return self.terminal_control("\x1b[H", span),
            Builtin::Reset => return self.terminal_control("\x1b[0m", span),
            Builtin::Move => {
                let (x, y) = (a.int(0)?, a.int(1)?);
                if !(0..=10000).contains(&x) || !(0..=10000).contains(&y) {
                    return Err(Error::new(
                        "ValueError",
                        "cursor coordinates must be in 0..10000",
                        span,
                    ));
                }
                return self.terminal_control(&format!("\x1b[{};{}H", y + 1, x + 1), span);
            }
            Builtin::Cursor => {
                return self.terminal_control(
                    if a.boolean(0)? {
                        "\x1b[?25h"
                    } else {
                        "\x1b[?25l"
                    },
                    span,
                );
            }
            Builtin::Color => {
                let (fg, bg) = (a.int(0)?, a.int(1)?);
                if !(-1..=15).contains(&fg) || !(-1..=15).contains(&bg) {
                    return Err(Error::new(
                        "ValueError",
                        "colors must be -1 (default) or 0..15",
                        span,
                    ));
                }
                let code = |n: i64, base: i64| {
                    if n < 0 {
                        base + 9
                    } else if n < 8 {
                        base + n
                    } else {
                        base + 60 + n - 8
                    }
                };
                return self
                    .terminal_control(&format!("\x1b[{};{}m", code(fg, 30), code(bg, 40)), span);
            }
            Builtin::ColorRgb => {
                let (foreground, background) = (a.int(0)?, a.int(1)?);
                if !(-1..=0xFF_FFFF).contains(&foreground)
                    || !(-1..=0xFF_FFFF).contains(&background)
                {
                    return Err(Error::new(
                        "ValueError",
                        "RGB colors must be -1 (default) or 0..16777215",
                        span,
                    ));
                }
                let foreground = if foreground < 0 {
                    "39".to_string()
                } else {
                    format!(
                        "38;2;{};{};{}",
                        (foreground >> 16) & 0xFF,
                        (foreground >> 8) & 0xFF,
                        foreground & 0xFF
                    )
                };
                let background = if background < 0 {
                    "49".to_string()
                } else {
                    format!(
                        "48;2;{};{};{}",
                        (background >> 16) & 0xFF,
                        (background >> 8) & 0xFF,
                        background & 0xFF
                    )
                };
                return self.terminal_control(&format!("\x1b[{foreground};{background}m"), span);
            }
            Builtin::Bell => return self.terminal_control("\x07", span),
            Builtin::Title => {
                let title = a.text(0)?;
                if title.chars().any(char::is_control) {
                    return Err(Error::new(
                        "ValueError",
                        "terminal title must not contain control characters",
                        span,
                    ));
                }
                if title.len() > 1024 {
                    return Err(Error::new(
                        "ResourceLimit",
                        "terminal title exceeds 1024 bytes",
                        span,
                    ));
                }
                return self.terminal_control(&format!("\x1b]0;{title}\x07"), span);
            }
            Builtin::SysOs => string(std::env::consts::OS),
            Builtin::SysArch => string(std::env::consts::ARCH),
            Builtin::PollKey => {
                let timeout = a.int(0)?;
                if !(-1..=60000).contains(&timeout) {
                    return Err(Error::new(
                        "ValueError",
                        "key timeout must be -1 or 0..60000 milliseconds",
                        span,
                    ));
                }
                self.terminal
                    .poll_key(timeout)
                    .map_err(|e| io_error(e, span))?
                    .map_or(Value::Unit, string)
            }
        };
        Ok(result)
    }
}
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    } as u32;
    (year + i64::from(month <= 2), month, day)
}

fn unix_parts(milliseconds: i64) -> (i64, u32, u32, i64, i64, i64, i64, u32) {
    let days = milliseconds.div_euclid(86_400_000);
    let in_day = milliseconds.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    (
        year,
        month,
        day,
        in_day / 3_600_000,
        in_day / 60_000 % 60,
        in_day / 1_000 % 60,
        in_day % 1_000,
        (days + 4).rem_euclid(7) as u32,
    )
}

fn compare(a: &Value, b: &Value, span: Span) -> Result<std::cmp::Ordering> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => Ok(a.partial_cmp(b).unwrap()),
        _ => Err(Error::new(
            "TypeError",
            "numeric arguments must have the same kind",
            span,
        )),
    }
}
