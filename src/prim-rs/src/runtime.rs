use crate::terminal::Terminal;
use crate::{
    ir::{Constant, Instruction, Program},
    jit::Jit,
    syntax::{Error, Result, Span},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
#[path = "ast_api.rs"]
mod ast_api;
#[path = "builtins.rs"]
mod builtins;
#[path = "inspect.rs"]
mod inspect;
#[path = "numeric_runtime.rs"]
mod numeric_runtime;
use builtins::Builtin;

#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(Rc<String>),
    Unit,
    Closure(usize),
}
impl Value {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::Bool(_) => "bool",
            Self::Str(_) => "str",
            Self::Unit => "unit",
            Self::Closure(_) => "closure",
        }
    }
    pub fn display(&self) -> String {
        match self {
            Self::Int(n) => n.to_string(),
            Self::Float(n) => {
                let mut s = n.to_string();
                if !s.contains(['.', 'e', 'E']) {
                    s.push_str(".0");
                }
                s
            }
            Self::Bool(v) => v.to_string(),
            Self::Str(s) => (**s).clone(),
            Self::Unit => "()".into(),
            Self::Closure(_) => "<closure>".into(),
        }
    }
    pub fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Unit, Self::Unit) => true,
            (Self::Closure(a), Self::Closure(b)) => a == b,
            _ => false,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum Cell {
    Empty,
    Value(Value),
    Slot(usize),
}
#[derive(Clone, Debug)]
struct Slot {
    value: Value,
    writable: bool,
}
#[derive(Clone, Copy, Debug)]
enum Code {
    User(usize),
    Builtin(Builtin),
    Data,
}
#[derive(Clone, Debug)]
struct Closure {
    fields: Vec<(String, usize)>,
    code: Code,
    jit: bool,
    marked: Span,
}
#[derive(Debug)]
pub(crate) struct Frame {
    pub pc: usize,
    pub persistent: bool,
    pub native: bool,
    pub function: usize,
    pub registers: Vec<Cell>,
    pub current: Option<usize>,
    pub result: Option<Value>,
}
#[derive(Clone, Debug)]
pub struct Config {
    pub jit_enabled: bool,
    pub opt_level: u8,
    pub jit_log: bool,
    pub echo_output: bool,
    pub capture_output: bool,
    pub args: Vec<String>,
    pub ir_dir: Option<PathBuf>,
    pub gc_threshold: usize,
    pub max_steps: Option<u64>,
    pub max_call_depth: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            jit_enabled: true,
            opt_level: 2,
            jit_log: false,
            echo_output: false,
            capture_output: true,
            args: vec![],
            ir_dir: None,
            gc_threshold: 1024,
            max_steps: None,
            max_call_depth: 256,
        }
    }
}
#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub compiled_functions: u64,
    pub native_calls: u64,
    pub native_integer_ops: u64,
    pub native_helper_calls: u64,
    pub scalar_calls: u64,
    pub scalar_fallbacks: u64,
    pub slot_allocations: u64,
    pub closure_allocations: u64,
    pub gc_ms: f64,
    pub native_code_bytes: usize,
    pub compile_ms: f64,
    pub interpreted_calls: u64,
    pub gc_collections: u64,
    pub live_slots: usize,
    pub live_closures: usize,
    pub live_strings: usize,
    pub live_string_bytes: usize,
    pub steps: u64,
}

#[derive(Clone, Copy, Default)]
struct FunctionStats {
    native_calls: u64,
    interpreted_calls: u64,
    native_integer_ops: u64,
    scalar_calls: u64,
    scalar_fallbacks: u64,
}

