use crate::syntax::*;

pub type Reg = usize;
#[derive(Clone, Debug)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Unit,
}
#[derive(Clone, Debug)]
pub enum Instruction {
    Const {
        dst: Reg,
        value: Constant,
    },
    Copy {
        dst: Reg,
        src: Reg,
    },
    NewSlot {
        dst: Reg,
        src: Reg,
    },
    Load {
        dst: Reg,
        slot: Reg,
    },
    Store {
        slot: Reg,
        src: Reg,
    },
    Field {
        dst: Reg,
        value: Reg,
        name: String,
        writable: bool,
    },
    SelfValue {
        dst: Reg,
    },
    Unary {
        dst: Reg,
        op: String,
        src: Reg,
    },
    Binary {
        dst: Reg,
        op: String,
        left: Reg,
        right: Reg,
    },
    BoolCheck {
        src: Reg,
    },
    MakeData {
        dst: Reg,
        fields: Vec<(String, Reg)>,
    },
    MakeClosure {
        dst: Reg,
        function: usize,
        captures: Vec<(Reg, bool)>,
    },
    CheckCall {
        callee: Reg,
        modes: Vec<bool>,
    },
    Call {
        dst: Reg,
        callee: Reg,
        args: Vec<Reg>,
    },
    CheckCallable {
        src: Reg,
    },
    Jump {
        target: usize,
    },
    Branch {
        cond: Reg,
        yes: usize,
        no: usize,
    },
    Return {
        src: Reg,
    },
}
#[derive(Clone, Debug)]
pub struct Op {
    pub instruction: Instruction,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Function {
    pub global_inputs: Vec<(String, Reg)>,
    pub global_outputs: Vec<(String, Reg)>,
    pub live_in: Vec<Vec<Reg>>,
    pub code: Vec<Op>,
    pub registers: usize,
    pub captures: Vec<(String, Reg)>,
    pub capture_modes: Vec<bool>,
    pub definition_span: Span,
    pub parameters: Vec<(Binding, Reg)>,
    pub span: Span,
}
#[derive(Clone, Debug, Default)]
pub struct Program {
    pub sources: Vec<crate::source::SourceUnit>,
    pub functions: Vec<Function>,
}

pub fn compile(body: &Body) -> Result<Program> {
    let mut p = Program::default();
    compile_function(&mut p, &[], &[], body, true)?;
    Ok(p)
}
fn compile_function(
    p: &mut Program,
    captures: &[Binding],
    params: &[Binding],
    body: &Body,
    top: bool,
) -> Result<usize> {
    let id = p.functions.len();
    p.functions.push(Function {
        global_inputs: vec![],
        global_outputs: vec![],
        live_in: vec![],
        code: vec![],
        registers: 2,
        captures: vec![],
        capture_modes: vec![],
        definition_span: body.span,
        parameters: vec![],
        span: body.span,
    });
    let mut b = Builder {
        function: p.functions[id].clone(),
        scopes: vec![vec![]],
        loops: vec![],
        can_return: !top,
        has_self: !top,
    };
    for capture in captures {
        let reg = b.define(&capture.name, capture.span)?;
        b.function.captures.push((capture.name.clone(), reg));
        b.function.capture_modes.push(capture.by_ref);
    }
    for param in params {
        let reg = b.define(&param.name, param.span)?;
        b.function.parameters.push((param.clone(), reg));
    }
    let result = b.body(body, false, p)?;
    b.emit(Instruction::Return { src: result }, body.span);
    b.function.live_in = crate::liveness::analyze(&b.function);
    p.functions[id] = b.function;
    Ok(id)
}
struct LoopContext {
    start: usize,
    result: Reg,
    exits: Vec<usize>,
}
struct Builder {
    function: Function,
    scopes: Vec<Vec<(String, Reg)>>,
    loops: Vec<LoopContext>,
    can_return: bool,
    has_self: bool,
}
impl Builder {
    fn reg(&mut self) -> Reg {
        let n = self.function.registers;
        self.function.registers += 1;
        n
    }
    fn emit(&mut self, instruction: Instruction, span: Span) -> usize {
        let i = self.function.code.len();
        self.function.code.push(Op { instruction, span });
        i
    }
    fn constant(&mut self, c: Constant, span: Span) -> Reg {
        let dst = self.reg();
        self.emit(Instruction::Const { dst, value: c }, span);
        dst
    }
    fn lookup(&self, name: &str, span: Span) -> Result<Reg> {
        if name == "std" {
            return Ok(0);
        }
        if name == "jit" {
            return Ok(1);
        }
        for scope in self.scopes.iter().rev() {
            for (n, r) in scope.iter().rev() {
                if n == name {
                    return Ok(*r);
                }
            }
        }
        Err(Error::new(
            "UndefinedName",
            format!("'{name}' is not visible; closures must explicitly capture outer names"),
            span,
        ))
    }
    fn define(&mut self, name: &str, span: Span) -> Result<Reg> {
        if name == "std" || name == "jit" {
            return Err(Error::new(
                "ReservedBinding",
                format!("'{name}' is a read-only builtin"),
                span,
            ));
        }
        if self.scopes.last().unwrap().iter().any(|(n, _)| n == name) {
            return Err(Error::new(
                "DuplicateBinding",
                format!("'{name}' is already declared in this scope"),
                span,
            ));
        }
        let r = self.reg();
        self.scopes.last_mut().unwrap().push((name.into(), r));
        Ok(r)
    }
    fn body(&mut self, body: &Body, scope: bool, p: &mut Program) -> Result<Reg> {
        if scope {
            self.scopes.push(vec![]);
        }
        for s in &body.statements {
            self.statement(s, p)?;
        }
        let value = if let Some(e) = &body.tail {
            self.expr(e, p)?
        } else {
            self.constant(Constant::Unit, body.span)
        };
        if scope {
            self.scopes.pop();
        }
        Ok(value)
    }
    fn place(&mut self, e: &Expr, p: &mut Program, writable: bool) -> Result<Reg> {
        match &e.kind {
            ExprKind::Group(e) => self.place(e, p, writable),
            ExprKind::Name(n) => {
                let r = self.lookup(n, e.span)?;
                if writable && r < 2 {
                    return Err(Error::new(
                        "InvalidPlace",
                        "builtin binding is read-only",
                        e.span,
                    ));
                }
                Ok(r)
            }
            ExprKind::Field(receiver, name) => {
                let value = self.expr(receiver, p)?;
                let dst = self.reg();
                self.emit(
                    Instruction::Field {
                        dst,
                        value,
                        name: name.clone(),
                        writable,
                    },
                    e.span,
                );
                Ok(dst)
            }
            _ => Err(Error::new(
                "InvalidPlace",
                "expected a writable variable or existing field",
                e.span,
            )),
        }
    }
    fn statement(&mut self, s: &Stmt, p: &mut Program) -> Result<()> {
        match &s.kind {
            StmtKind::Let(name, arg) => {
                let src = if arg.by_ref {
                    self.place(&arg.expr, p, true)?
                } else {
                    self.expr(&arg.expr, p)?
                };
                let dst = self.define(name, s.span)?;
                self.emit(
                    if arg.by_ref {
                        Instruction::Copy { dst, src }
                    } else {
                        Instruction::NewSlot { dst, src }
                    },
                    s.span,
                );
            }
            StmtKind::Assign(target, value) => {
                let slot = self.place(target, p, true)?;
                let src = self.expr(value, p)?;
                self.emit(Instruction::Store { slot, src }, s.span);
            }
            StmtKind::Expr(e) => {
                self.expr(e, p)?;
            }
            StmtKind::Return(e) => {
                if !self.can_return {
                    return Err(Error::new(
                        "InvalidControlTarget",
                        "return requires a function and cannot cross a data initializer",
                        s.span,
                    ));
                }
                let src = if let Some(e) = e {
                    self.expr(e, p)?
                } else {
                    self.constant(Constant::Unit, s.span)
                };
                self.emit(Instruction::Return { src }, s.span);
            }
            StmtKind::Break(e) => {
                if self.loops.is_empty() {
                    return Err(Error::new(
                        "InvalidControlTarget",
                        "break requires a loop in this execution boundary",
                        s.span,
                    ));
                }
                let src = if let Some(e) = e {
                    self.expr(e, p)?
                } else {
                    self.constant(Constant::Unit, s.span)
                };
                let dst = self.loops.last().unwrap().result;
                self.emit(Instruction::Copy { dst, src }, s.span);
                let pos = self.emit(Instruction::Jump { target: 0 }, s.span);
                self.loops.last_mut().unwrap().exits.push(pos);
            }
            StmtKind::Continue => {
                let target = self
                    .loops
                    .last()
                    .ok_or_else(|| {
                        Error::new("InvalidControlTarget", "continue requires a loop", s.span)
                    })?
                    .start;
                self.emit(Instruction::Jump { target }, s.span);
            }
        }
        Ok(())
    }
    fn expr(&mut self, e: &Expr, p: &mut Program) -> Result<Reg> {
        let dst = match &e.kind {
            ExprKind::Int(raw) => {
                let magnitude = parse_integer(raw, e.span)?;
                let v = i64::try_from(magnitude).map_err(|_| {
                    Error::new(
                        "IntegerLiteralOutOfRange",
                        "integer is outside signed 64-bit range",
                        e.span,
                    )
                })?;
                self.constant(Constant::Int(v), e.span)
            }
            ExprKind::Float(raw) => {
                let n = raw
                    .replace('_', "")
                    .parse::<f64>()
                    .map_err(|_| Error::new("FloatLiteralOutOfRange", "invalid float", e.span))?;
                if !n.is_finite() {
                    return Err(Error::new(
                        "FloatLiteralOutOfRange",
                        "float must be finite",
                        e.span,
                    ));
                }
                self.constant(Constant::Float(n), e.span)
            }
            ExprKind::Str(s) => self.constant(Constant::Str(s.clone()), e.span),
            ExprKind::Bool(v) => self.constant(Constant::Bool(*v), e.span),
            ExprKind::Unit => self.constant(Constant::Unit, e.span),
            ExprKind::Name(_) | ExprKind::Field(_, _) => {
                let slot = self.place(e, p, false)?;
                let dst = self.reg();
                self.emit(Instruction::Load { dst, slot }, e.span);
                dst
            }
            ExprKind::SelfValue => {
                if !self.has_self {
                    return Err(Error::new(
                        "InvalidSelf",
                        "self requires a callable closure",
                        e.span,
                    ));
                }
                let dst = self.reg();
                self.emit(Instruction::SelfValue { dst }, e.span);
                dst
            }
            ExprKind::Group(e) => self.expr(e, p)?,
            ExprKind::Unary(op, operand) => {
                if op == "-" {
                    if let Some(raw) = integer_operand(operand) {
                        if parse_integer(raw, operand.span)? == 1u64 << 63 {
                            return Ok(self.constant(Constant::Int(i64::MIN), e.span));
                        }
                    }
                }
                let src = self.expr(operand, p)?;
                let dst = self.reg();
                self.emit(
                    Instruction::Unary {
                        dst,
                        op: op.clone(),
                        src,
                    },
                    e.span,
                );
                dst
            }
            ExprKind::Binary(op, left, right) if op == "&&" || op == "||" => {
                let left = self.expr(left, p)?;
                let dst = self.reg();
                self.emit(Instruction::Copy { dst, src: left }, e.span);
                let branch = self.emit(
                    Instruction::Branch {
                        cond: left,
                        yes: 0,
                        no: 0,
                    },
                    e.span,
                );
                let start = self.function.code.len();
                let right = self.expr(right, p)?;
                self.emit(Instruction::BoolCheck { src: right }, e.span);
                self.emit(Instruction::Copy { dst, src: right }, e.span);
                let end = self.function.code.len();
                self.function.code[branch].instruction = Instruction::Branch {
                    cond: left,
                    yes: if op == "&&" { start } else { end },
                    no: if op == "&&" { end } else { start },
                };
                dst
            }
            ExprKind::Binary(op, left, right) => {
                let left = self.expr(left, p)?;
                let right = self.expr(right, p)?;
                let dst = self.reg();
                self.emit(
                    Instruction::Binary {
                        dst,
                        op: op.clone(),
                        left,
                        right,
                    },
                    e.span,
                );
                dst
            }
            ExprKind::Call(callee, args) => {
                let callee = self.expr(callee, p)?;
                self.emit(
                    Instruction::CheckCall {
                        callee,
                        modes: args.iter().map(|a| a.by_ref).collect(),
                    },
                    e.span,
                );
                let mut registers = vec![];
                for a in args {
                    registers.push(if a.by_ref {
                        self.place(&a.expr, p, true)?
                    } else {
                        self.expr(&a.expr, p)?
                    });
                }
                let dst = self.reg();
                self.emit(
                    Instruction::Call {
                        dst,
                        callee,
                        args: registers,
                    },
                    e.span,
                );
                dst
            }
            ExprKind::Block(body) => self.body(body, true, p)?,
            ExprKind::If(cond, yes, no) => {
                let cond = self.expr(cond, p)?;
                let dst = self.reg();
                let branch = self.emit(
                    Instruction::Branch {
                        cond,
                        yes: 0,
                        no: 0,
                    },
                    e.span,
                );
                let yes_pc = self.function.code.len();
                let src = self.body(yes, true, p)?;
                self.emit(Instruction::Copy { dst, src }, e.span);
                let skip = self.emit(Instruction::Jump { target: 0 }, e.span);
                let no_pc = self.function.code.len();
                let src = if let Some(no) = no {
                    self.expr(no, p)?
                } else {
                    self.constant(Constant::Unit, e.span)
                };
                self.emit(Instruction::Copy { dst, src }, e.span);
                let end = self.function.code.len();
                self.function.code[branch].instruction = Instruction::Branch {
                    cond,
                    yes: yes_pc,
                    no: no_pc,
                };
                self.function.code[skip].instruction = Instruction::Jump { target: end };
                dst
            }
            ExprKind::Loop(body) => {
                let dst = self.constant(Constant::Unit, e.span);
                let start = self.function.code.len();
                self.loops.push(LoopContext {
                    start,
                    result: dst,
                    exits: vec![],
                });
                self.body(body, true, p)?;
                self.emit(Instruction::Jump { target: start }, e.span);
                let end = self.function.code.len();
                let ctx = self.loops.pop().unwrap();
                for pos in ctx.exits {
                    self.function.code[pos].instruction = Instruction::Jump { target: end };
                }
                dst
            }
            ExprKind::Data(statements) => {
                let loops = std::mem::take(&mut self.loops);
                let returns = self.can_return;
                self.can_return = false;
                self.scopes.push(vec![]);
                for stmt in statements {
                    self.statement(stmt, p)?;
                }
                let fields = self.scopes.pop().unwrap();
                self.loops = loops;
                self.can_return = returns;
                let dst = self.reg();
                self.emit(Instruction::MakeData { dst, fields }, e.span);
                dst
            }
            ExprKind::Closure(captures, params, body) => {
                let mut sources = vec![];
                for capture in captures {
                    let reg = self.lookup(&capture.name, capture.span)?;
                    if reg < 2 {
                        return Err(Error::new(
                            "ReservedBinding",
                            "builtins are visible without capture",
                            capture.span,
                        ));
                    }
                    sources.push((reg, capture.by_ref));
                }
                let function = compile_function(p, captures, params, body, false)?;
                p.functions[function].definition_span = e.span;
                let dst = self.reg();
                self.emit(
                    Instruction::MakeClosure {
                        dst,
                        function,
                        captures: sources,
                    },
                    e.span,
                );
                dst
            }
            ExprKind::Decorate(name, target) => {
                let slot = self.lookup(name, e.span)?;
                let callee = self.reg();
                self.emit(Instruction::Load { dst: callee, slot }, e.span);
                self.emit(
                    Instruction::CheckCall {
                        callee,
                        modes: vec![false],
                    },
                    e.span,
                );
                let arg = self.expr(target, p)?;
                let dst = self.reg();
                self.emit(
                    Instruction::Call {
                        dst,
                        callee,
                        args: vec![arg],
                    },
                    e.span,
                );
                self.emit(Instruction::CheckCallable { src: dst }, e.span);
                dst
            }
        };
        Ok(dst)
    }
}
fn integer_operand(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Int(s) => Some(s),
        ExprKind::Group(e) => integer_operand(e),
        _ => None,
    }
}
fn parse_integer(raw: &str, span: Span) -> Result<u64> {
    let s = raw.replace('_', "");
    let (digits, radix) = if s.starts_with("0x") || s.starts_with("0X") {
        (&s[2..], 16)
    } else if s.starts_with("0o") || s.starts_with("0O") {
        (&s[2..], 8)
    } else if s.starts_with("0b") || s.starts_with("0B") {
        (&s[2..], 2)
    } else {
        (s.as_str(), 10)
    };
    u64::from_str_radix(digits, radix).map_err(|_| {
        Error::new(
            "IntegerLiteralOutOfRange",
            "integer magnitude exceeds supported range",
            span,
        )
    })
}

