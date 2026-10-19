//! Local scalar lowering. No slot may escape or alias another local slot.
//! State is materialized only when resuming the interpreter at a guarded op.
use crate::ir::{Constant, Function, Instruction};
use std::fmt::Write;

pub(crate) fn eligible(f: &Function) -> bool {
    if !f.captures.is_empty()
        || !f.global_inputs.is_empty()
        || f.parameters.iter().any(|(p, _)| p.by_ref)
        || f.registers > 256
        || f.code.len() > 512
    {
        return false;
    }
    let mut slots = vec![false; f.registers];
    for (_, r) in &f.parameters {
        slots[*r] = true;
    }
    for op in &f.code {
        if let Instruction::NewSlot { dst, .. } = op.instruction {
            slots[dst] = true;
        }
    }
    f.code.iter().all(|op| match &op.instruction {
        Instruction::Const { value, .. } => {
            matches!(value, Constant::Int(_) | Constant::Bool(_) | Constant::Unit)
        }
        Instruction::Copy { dst, src } => !slots[*dst] && !slots[*src],
        Instruction::NewSlot { src, .. } => !slots[*src],
        Instruction::Load { slot, .. } => slots[*slot],
        Instruction::Store { slot, src } => slots[*slot] && !slots[*src],
        Instruction::Binary { op, .. } => matches!(
            op.as_str(),
            "+" | "-" | "*" | "/" | "%" | "<" | "<=" | ">" | ">=" | "==" | "!="
        ),
        Instruction::Unary { op, .. } => matches!(op.as_str(), "-" | "!"),
        Instruction::BoolCheck { .. }
        | Instruction::Branch { .. }
        | Instruction::Jump { .. }
        | Instruction::Return { .. } => true,
        _ => false,
    })
}

struct Emitter {
    out: String,
    ptr: &'static str,
    ctx: &'static str,
    serial: usize,
}
impl Emitter {
    fn line(&mut self, s: impl AsRef<str>) {
        writeln!(self.out, "{}", s.as_ref()).unwrap();
    }
    fn temp(&mut self, expr: impl AsRef<str>) -> String {
        let v = format!("%v{}", self.serial);
        self.serial += 1;
        self.line(format!("  {v} = {}", expr.as_ref()));
        v
    }
    fn load(&mut self, ptr: &str) -> String {
        self.temp(format!("load i64, {} {ptr}", self.ptr))
    }
    fn store(&mut self, value: impl std::fmt::Display, ptr: &str) {
        self.line(format!("  store i64 {value}, {} {ptr}", self.ptr));
    }
    fn packet_ptr(&mut self, index: usize) -> String {
        self.temp(format!(
            "getelementptr i64, {} %packet, i64 {index}",
            self.ptr
        ))
    }
    fn packet_store(&mut self, index: usize, value: &str) {
        let p = self.packet_ptr(index);
        self.store(value, &p);
    }
    fn packet_load(&mut self, index: usize) -> String {
        let p = self.packet_ptr(index);
        self.load(&p)
    }
    fn callback(&mut self, action: usize, pc: usize) -> String {
        self.temp(format!(
            "call i32 %numeric({} %ctx, i64 %frame, i64 {action}, i64 {pc}, {} %packet)",
            self.ctx, self.ptr
        ))
    }
    fn flush(&mut self) {
        for (i, name) in ["%steps", "%arith"].iter().enumerate() {
            let v = self.load(name);
            self.packet_store(i, &v);
            self.store(0, name);
        }
    }
    fn bump(&mut self, name: &str) {
        let v = self.load(name);
        let next = self.temp(format!("add i64 {v}, 1"));
        self.store(next, name);
    }
    fn set(&mut self, reg: usize, value: impl std::fmt::Display, kind: impl std::fmt::Display) {
        self.store(value, &format!("%nr{reg}"));
        self.store(kind, &format!("%nt{reg}"));
    }
    fn copy(&mut self, dst: usize, src: usize) {
        let v = self.load(&format!("%nr{src}"));
        let t = self.load(&format!("%nt{src}"));
        self.set(dst, v, t);
    }
    fn guard(&mut self, condition: &str, pc: usize) {
        let label = format!("guard{}", self.serial);
        self.serial += 1;
        self.line(format!(
            "  br i1 {condition}, label %{label}, label %deopt{pc}\n{label}:"
        ));
    }
    fn kind_guard(&mut self, reg: usize, kind: u8, pc: usize) {
        let t = self.load(&format!("%nt{reg}"));
        let ok = self.temp(format!("icmp eq i64 {t}, {kind}"));
        self.guard(&ok, pc);
    }
}

