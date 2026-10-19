use super::*;

impl Runtime {
    /// The generated entry owns a packet of 2 + 2 * registers i64 words.
    /// Actions: 0 guard/arguments, 1 resume, 2 poll, 3 return.
    pub(crate) unsafe fn numeric_callback(
        &mut self,
        frame: usize,
        action: usize,
        pc: usize,
        data: *mut i64,
    ) -> Result<i32> {
        let code = self.frames[frame].function;
        let f = &self.program.functions[code];
        // Read only the words initialized by this action in the native entry.
        let read = |index: usize| unsafe { data.add(index).read() };
        if action == 0 {
            // Fuel requires instruction-exact checks, including the failing op.
            if self.config.max_steps.is_some() {
                return Ok(0);
            }
            for (_, reg) in &f.parameters {
                let Value::Int(v) = self.slot(self.slot_id(frame, *reg)).value else {
                    return Ok(0);
                };
                unsafe {
                    data.add(2 + 2 * reg).write(v);
                }
            }
            self.stats.scalar_calls += 1;
            self.function_stats.entry(code).or_default().scalar_calls += 1;
            return Ok(1);
        }
        self.stats.steps += read(0) as u64;
        self.stats.native_integer_ops += read(1) as u64;
        self.function_stats
            .entry(code)
            .or_default()
            .native_integer_ops += read(1) as u64;
        self.frames[frame].pc = pc;
        let span = f.code[pc].span;
        if self.terminal.interrupted() {
            return Err(Error::new("Interrupted", "terminal interrupted", span));
        }
        fn decode(value: i64, tag: i64) -> Option<Value> {
            match tag {
                1 => Some(Value::Int(value)),
                2 => Some(Value::Bool(value != 0)),
                3 => Some(Value::Unit),
                _ => None,
            }
        }
        match action {
            1 => {
                let mut slots = vec![false; f.registers];
                for (_, reg) in &f.parameters {
                    slots[*reg] = true;
                }
                for op in &f.code {
                    if let Instruction::NewSlot { dst, .. } = op.instruction {
                        slots[dst] = true;
                    }
                }
                // No references escape this function, so only live cells need materializing.
                let live = f.live_in[pc].clone();
                for reg in live.into_iter().filter(|r| *r >= 2) {
                    if let Some(value) = decode(read(2 + 2 * reg), read(3 + 2 * reg)) {
                        self.frames[frame].registers[reg] = if slots[reg] {
                            Cell::Slot(self.new_slot(value, true))
                        } else {
                            Cell::Value(value)
                        };
                    }
                }
                self.stats.scalar_fallbacks += 1;
                self.function_stats
                    .entry(code)
                    .or_default()
                    .scalar_fallbacks += 1;
                self.interpret_from(frame, pc)?;
            }
            2 => {}
            3 => {
                self.frames[frame].result = Some(decode(read(2), read(3)).ok_or_else(|| {
                    Error::new("InternalError", "invalid scalar return tag", span)
                })?);
            }
            _ => unreachable!(),
        }
        Ok(0)
    }
}