pub fn compile_fragment(program: &mut Program, globals: &[String], body: &Body) -> Result<usize> {
    let before = program.functions.len();
    let result = (|| {
        let id = program.functions.len();
        program.functions.push(Function {
            global_inputs: vec![],
            global_outputs: vec![],
            live_in: vec![],
            code: vec![],
            registers: 2,
            captures: vec![],
            capture_modes: vec![],
            definition_span: body.span,
            parameters: vec![],
            span: body.span,
        });
        let mut builder = Builder {
            function: program.functions[id].clone(),
            scopes: vec![vec![]],
            loops: vec![],
            can_return: false,
            has_self: false,
        };
        for name in globals {
            let reg = builder.define(name, body.span)?;
            builder.function.global_inputs.push((name.clone(), reg));
        }
        builder.scopes.push(vec![]);
        let value = builder.body(body, false, program)?;
        builder.emit(Instruction::Return { src: value }, body.span);
        builder.function.global_outputs = builder.scopes.pop().unwrap();
        builder.function.live_in = crate::liveness::analyze(&builder.function);
        program.functions[id] = builder.function;
        Ok(id)
    })();
    if result.is_err() {
        program.functions.truncate(before);
    }
    result
}

impl Program {
    pub fn source_at(&self, offset: usize) -> Option<&crate::source::SourceUnit> {
        self.sources.iter().find(|s| s.contains(offset))
    }
    pub fn add_source(&mut self, name: &str, source: &str, start: usize) {
        self.sources.push(crate::source::SourceUnit {
            name: name.into(),
            start,
            text: std::rc::Rc::new(format!("{source}\n")),
        });
    }
}