pub struct Runtime {
    pub program: Program,
    pub config: Config,
    pub output: String,
    pub stats: Stats,
    slots: Vec<Option<Slot>>,
    closures: Vec<Option<Closure>>,
    free_slots: Vec<usize>,
    free_closures: Vec<usize>,
    pub(crate) frames: Vec<Frame>,
    prelude: [usize; 2],
    allocations: usize,
    last_result: Option<Value>,
    globals: BTreeMap<String, usize>,
    steps_at_start: u64,
    jit: Jit,
    function_stats: HashMap<usize, FunctionStats>,
    terminal: Terminal,
    started: Instant,
    random_state: u64,
    pub(crate) error: Option<Error>,
}
impl Runtime {
    pub fn new(program: Program, config: Config) -> Self {
        let mut rt = Self {
            program,
            config,
            output: String::new(),
            stats: Stats::default(),
            slots: vec![],
            closures: vec![],
            free_slots: vec![],
            free_closures: vec![],
            frames: vec![],
            prelude: [0, 0],
            allocations: 0,
            last_result: None,
            globals: BTreeMap::new(),
            steps_at_start: 0,
            jit: Jit::new(),
            function_stats: HashMap::new(),
            terminal: Terminal::new(),
            started: Instant::now(),
            random_state: (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64)
                ^ u64::from(std::process::id()),
            error: None,
        };
        rt.install_builtins();
        rt
    }
    pub fn run(&mut self) -> Result<Value> {
        self.run_top(0, false)
    }
    pub fn evaluate_fragment(&mut self, body: &crate::syntax::Body) -> Result<Value> {
        let names = self.globals.keys().cloned().collect::<Vec<_>>();
        let function = crate::ir::compile_fragment(&mut self.program, &names, body)?;
        self.run_top(function, true)
    }
    pub fn global_values(&self) -> Vec<(String, Value)> {
        self.globals
            .iter()
            .map(|(n, s)| (n.clone(), self.slot(*s).value.clone()))
            .collect()
    }
    pub fn forget_global(&mut self, name: &str) -> bool {
        let found = self.globals.remove(name).is_some();
        if found {
            self.last_result = None;
            self.collect();
        }
        found
    }
    fn run_top(&mut self, function: usize, persistent: bool) -> Result<Value> {
        if !self.frames.is_empty() {
            return Err(Error::new(
                "RuntimeStateError",
                "runtime is already executing",
                Span::default(),
            ));
        }
        self.last_result = None;
        self.error = None;
        self.steps_at_start = self.stats.steps;
        let mut frame = self.make_frame(function, None, &[])?;
        frame.persistent = persistent;
        self.frames.push(frame);
        let result = self.interpret(0).and_then(|_| {
            self.frames[0].result.clone().ok_or_else(|| {
                Error::new(
                    "InternalError",
                    "program returned no result",
                    Span::default(),
                )
            })
        });
        let frame = self.frames.pop().unwrap();
        if persistent {
            for (name, reg) in &self.program.functions[function].global_outputs {
                if let Cell::Slot(slot) = frame.registers[*reg] {
                    self.globals.insert(name.clone(), slot);
                }
            }
        }
        if let Ok(value) = &result {
            self.last_result = Some(value.clone());
        }
        self.collect();
        let cleanup = self
            .terminal
            .leave()
            .map_err(|e| Error::new("TerminalError", e.to_string(), Span::default()));
        match (result, cleanup) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    fn new_slot(&mut self, value: Value, writable: bool) -> usize {
        self.stats.slot_allocations += 1;
        self.allocations += 1;
        let slot = Some(Slot { value, writable });
        if let Some(id) = self.free_slots.pop() {
            self.slots[id] = slot;
            id
        } else {
            let id = self.slots.len();
            self.slots.push(slot);
            id
        }
    }
    fn new_closure(&mut self, closure: Closure) -> usize {
        self.stats.closure_allocations += 1;
        self.allocations += 1;
        if let Some(id) = self.free_closures.pop() {
            self.closures[id] = Some(closure);
            id
        } else {
            let id = self.closures.len();
            self.closures.push(Some(closure));
            id
        }
    }
    fn slot(&self, id: usize) -> &Slot {
        self.slots[id].as_ref().expect("live slot")
    }
    fn closure(&self, id: usize) -> &Closure {
        self.closures[id].as_ref().expect("live closure")
    }
    fn value(&self, frame: usize, reg: usize) -> Value {
        match &self.frames[frame].registers[reg] {
            Cell::Value(v) => v.clone(),
            _ => panic!("value register expected"),
        }
    }
    fn slot_id(&self, frame: usize, reg: usize) -> usize {
        match self.frames[frame].registers[reg] {
            Cell::Slot(s) => s,
            _ => panic!("slot register expected"),
        }
    }
    fn put(&mut self, frame: usize, reg: usize, value: Value) {
        self.frames[frame].registers[reg] = Cell::Value(value);
    }
    fn type_error(&self, message: impl Into<String>, span: Span) -> Error {
        Error::new("TypeError", message, span)
    }
    fn callable(&self, value: &Value, span: Span) -> Result<usize> {
        match value {
            Value::Closure(id) if !matches!(self.closure(*id).code, Code::Data) => Ok(*id),
            _ => Err(Error::new("NotCallable", "value has no code entry", span)),
        }
    }
    fn check_call(&self, value: &Value, modes: &[bool], span: Span) -> Result<usize> {
        let id = self.callable(value, span)?;
        let expected = match &self.closure(id).code {
            Code::User(f) => self.program.functions[*f]
                .parameters
                .iter()
                .map(|(p, _)| p.by_ref)
                .collect::<Vec<_>>(),
            Code::Builtin(b) => vec![false; b.arity()],
            Code::Data => unreachable!(),
        };
        if expected.len() != modes.len() {
            return Err(Error::new(
                "ArityError",
                format!("expected {} arguments, got {}", expected.len(), modes.len()),
                span,
            ));
        }
        if expected != modes {
            return Err(Error::new(
                "ArgumentModeError",
                "value and &slot arguments must match the parameter modes",
                span,
            ));
        }
        Ok(id)
    }
    fn make_frame(
        &mut self,
        function: usize,
        current: Option<usize>,
        args: &[Cell],
    ) -> Result<Frame> {
        let f = &self.program.functions[function];
        let parameters = f.parameters.clone();
        let global_inputs = f.global_inputs.clone();
        let captures = f.captures.clone();
        let mut registers = vec![Cell::Empty; f.registers];
        registers[0] = Cell::Slot(self.prelude[0]);
        registers[1] = Cell::Slot(self.prelude[1]);
        if current.is_none() {
            for (name, reg) in global_inputs {
                if let Some(slot) = self.globals.get(&name) {
                    registers[reg] = Cell::Slot(*slot);
                }
            }
        }
        if let Some(id) = current {
            let c = self.closure(id).clone();
            for ((_, r), (_, slot)) in captures.iter().zip(c.fields.iter()) {
                registers[*r] = Cell::Slot(*slot);
            }
        }
        for ((parameter, r), arg) in parameters.iter().zip(args) {
            registers[*r] = match (arg, parameter.by_ref) {
                (Cell::Value(v), false) => Cell::Slot(self.new_slot(v.clone(), true)),
                (Cell::Slot(s), true) => Cell::Slot(*s),
                _ => {
                    return Err(Error::new(
                        "ArgumentModeError",
                        "argument mode mismatch",
                        parameter.span,
                    ));
                }
            };
        }
        Ok(Frame {
            pc: 0,
            persistent: false,
            native: false,
            function,
            registers,
            current,
            result: None,
        })
    }
    fn invoke(&mut self, id: usize, args: Vec<Cell>, span: Span) -> Result<Value> {
        let (code, jit, marked) = {
            let c = self.closure(id);
            (c.code, c.jit, c.marked)
        };
        if let Code::Builtin(b) = code {
            return self.builtin(b, args, span);
        }
        let Code::User(function) = code else {
            unreachable!()
        };
        if self.frames.len() >= self.config.max_call_depth {
            return Err(Error::new(
                "RecursionLimit",
                "maximum call depth reached",
                span,
            ));
        }
        let native = if jit && self.config.jit_enabled {
            Some(self.ensure_native(function, marked)?)
        } else {
            None
        };
        let mut frame = self.make_frame(function, Some(id), &args)?;
        frame.native = native.is_some();
        let index = self.frames.len();
        self.frames.push(frame);
        let status = if let Some(entry) = native {
            self.stats.native_calls += 1;
            self.function_stats
                .entry(function)
                .or_default()
                .native_calls += 1;
            // Frame and runtime addresses are accessed only through callbacks during this call.
            let result = unsafe {
                entry(
                    self as *mut Self,
                    index,
                    crate::jit::step_callback,
                    crate::jit::condition_callback,
                    crate::jit::integer_operands_callback,
                    crate::jit::integer_result_callback,
                    crate::jit::numeric_callback,
                )
            };
            if result == 0 {
                Ok(())
            } else {
                Err(self.error.take().unwrap_or_else(|| {
                    Error::new(
                        "InternalError",
                        "native entry failed without diagnostic",
                        span,
                    )
                }))
            }
        } else {
            self.stats.interpreted_calls += 1;
            self.function_stats
                .entry(function)
                .or_default()
                .interpreted_calls += 1;
            self.interpret(index)
        };
        let returned = self.frames.pop().unwrap().result;
        match status {
            Ok(()) => returned
                .ok_or_else(|| Error::new("InternalError", "function returned no result", span)),
            Err(mut e) => {
                e.trace.push(span);
                Err(e)
            }
        }
    }
    fn ensure_native(&mut self, function: usize, marked: Span) -> Result<crate::jit::NativeEntry> {
        if let Some(artifact) = self.jit.artifact(function) {
            return artifact.as_ref().map(|a| a.entry)
                .map_err(|e| Error::new("JitCompileError", e.clone(), marked));
        }
        let started = Instant::now();
        let (entry, compiled) = self
            .jit
            .compile(
                function,
                &self.program.functions[function],
                self.config.ir_dir.as_deref(),
                self.config.opt_level,
            )
            .map_err(|e| Error::new("JitCompileError", e, marked))?;
        if compiled {
            if let Some(Ok(artifact)) = self.jit.artifact(function) {
                self.stats.native_code_bytes += artifact.machine.as_ref().map_or(0, |m| m.bytes.len());
            }
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            self.stats.compile_ms += ms;
            self.stats.compiled_functions += 1;
            if self.config.jit_log {
                eprintln!(
                    "[jit] compiled function={function} LLVM={} backend=ORC-LLJIT compile_ms={ms:.3}",
                    env!("PRIM_LLVM_VERSION")
                );
            }
        }
        Ok(entry)
    }
    fn tick(&mut self, span: Span) -> Result<()> {
        if self.terminal.interrupted() {
            return Err(Error::new("Interrupted", "terminal interrupted", span));
        }
        self.stats.steps += 1;
        if self
            .config
            .max_steps
            .is_some_and(|limit| self.stats.steps.saturating_sub(self.steps_at_start) > limit)
        {
            Err(Error::new(
                "ExecutionLimit",
                "execution fuel exhausted",
                span,
            ))
        } else {
            Ok(())
        }
    }
    fn interpret(&mut self, frame: usize) -> Result<()> {
        self.interpret_from(frame, 0)
    }
    fn interpret_from(&mut self, frame: usize, mut pc: usize) -> Result<()> {
        let function = self.frames[frame].function;
        loop {
            match &self.program.functions[function].code[pc].instruction {
                Instruction::Jump { target } => pc = *target,
                Instruction::Branch { yes, no, .. } => {
                    let (yes, no) = (*yes, *no);
                    pc = if self.condition(frame, pc)? { yes } else { no };
                }
                Instruction::Return { .. } => {
                    self.step(frame, pc)?;
                    return Ok(());
                }
                _ => {
                    self.step(frame, pc)?;
                    pc += 1;
                }
            }
        }
    }
    pub(crate) fn condition(&mut self, frame: usize, pc: usize) -> Result<bool> {
        self.frames[frame].pc = pc;
        let op = &self.program.functions[self.frames[frame].function].code[pc];
        let Instruction::Branch { cond, yes, no } = op.instruction else {
            unreachable!()
        };
        let span = op.span;
        self.tick(span)?;
        match self.value(frame, cond) {
            Value::Bool(b) => {
                self.frames[frame].pc = if b { yes } else { no };
                Ok(b)
            }
            v => Err(self.type_error(
                format!("condition requires bool, got {}", v.kind()),
                span,
            )),
        }
    }
    pub(crate) fn integer_operands(&mut self, frame: usize, pc: usize) -> Option<(i64, i64)> {
        self.frames[frame].pc = pc;
        let Instruction::Binary { left, right, .. } =
            self.program.functions[self.frames[frame].function].code[pc].instruction
        else {
            unreachable!()
        };
        match (self.value(frame, left), self.value(frame, right)) {
            (Value::Int(a), Value::Int(b)) => Some((a, b)),
            _ => None,
        }
    }
    pub(crate) fn integer_result(&mut self, frame: usize, pc: usize, value: i64) -> Result<()> {
        let op = &self.program.functions[self.frames[frame].function].code[pc];
        let Instruction::Binary { dst, .. } = op.instruction else {
            unreachable!()
        };
        let span = op.span;
        self.tick(span)?;
        self.put(frame, dst, Value::Int(value));
        self.stats.native_integer_ops += 1;
        self.function_stats
            .entry(self.frames[frame].function)
            .or_default()
            .native_integer_ops += 1;
        self.frames[frame].pc = pc + 1;
        Ok(())
    }
    pub(crate) fn step(&mut self, frame: usize, pc: usize) -> Result<()> {
        self.frames[frame].pc = pc;
        let op = self.program.functions[self.frames[frame].function].code[pc].clone();
        let span = op.span;
        self.tick(span)?;
        match op.instruction {
            Instruction::Const { dst, value } => self.put(
                frame,
                dst,
                match value {
                    Constant::Int(n) => Value::Int(n),
                    Constant::Float(n) => Value::Float(n),
                    Constant::Bool(b) => Value::Bool(b),
                    Constant::Str(s) => Value::Str(Rc::new(s)),
                    Constant::Unit => Value::Unit,
                },
            ),
            Instruction::Copy { dst, src } => {
                self.frames[frame].registers[dst] = self.frames[frame].registers[src].clone()
            }
            Instruction::NewSlot { dst, src } => {
                let v = self.value(frame, src);
                let slot = self.new_slot(v, true);
                self.frames[frame].registers[dst] = Cell::Slot(slot);
            }
            Instruction::Load { dst, slot } => {
                let v = self.slot(self.slot_id(frame, slot)).value.clone();
                self.put(frame, dst, v);
            }
            Instruction::Store { slot, src } => {
                let id = self.slot_id(frame, slot);
                if !self.slot(id).writable {
                    return Err(Error::new("ReadOnlySlot", "slot cannot be assigned", span));
                }
                let value = self.value(frame, src);
                self.slots[id].as_mut().unwrap().value = value;
            }
            Instruction::Field {
                dst,
                value,
                name,
                writable,
            } => {
                let Value::Closure(id) = self.value(frame, value) else {
                    return Err(self.type_error("field receiver must be a closure", span));
                };
                let slot = self
                    .closure(id)
                    .fields
                    .iter()
                    .find(|(n, _)| n == &name)
                    .map(|(_, s)| *s)
                    .ok_or_else(|| {
                        Error::new(
                            "MissingField",
                            format!("field '{name}' does not exist"),
                            span,
                        )
                    })?;
                if writable && !self.slot(slot).writable {
                    return Err(Error::new(
                        "ReadOnlySlot",
                        format!("field '{name}' is read-only"),
                        span,
                    ));
                }
                self.frames[frame].registers[dst] = Cell::Slot(slot);
            }
            Instruction::SelfValue { dst } => self.put(
                frame,
                dst,
                Value::Closure(self.frames[frame].current.unwrap()),
            ),
            Instruction::Unary { dst, op, src } => {
                let value = unary(&op, self.value(frame, src), span)?;
                self.put(frame, dst, value);
            }
            Instruction::Binary {
                dst,
                op,
                left,
                right,
            } => {
                let value = binary(&op, self.value(frame, left), self.value(frame, right), span)?;
                self.put(frame, dst, value);
            }
            Instruction::BoolCheck { src } => {
                if !matches!(self.value(frame, src), Value::Bool(_)) {
                    return Err(self.type_error("logical operand must be bool", span));
                }
            }
            Instruction::MakeData { dst, fields } => {
                let fields = fields
                    .into_iter()
                    .map(|(n, r)| (n, self.slot_id(frame, r)))
                    .collect();
                let c = self.new_closure(Closure {
                    fields,
                    code: Code::Data,
                    jit: false,
                    marked: span,
                });
                self.put(frame, dst, Value::Closure(c));
            }
            Instruction::MakeClosure {
                dst,
                function,
                captures,
            } => {
                let names = self.program.functions[function]
                    .captures
                    .iter()
                    .map(|(n, _)| n.clone())
                    .collect::<Vec<_>>();
                let mut fields = vec![];
                for ((r, by_ref), name) in captures.into_iter().zip(names) {
                    let source = self.slot_id(frame, r);
                    let slot = if by_ref {
                        source
                    } else {
                        self.new_slot(self.slot(source).value.clone(), true)
                    };
                    fields.push((name, slot));
                }
                let c = self.new_closure(Closure {
                    fields,
                    code: Code::User(function),
                    jit: false,
                    marked: span,
                });
                self.put(frame, dst, Value::Closure(c));
            }
            Instruction::CheckCall { callee, modes } => {
                self.check_call(&self.value(frame, callee), &modes, span)?;
            }
            Instruction::Call { dst, callee, args } => {
                let id = self.callable(&self.value(frame, callee), span)?;
                let args = args
                    .into_iter()
                    .map(|r| self.frames[frame].registers[r].clone())
                    .collect();
                let result = self.invoke(id, args, span)?;
                self.put(frame, dst, result);
            }
            Instruction::CheckCallable { src } => {
                if self.callable(&self.value(frame, src), span).is_err() {
                    return Err(Error::new(
                        "DecoratorResultError",
                        "decorator must return a callable closure",
                        span,
                    ));
                }
            }
            Instruction::Return { src } => self.frames[frame].result = Some(self.value(frame, src)),
            Instruction::Jump { .. } | Instruction::Branch { .. } => unreachable!(),
        }
        self.frames[frame].pc = if self.frames[frame].result.is_some() {
            self.program.functions[self.frames[frame].function]
                .code
                .len()
        } else {
            pc + 1
        };
        if self.allocations >= self.config.gc_threshold.max(1) {
            self.collect();
        }
        Ok(())
    }
    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }
    pub fn collect(&mut self) {
        let started = Instant::now();
        let mut slots = HashSet::new();
        let mut closures = HashSet::new();
        let mut strings = HashSet::new();
        let mut string_bytes = 0;
        let mut work = vec![];
        for slot in self.globals.values() {
            work.push(Cell::Slot(*slot));
        }
        for s in self.prelude {
            work.push(Cell::Slot(s));
        }
        if let Some(v) = &self.last_result {
            work.push(Cell::Value(v.clone()));
        }
        for f in &mut self.frames {
            let live = self.program.functions[f.function]
                .live_in
                .get(f.pc)
                .map_or(&[][..], |r| r.as_slice());
            for (index, cell) in f.registers.iter_mut().enumerate() {
                if live.binary_search(&index).is_ok()
                    || f.persistent
                        && self.program.functions[f.function]
                            .global_outputs
                            .iter()
                            .any(|(_, r)| *r == index)
                {
                    work.push(cell.clone());
                } else {
                    *cell = Cell::Empty;
                }
            }
            if let Some(c) = f.current {
                work.push(Cell::Value(Value::Closure(c)));
            }
            if let Some(v) = &f.result {
                work.push(Cell::Value(v.clone()));
            }
        }
        while let Some(item) = work.pop() {
            match item {
                Cell::Slot(s) => {
                    if slots.insert(s) {
                        work.push(Cell::Value(self.slot(s).value.clone()));
                    }
                }
                Cell::Value(Value::Str(s)) => {
                    if strings.insert(Rc::as_ptr(&s)) {
                        string_bytes += s.len();
                    }
                }
                Cell::Value(Value::Closure(c)) => {
                    if closures.insert(c) {
                        for (_, s) in &self.closure(c).fields {
                            work.push(Cell::Slot(*s));
                        }
                    }
                }
                _ => {}
            }
        }
        for (id, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_some() && !slots.contains(&id) {
                *slot = None;
                self.free_slots.push(id);
            }
        }
        for (id, c) in self.closures.iter_mut().enumerate() {
            if c.is_some() && !closures.contains(&id) {
                *c = None;
                self.free_closures.push(id);
            }
        }
        trim_arena(&mut self.slots, &mut self.free_slots);
        trim_arena(&mut self.closures, &mut self.free_closures);
        self.allocations = 0;
        self.stats.gc_collections += 1;
        self.stats.live_slots = slots.len();
        self.stats.live_closures = closures.len();
        self.stats.live_strings = strings.len();
        self.stats.live_string_bytes = string_bytes;
        self.stats.gc_ms += started.elapsed().as_secs_f64() * 1000.0;
    }
}
fn overflow(span: Span) -> Error {
    Error::new("IntegerOverflow", "signed 64-bit integer overflow", span)
}
fn finite(n: f64, span: Span) -> Result<Value> {
    if n.is_finite() {
        Ok(Value::Float(n))
    } else {
        Err(Error::new(
            "FloatRangeError",
            "floating-point result is not finite",
            span,
        ))
    }
}
fn unary(op: &str, v: Value, span: Span) -> Result<Value> {
    match (op, v) {
        ("!", Value::Bool(b)) => Ok(Value::Bool(!b)),
        ("+", v @ Value::Int(_)) | ("+", v @ Value::Float(_)) => Ok(v),
        ("-", Value::Int(n)) => n
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| overflow(span)),
        ("-", Value::Float(n)) => finite(-n, span),
        _ => Err(Error::new("TypeError", "invalid unary operand", span)),
    }
}
fn binary(op: &str, a: Value, b: Value, span: Span) -> Result<Value> {
    if op == "==" || op == "!=" {
        return Ok(Value::Bool(a.equals(&b) == (op == "==")));
    }
    let zero = || Error::new("DivisionByZero", "division or remainder by zero", span);
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => {
            let n = match op {
                "+" => a.checked_add(b),
                "-" => a.checked_sub(b),
                "*" => a.checked_mul(b),
                "/" => {
                    if b == 0 {
                        return Err(zero());
                    }
                    a.checked_div(b)
                }
                "%" => {
                    if b == 0 {
                        return Err(zero());
                    }
                    a.checked_rem(b)
                }
                "<" => return Ok(Value::Bool(a < b)),
                "<=" => return Ok(Value::Bool(a <= b)),
                ">" => return Ok(Value::Bool(a > b)),
                ">=" => return Ok(Value::Bool(a >= b)),
                _ => return Err(Error::new("TypeError", "invalid integer operation", span)),
            };
            n.map(Value::Int).ok_or_else(|| overflow(span))
        }
        (Value::Float(a), Value::Float(b)) => match op {
            "+" => finite(a + b, span),
            "-" => finite(a - b, span),
            "*" => finite(a * b, span),
            "/" => {
                if b == 0.0 {
                    Err(zero())
                } else {
                    finite(a / b, span)
                }
            }
            "<" => Ok(Value::Bool(a < b)),
            "<=" => Ok(Value::Bool(a <= b)),
            ">" => Ok(Value::Bool(a > b)),
            ">=" => Ok(Value::Bool(a >= b)),
            _ => Err(Error::new("TypeError", "invalid float operation", span)),
        },
        (Value::Str(a), Value::Str(b)) if op == "+" => Ok(Value::Str(Rc::new(format!("{a}{b}")))),
        (a, b) => Err(Error::new(
            "TypeError",
            format!(
                "operator {op} does not accept {} and {}",
                a.kind(),
                b.kind()
            ),
            span,
        )),
    }
}

fn trim_arena<T>(arena: &mut Vec<Option<T>>, free: &mut Vec<usize>) {
    let length = arena.iter().rposition(Option::is_some).map_or(0, |i| i + 1);
    arena.truncate(length);
    free.retain(|id| *id < length);
    if arena.capacity() > length.saturating_mul(4).max(256) {
        arena.shrink_to(length.max(128));
    }
    if free.capacity() > free.len().saturating_mul(4).max(256) {
        free.shrink_to(free.len().max(128));
    }
}
