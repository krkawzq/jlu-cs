use crate::ir::{Function, Instruction, Reg};

pub(crate) fn analyze(function: &Function) -> Vec<Vec<Reg>> {
    let count = function.code.len();
    let words = function.registers.div_ceil(64);
    let mut live = vec![vec![0u64; words]; count];
    let mut predecessors = vec![vec![]; count];
    let mut edges = vec![vec![]; count];
    let mut operands = Vec::with_capacity(count);
    for (pc, op) in function.code.iter().enumerate() {
        let next = match op.instruction {
            Instruction::Jump { target } => vec![target],
            Instruction::Branch { yes, no, .. } => vec![yes, no],
            Instruction::Return { .. } => vec![],
            _ => {
                if pc + 1 < count {
                    vec![pc + 1]
                } else {
                    vec![]
                }
            }
        };
        for &target in &next {
            predecessors[target].push(pc);
        }
        edges[pc] = next;
        operands.push(uses_and_definition(&op.instruction));
    }
    let mut pending = (0..count).collect::<Vec<_>>();
    let mut queued = vec![true; count];
    while let Some(pc) = pending.pop() {
        queued[pc] = false;
        let mut incoming = vec![0u64; words];
        for &successor in &edges[pc] {
            for (out, input) in incoming.iter_mut().zip(&live[successor]) {
                *out |= *input;
            }
        }
        let (uses, definition) = &operands[pc];
        if let Some(reg) = definition {
            incoming[reg / 64] &= !(1u64 << (reg % 64));
        }
        for &reg in uses {
            incoming[reg / 64] |= 1u64 << (reg % 64);
        }
        if incoming != live[pc] {
            live[pc] = incoming;
            for &predecessor in &predecessors[pc] {
                if !queued[predecessor] {
                    queued[predecessor] = true;
                    pending.push(predecessor);
                }
            }
        }
    }
    live.into_iter()
        .map(|bits| {
            (0..function.registers)
                .filter(|r| bits[r / 64] & (1u64 << (r % 64)) != 0)
                .collect()
        })
        .collect()
}
fn uses_and_definition(op: &Instruction) -> (Vec<Reg>, Option<Reg>) {
    match op {
        Instruction::Const { dst, .. } | Instruction::SelfValue { dst } => (vec![], Some(*dst)),
        Instruction::Copy { dst, src }
        | Instruction::NewSlot { dst, src }
        | Instruction::Unary { dst, src, .. } => (vec![*src], Some(*dst)),
        Instruction::Load { dst, slot } => (vec![*slot], Some(*dst)),
        Instruction::Store { slot, src } => (vec![*slot, *src], None),
        Instruction::Field { dst, value, .. } => (vec![*value], Some(*dst)),
        Instruction::Binary {
            dst, left, right, ..
        } => (vec![*left, *right], Some(*dst)),
        Instruction::BoolCheck { src }
        | Instruction::CheckCallable { src }
        | Instruction::Return { src } => (vec![*src], None),
        Instruction::MakeData { dst, fields } => {
            (fields.iter().map(|(_, r)| *r).collect(), Some(*dst))
        }
        Instruction::MakeClosure { dst, captures, .. } => {
            (captures.iter().map(|(r, _)| *r).collect(), Some(*dst))
        }
        Instruction::CheckCall { callee, .. } => (vec![*callee], None),
        Instruction::Call { dst, callee, args } => {
            let mut uses = vec![*callee];
            uses.extend(args);
            (uses, Some(*dst))
        }
        Instruction::Jump { .. } => (vec![], None),
        Instruction::Branch { cond, .. } => (vec![*cond], None),
    }
}