/// Emits the entry and fast path, ending in blocks that return or enter b0.
pub(crate) fn emit(f: &Function, opaque: bool) -> String {
    let mut e = Emitter {
        out: String::new(),
        ptr: if opaque { "ptr" } else { "i64*" },
        ctx: if opaque { "ptr" } else { "i8*" },
        serial: 0,
    };
    e.line(format!(
        "  %packet = alloca i64, i64 {}",
        2 + 2 * f.registers
    ));
    e.line("  %steps = alloca i64\n  %arith = alloca i64");
    e.store(0, "%steps");
    e.store(0, "%arith");
    for r in 2..f.registers {
        e.line(format!("  %nr{r} = alloca i64\n  %nt{r} = alloca i64"));
        e.set(r, 0, 0);
    }
    let accepted = e.callback(0, 0);
    e.line(format!(
        "  switch i32 {accepted}, label %error [ i32 0, label %b0\n    i32 1, label %numeric_init ]\nnumeric_init:"
    ));
    for (_, r) in &f.parameters {
        let v = e.packet_load(2 + 2 * r);
        e.set(*r, v, 1);
    }
    e.line("  br label %n0");
    for (pc, op) in f.code.iter().enumerate() {
        e.line(format!(
            "n{pc}:\n  ; scalar source bytes {}..{}",
            op.span.start, op.span.end
        ));
        // Poll regularly even for loops whose backedge is a conditional branch.
        if pc % 128 == 0
            || f.code
                .iter()
                .enumerate()
                .any(|(from, op)| match op.instruction {
                    Instruction::Jump { target } => target == pc && pc <= from,
                    Instruction::Branch { yes, no, .. } => (yes == pc || no == pc) && pc <= from,
                    _ => false,
                })
        {
            let steps = e.load("%steps");
            let due = e.temp(format!("icmp uge i64 {steps}, 1024"));
            e.line(format!(
                "  br i1 {due}, label %poll{pc}, label %body{pc}\npoll{pc}:"
            ));
            e.flush();
            let status = e.callback(2, pc);
            let ok = e.temp(format!("icmp eq i32 {status}, 0"));
            e.line(format!(
                "  br i1 {ok}, label %body{pc}, label %error\nbody{pc}:"
            ));
        }
        let mut arith = false;
        match &op.instruction {
            Instruction::Const { dst, value } => match value {
                Constant::Int(v) => e.set(*dst, v, 1),
                Constant::Bool(v) => e.set(*dst, i64::from(*v), 2),
                Constant::Unit => e.set(*dst, 0, 3),
                _ => unreachable!(),
            },
            Instruction::Copy { dst, src } | Instruction::NewSlot { dst, src } => {
                e.copy(*dst, *src)
            }
            Instruction::Load { dst, slot } => e.copy(*dst, *slot),
            Instruction::Store { slot, src } => e.copy(*slot, *src),
            Instruction::BoolCheck { src } => e.kind_guard(*src, 2, pc),
            Instruction::Unary { dst, op, src } => {
                e.kind_guard(*src, if op == "!" { 2 } else { 1 }, pc);
                let v = e.load(&format!("%nr{src}"));
                if op == "!" {
                    let v = e.temp(format!("xor i64 {v}, 1"));
                    e.set(*dst, v, 2);
                } else {
                    let ok = e.temp(format!("icmp ne i64 {v}, -9223372036854775808"));
                    e.guard(&ok, pc);
                    let v = e.temp(format!("sub i64 0, {v}"));
                    e.set(*dst, v, 1);
                }
            }
            Instruction::Binary {
                dst,
                op,
                left,
                right,
            } => {
                let a = e.load(&format!("%nr{left}"));
                let b = e.load(&format!("%nr{right}"));
                if matches!(op.as_str(), "==" | "!=") {
                    let ta = e.load(&format!("%nt{left}"));
                    let tb = e.load(&format!("%nt{right}"));
                    let tags = e.temp(format!("icmp eq i64 {ta}, {tb}"));
                    let vals = e.temp(format!("icmp eq i64 {a}, {b}"));
                    let mut v = e.temp(format!("and i1 {tags}, {vals}"));
                    if op == "!=" {
                        v = e.temp(format!("xor i1 {v}, true"));
                    }
                    let v = e.temp(format!("zext i1 {v} to i64"));
                    e.set(*dst, v, 2);
                } else {
                    e.kind_guard(*left, 1, pc);
                    e.kind_guard(*right, 1, pc);
                    match op.as_str() {
                        "+" | "-" | "*" => {
                            let intrinsic = match op.as_str() {
                                "+" => "sadd",
                                "-" => "ssub",
                                _ => "smul",
                            };
                            let pair = e.temp(format!("call {{i64, i1}} @llvm.{intrinsic}.with.overflow.i64(i64 {a}, i64 {b})"));
                            let overflow = e.temp(format!("extractvalue {{i64, i1}} {pair}, 1"));
                            let ok = e.temp(format!("xor i1 {overflow}, true"));
                            e.guard(&ok, pc);
                            let v = e.temp(format!("extractvalue {{i64, i1}} {pair}, 0"));
                            e.set(*dst, v, 1);
                            arith = true;
                        }
                        "/" | "%" => {
                            let nz = e.temp(format!("icmp ne i64 {b}, 0"));
                            e.guard(&nz, pc);
                            let min = e.temp(format!("icmp eq i64 {a}, -9223372036854775808"));
                            let neg = e.temp(format!("icmp eq i64 {b}, -1"));
                            let bad = e.temp(format!("and i1 {min}, {neg}"));
                            let ok = e.temp(format!("xor i1 {bad}, true"));
                            e.guard(&ok, pc);
                            let opcode = if op == "/" { "sdiv" } else { "srem" };
                            let v = e.temp(format!("{opcode} i64 {a}, {b}"));
                            e.set(*dst, v, 1);
                            arith = true;
                        }
                        _ => {
                            let pred = match op.as_str() {
                                "<" => "slt",
                                "<=" => "sle",
                                ">" => "sgt",
                                ">=" => "sge",
                                _ => unreachable!(),
                            };
                            let v = e.temp(format!("icmp {pred} i64 {a}, {b}"));
                            let v = e.temp(format!("zext i1 {v} to i64"));
                            e.set(*dst, v, 2);
                        }
                    }
                }
            }
            Instruction::Jump { target } => {
                e.line(format!("  br label %n{target}"));
                continue;
            }
            Instruction::Branch { cond, yes, no } => {
                e.kind_guard(*cond, 2, pc);
                e.bump("%steps");
                let v = e.load(&format!("%nr{cond}"));
                let b = e.temp(format!("icmp ne i64 {v}, 0"));
                e.line(format!("  br i1 {b}, label %n{yes}, label %n{no}"));
                continue;
            }
            Instruction::Return { src } => {
                e.bump("%steps");
                e.flush();
                let v = e.load(&format!("%nr{src}"));
                let t = e.load(&format!("%nt{src}"));
                e.packet_store(2, &v);
                e.packet_store(3, &t);
                let status = e.callback(3, pc);
                e.line(format!("  ret i32 {status}"));
                continue;
            }
            _ => unreachable!(),
        }
        e.bump("%steps");
        if arith {
            e.bump("%arith");
        }
        e.line(format!("  br label %n{}", pc + 1));
    }
    for (pc, op) in f.code.iter().enumerate() {
        if matches!(
            op.instruction,
            Instruction::Unary { .. }
                | Instruction::Binary { .. }
                | Instruction::Branch { .. }
                | Instruction::BoolCheck { .. }
        ) {
            e.line(format!("deopt{pc}:"));
            e.flush();
            for r in f.live_in[pc].iter().copied().filter(|r| *r >= 2) {
                let v = e.load(&format!("%nr{r}"));
                let t = e.load(&format!("%nt{r}"));
                e.packet_store(2 + 2 * r, &v);
                e.packet_store(3 + 2 * r, &t);
            }
            let status = e.callback(1, pc);
            e.line(format!("  ret i32 {status}"));
        }
    }
    e.out
}
