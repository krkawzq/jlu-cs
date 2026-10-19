use super::{Closure, Code, Runtime, Value, builtins::Builtin};
use crate::{
    jit::CompiledCode,
    syntax::{self, Error, Result, Span},
};
use std::rc::Rc;
fn text(value: impl Into<String>) -> Value {
    Value::Str(Rc::new(value.into()))
}
fn integer(value: usize) -> Value {
    Value::Int(value.min(i64::MAX as usize) as i64)
}
fn input_text(value: &Value, span: Span) -> Result<&str> {
    if let Value::Str(s) = value {
        Ok(s)
    } else {
        Err(Error::new(
            "TypeError",
            "inspection source requires str",
            span,
        ))
    }
}

impl Runtime {
    pub(super) fn snapshot(&mut self, fields: Vec<(&str, Value)>) -> Value {
        let fields = fields
            .into_iter()
            .map(|(n, v)| (n.to_string(), self.new_slot(v, false)))
            .collect();
        Value::Closure(self.new_closure(Closure {
            fields,
            code: Code::Data,
            jit: false,
            marked: Span::default(),
        }))
    }
    pub(super) fn snapshot_array(&mut self, values: Vec<Value>) -> Value {
        let fields = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), self.new_slot(v, false)))
            .collect();
        Value::Closure(self.new_closure(Closure {
            fields,
            code: Code::Data,
            jit: false,
            marked: Span::default(),
        }))
    }
    fn object_id(&self, value: &Value, span: Span) -> Result<usize> {
        if let Value::Closure(id) = value {
            Ok(*id)
        } else {
            Err(self.type_error("expected a closure", span))
        }
    }
    pub(super) fn snapshot_field(&self, value: &Value, name: &str, span: Span) -> Result<Value> {
        let id = self.object_id(value, span)?;
        self.closure(id)
            .fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, s)| self.slot(*s).value.clone())
            .ok_or_else(|| Error::new("AstError", format!("missing node field '{name}'"), span))
    }
    pub(super) fn snapshot_items(&self, value: &Value, span: Span) -> Result<Vec<Value>> {
        let id = self.object_id(value, span)?;
        self.closure(id)
            .fields
            .iter()
            .enumerate()
            .map(|(i, (name, s))| {
                if name == &i.to_string() {
                    Ok(self.slot(*s).value.clone())
                } else {
                    Err(Error::new("AstError", "expected an indexed sequence", span))
                }
            })
            .collect()
    }
    fn user_function(&self, value: &Value, span: Span) -> Result<(usize, usize)> {
        let id = self.object_id(value, span)?;
        if let Code::User(code) = self.closure(id).code {
            Ok((id, code))
        } else {
            Err(Error::new(
                "InspectError",
                "this operation requires a Prim user function",
                span,
            ))
        }
    }
    fn source_for(&self, code: usize) -> Option<String> {
        let span = self.program.functions[code].definition_span;
        self.program
            .source_at(span.start)?
            .segment(span)
            .map(str::to_owned)
    }
    fn location_snapshot(&mut self, span: Span) -> Value {
        let Some(source) = self.program.source_at(span.start) else {
            return Value::Unit;
        };
        let local = source.local_span(span);
        let (line, column) = syntax::location(&source.text, local.start);
        let (end_line, end_column) = syntax::location(&source.text, local.end);
        let name = source.name.clone();
        self.snapshot(vec![
            ("file", text(name)),
            ("start", integer(local.start)),
            ("end", integer(local.end)),
            ("line", integer(line)),
            ("column", integer(column)),
            ("end_line", integer(end_line)),
            ("end_column", integer(end_column)),
        ])
    }
    fn artifact(&self, code: usize, span: Span) -> Result<&CompiledCode> {
        match self.jit.artifact(code) {
            Some(Ok(artifact)) => Ok(artifact),
            Some(Err(error)) => Err(Error::new(
                "InspectError",
                format!("compilation failed: {error}"),
                span,
            )),
            None => Err(Error::new(
                "JitNotCompiled",
                "call the @jit closure or std.inspect.compile first",
                span,
            )),
        }
    }
    fn jit_snapshot(&mut self, id: usize, code: usize) -> Value {
        let policy = if self.closure(id).jit {
            "jit"
        } else {
            "interpreted"
        };
        let state = match self.jit.artifact(code) {
            Some(Ok(_)) => "ready",
            Some(Err(_)) => "failed",
            None => "uncompiled",
        };
        let (mut address, mut size, mut triple, mut compile_ms, mut error, mut metadata_error) = (
            Value::Unit,
            Value::Unit,
            Value::Unit,
            Value::Unit,
            Value::Unit,
            Value::Unit,
        );
        let (mut data_layout, mut ir_bytes) = (Value::Unit, Value::Unit);
        match self.jit.artifact(code) {
            Some(Ok(a)) => {
                address = text(format!("0x{:x}", a.address));
                triple = text(a.triple.clone());
                compile_ms = Value::Float(a.compile_ms);
                data_layout = text(a.data_layout.clone());
                ir_bytes = integer(a.ir.len());
                match &a.machine {
                    Ok(m) => size = integer(m.bytes.len()),
                    Err(e) => metadata_error = text(e.clone()),
                }
            }
            Some(Err(e)) => error = text(e.clone()),
            None => {}
        }
        let (opt_level, optimized_ir_bytes, lowering) = match self.jit.artifact(code) {
            Some(Ok(a)) => (a.opt_level, integer(a.optimized_ir.len()), text(if a.scalar_lowering { "local-scalars" } else { "generic" })),
            _ => (self.config.opt_level, Value::Unit, Value::Unit),
        };
        let stats = self.function_stats.get(&code).copied().unwrap_or_default();
        self.snapshot(vec![
            ("code_id", integer(code)),
            (
                "arity",
                integer(self.program.functions[code].parameters.len()),
            ),
            (
                "capture_count",
                integer(self.program.functions[code].captures.len()),
            ),
            ("registers", integer(self.program.functions[code].registers)),
            (
                "instruction_count",
                integer(self.program.functions[code].code.len()),
            ),
            ("policy", text(policy)),
            ("enabled", Value::Bool(self.config.jit_enabled)),
            ("state", text(state)),
            ("compiled", Value::Bool(state == "ready")),
            (
                "will_use_native",
                Value::Bool(policy == "jit" && self.config.jit_enabled && state == "ready"),
            ),
            ("symbol", text(format!("prim_function_{code}"))),
            ("address", address),
            ("code_size", size),
            ("triple", triple),
            ("data_layout", data_layout),
            ("ir_bytes", ir_bytes),
            ("optimized_ir_bytes", optimized_ir_bytes),
            ("opt_level", integer(opt_level as usize)),
            ("lowering", lowering),
            ("scalar_calls", integer(stats.scalar_calls as usize)),
            ("scalar_fallbacks", integer(stats.scalar_fallbacks as usize)),
            ("llvm_version", text(env!("PRIM_LLVM_VERSION"))),
            ("compile_ms", compile_ms),
            ("native_calls", integer(stats.native_calls as usize)),
            (
                "interpreted_calls",
                integer(stats.interpreted_calls as usize),
            ),
            (
                "native_integer_ops",
                integer(stats.native_integer_ops as usize),
            ),
            ("error", error),
            ("metadata_error", metadata_error),
        ])
    }
    pub(super) fn inspect_builtin(
        &mut self,
        b: Builtin,
        values: Vec<Value>,
        span: Span,
    ) -> Result<Value> {
        let first = values.first().cloned().unwrap_or(Value::Unit);
        match b {
            Builtin::AstParse => {
                let source = input_text(&first, span)?;
                self.ast_parse(source, span)
            }
            Builtin::AstTokens => {
                let source = input_text(&first, span)?;
                self.ast_tokens(source, span)
            }
            Builtin::AstWalk => {
                let nodes = self.ast_nodes(&first, span)?;
                Ok(self.snapshot_array(nodes))
            }
            Builtin::AstFind => {
                let kind = input_text(&values[1], span)?;
                let nodes = self.ast_nodes(&first, span)?;
                let filtered=nodes.into_iter().filter(|n|matches!(self.snapshot_field(n,"kind",span),Ok(Value::Str(k))if k.as_str()==kind)).collect();
                Ok(self.snapshot_array(filtered))
            }
            Builtin::AstDump => self.ast_dump(&first, span).map(text),
            Builtin::AstIsNode => Ok(Value::Bool(self.ast_node_shape(&first, span))),
            Builtin::AstSegment => {
                let source = input_text(&first, span)?;
                let loc = self.snapshot_field(&values[1], "span", span)?;
                let start = self.snapshot_field(&loc, "start", span)?;
                let end = self.snapshot_field(&loc, "end", span)?;
                let (Value::Int(start), Value::Int(end)) = (start, end) else {
                    return Err(Error::new("AstError", "invalid source span", span));
                };
                let segment = usize::try_from(start)
                    .ok()
                    .zip(usize::try_from(end).ok())
                    .and_then(|(a, b)| source.get(a..b))
                    .ok_or_else(|| {
                        Error::new(
                            "AstError",
                            "span is outside this source or not a UTF-8 boundary",
                            span,
                        )
                    })?;
                Ok(text(segment))
            }
            Builtin::InspectIsCallable => Ok(Value::Bool(self.callable(&first, span).is_ok())),
            Builtin::InspectIsFunction => Ok(Value::Bool(
                matches!(first,Value::Closure(id)if matches!(self.closure(id).code,Code::User(_))),
            )),
            Builtin::InspectIsBuiltin => Ok(Value::Bool(
                matches!(first,Value::Closure(id)if matches!(self.closure(id).code,Code::Builtin(_))),
            )),
            Builtin::InspectIsJit => Ok(Value::Bool(
                matches!(first,Value::Closure(id)if self.closure(id).jit),
            )),
            Builtin::InspectDescribe => {
                let (
                    mut identity,
                    mut code_id,
                    mut arity,
                    mut count,
                    mut name,
                    mut kind,
                    mut policy,
                ) = (
                    Value::Unit,
                    Value::Unit,
                    Value::Unit,
                    0,
                    Value::Unit,
                    first.kind(),
                    "none",
                );
                if let Value::Closure(id) = first {
                    identity = integer(id);
                    let c = self.closure(id);
                    count = c.fields.len();
                    match c.code {
                        Code::Data => kind = "object",
                        Code::Builtin(b) => {
                            kind = "builtin";
                            name = text(b.name());
                            arity = integer(b.arity());
                            policy = "host";
                        }
                        Code::User(code) => {
                            kind = "function";
                            name = text(format!("<function #{code}>"));
                            arity = integer(self.program.functions[code].parameters.len());
                            code_id = integer(code);
                            policy = if c.jit { "jit" } else { "interpreted" };
                        }
                    }
                }
                Ok(self.snapshot(vec![
                    ("type", text(first.kind())),
                    ("kind", text(kind)),
                    ("display", text(first.display())),
                    ("name", name),
                    ("closure_id", identity),
                    ("code_id", code_id),
                    ("arity", arity),
                    ("field_count", integer(count)),
                    ("callable", Value::Bool(self.callable(&first, span).is_ok())),
                    ("policy", text(policy)),
                ]))
            }
            Builtin::InspectMembers => {
                let id = self.object_id(&first, span)?;
                let fields = self.closure(id).fields.clone();
                let mut result = vec![];
                for (name, slot) in fields {
                    let value = self.slot(slot).value.clone();
                    let writable = self.slot(slot).writable;
                    result.push(self.snapshot(vec![
                        ("name", text(name)),
                        ("slot_id", integer(slot)),
                        ("type", text(value.kind())),
                        ("value", value),
                        ("writable", Value::Bool(writable)),
                    ]));
                }
                Ok(self.snapshot_array(result))
            }
            Builtin::InspectSignature => {
                let id = self.callable(&first, span)?;
                let code = self.closure(id).code;
                let mut params = vec![];
                let mut labels = vec![];
                let name = match code {
                    Code::Builtin(b) => {
                        for i in 0..b.arity() {
                            let name = format!("arg{i}");
                            labels.push(name.clone());
                            params.push(self.snapshot(vec![
                                ("name", text(name)),
                                ("index", integer(i)),
                                ("by_ref", Value::Bool(false)),
                            ]));
                        }
                        b.name().to_string()
                    }
                    Code::User(id) => {
                        let p = self.program.functions[id].parameters.clone();
                        for (i, (p, _)) in p.into_iter().enumerate() {
                            labels.push(format!("{}{}", if p.by_ref { "&" } else { "" }, p.name));
                            params.push(self.snapshot(vec![
                                ("name", text(p.name)),
                                ("index", integer(i)),
                                ("by_ref", Value::Bool(p.by_ref)),
                            ]));
                        }
                        format!("<function #{id}>")
                    }
                    Code::Data => unreachable!(),
                };
                let arity = params.len();
                let parameters = self.snapshot_array(params);
                Ok(self.snapshot(vec![
                    ("name", text(name.clone())),
                    ("arity", integer(arity)),
                    ("parameters", parameters),
                    ("text", text(format!("{name}({})", labels.join(", ")))),
                ]))
            }
            Builtin::InspectCaptures => {
                let (id, code) = self.user_function(&first, span)?;
                let fields = self.closure(id).fields.clone();
                let modes = self.program.functions[code].capture_modes.clone();
                let mut result = vec![];
                for (i, (name, slot)) in fields.into_iter().enumerate() {
                    let value = self.slot(slot).value.clone();
                    let writable = self.slot(slot).writable;
                    result.push(self.snapshot(vec![
                        ("name", text(name)),
                        ("mode", text(if modes[i] { "slot" } else { "value" })),
                        ("slot_id", integer(slot)),
                        ("value", value),
                        ("writable", Value::Bool(writable)),
                    ]));
                }
                Ok(self.snapshot_array(result))
            }
            Builtin::InspectSource | Builtin::InspectLocation => {
                let id = self.object_id(&first, span)?;
                let Code::User(code) = self.closure(id).code else {
                    return Ok(Value::Unit);
                };
                if matches!(b, Builtin::InspectSource) {
                    Ok(self.source_for(code).map_or(Value::Unit, text))
                } else {
                    Ok(self.location_snapshot(self.program.functions[code].definition_span))
                }
            }
            Builtin::InspectAst => {
                let (_, code) = self.user_function(&first, span)?;
                let source = self.source_for(code).ok_or_else(|| {
                    Error::new(
                        "InspectError",
                        "source was not retained for this function",
                        span,
                    )
                })?;
                self.ast_parse(&source, span)
            }
            Builtin::InspectCoreIr => {
                let (_, code) = self.user_function(&first, span)?;
                let function = &self.program.functions[code];
                let mut result = format!("function #{code}, {} registers\n", function.registers);
                for (i, op) in function.code.iter().enumerate() {
                    result.push_str(&format!(
                        "{i:04}: {:?}  ; bytes {}..{}\n",
                        op.instruction, op.span.start, op.span.end
                    ));
                }
                Ok(text(result))
            }
            Builtin::InspectStack => {
                let frames = self
                    .frames
                    .iter()
                    .map(|f| (f.function, f.current, f.pc, f.native))
                    .collect::<Vec<_>>();
                let mut result = vec![];
                for (depth, (code, closure, pc, native)) in frames.into_iter().enumerate() {
                    let loc = self.program.functions[code]
                        .code
                        .get(pc)
                        .map_or(self.program.functions[code].span, |op| op.span);
                    let location = self.location_snapshot(loc);
                    result.push(self.snapshot(vec![
                        ("depth", integer(depth)),
                        ("code_id", integer(code)),
                        ("closure_id", closure.map_or(Value::Unit, integer)),
                        ("pc", integer(pc)),
                        ("mode", text(if native { "native" } else { "interpreted" })),
                        ("location", location),
                    ]));
                }
                Ok(self.snapshot_array(result))
            }
            Builtin::InspectBuiltins => {
                let mut list = vec![];
                for (name, arity) in Self::builtin_signatures() {
                    list.push(self.snapshot(vec![("name", text(name)), ("arity", integer(arity))]));
                }
                Ok(self.snapshot_array(list))
            }
            Builtin::InspectJit => {
                let (id, code) = self.user_function(&first, span)?;
                Ok(self.jit_snapshot(id, code))
            }
            Builtin::InspectCompile => {
                let (id, code) = self.user_function(&first, span)?;
                if !self.closure(id).jit {
                    return Err(Error::new(
                        "JitTargetError",
                        "mark the function with @jit or jit(f) before explicit compilation",
                        span,
                    ));
                }
                if !self.config.jit_enabled {
                    return Err(Error::new(
                        "JitDisabled",
                        "JIT is disabled by runtime configuration",
                        span,
                    ));
                }
                self.ensure_native(code, span)?;
                Ok(first)
            }
            Builtin::InspectOptimizedIr => {
                let (_, code) = self.user_function(&first, span)?;
                Ok(text(self.artifact(code, span)?.optimized_ir.clone()))
            }
            Builtin::InspectLlvmIr => {
                let (_, code) = self.user_function(&first, span)?;
                Ok(text(self.artifact(code, span)?.ir.clone()))
            }
            Builtin::InspectMachineCode | Builtin::InspectDisassemble | Builtin::InspectNative => {
                let (_, code) = self.user_function(&first, span)?;
                let artifact = self.artifact(code, span)?;
                let machine = artifact
                    .machine
                    .as_ref()
                    .map_err(|e| Error::new("NativeInspectionError", e.clone(), span))?;
                if machine.bytes.len() > 16 * 1024 * 1024 / 3 {
                    return Err(Error::new(
                        "ResourceLimit",
                        "native inspection output exceeds limit",
                        span,
                    ));
                }
                if matches!(b, Builtin::InspectMachineCode) {
                    return Ok(text(machine.hex()));
                }
                let assembly = machine
                    .disassemble(&artifact.triple)
                    .map_err(|e| Error::new("NativeInspectionError", e, span))?;
                if matches!(b, Builtin::InspectDisassemble) {
                    return Ok(text(assembly));
                }
                let size = machine.bytes.len();
                let address = format!("0x{:x}", machine.address);
                let hex = machine.hex();
                let triple = artifact.triple.clone();
                let symbol = artifact.symbol.clone();
                Ok(self.snapshot(vec![
                    ("address", text(address)),
                    ("size", integer(size)),
                    ("hex", text(hex)),
                    ("assembly", text(assembly)),
                    ("triple", text(triple)),
                    ("symbol", text(symbol)),
                    ("origin", text("loaded-memory")),
                ]))
            }
            _ => unreachable!(),
        }
    }
}
