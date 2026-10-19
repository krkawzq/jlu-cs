use crate::{
    ir::{Function, Instruction},
    runtime::Runtime,
    syntax::{Error, Span},
};
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::Path,
    ptr,
    sync::{Mutex, Once},
    time::Instant,
};

pub type Callback = unsafe extern "C" fn(*mut Runtime, usize, usize) -> i32;
pub type IntegerOperands =
    unsafe extern "C" fn(*mut Runtime, usize, usize, *mut i64, *mut i64) -> i32;
pub type IntegerResult = unsafe extern "C" fn(*mut Runtime, usize, usize, i64) -> i32;
pub type NumericCallback = unsafe extern "C" fn(*mut Runtime, usize, usize, usize, *mut i64) -> i32;
pub type NativeEntry = unsafe extern "C" fn(
    *mut Runtime,
    usize,
    Callback,
    Callback,
    IntegerOperands,
    IntegerResult,
    NumericCallback,
) -> i32;
type Handle = *mut c_void;
#[path = "native.rs"]
mod native;
#[path = "numeric.rs"]
pub(crate) mod numeric;
pub struct CompiledCode {
    pub entry: NativeEntry,
    pub ir: String,
    pub optimized_ir: String,
    pub opt_level: u8,
    pub scalar_lowering: bool,
    pub triple: String,
    pub data_layout: String,
    pub symbol: String,
    pub address: usize,
    pub machine: Result<native::MachineCode, String>,
    pub compile_ms: f64,
}

unsafe extern "C" {
    fn LLVMInitializeX86TargetInfo();
    fn LLVMInitializeX86Target();
    fn LLVMInitializeX86TargetMC();
    fn LLVMInitializeX86AsmPrinter();
    fn LLVMInitializeX86Disassembler();
    fn LLVMOrcLLJITGetObjTransformLayer(jit: Handle) -> Handle;
    fn LLVMOrcObjectTransformLayerSetTransform(
        layer: Handle,
        callback: unsafe extern "C" fn(Handle, *mut Handle) -> Handle,
        context: Handle,
    );
    fn LLVMOrcCreateLLJIT(result: *mut Handle, builder: Handle) -> Handle;
    fn LLVMOrcDisposeLLJIT(jit: Handle) -> Handle;
    fn LLVMOrcLLJITGetMainJITDylib(jit: Handle) -> Handle;
    fn LLVMOrcLLJITGetTripleString(jit: Handle) -> *const c_char;
    fn LLVMOrcLLJITGetDataLayoutStr(jit: Handle) -> *const c_char;
    fn LLVMOrcCreateNewThreadSafeContext() -> Handle;
    fn LLVMOrcThreadSafeContextGetContext(context: Handle) -> Handle;
    fn LLVMOrcDisposeThreadSafeContext(context: Handle);
    fn LLVMCreateMemoryBufferWithMemoryRangeCopy(
        data: *const c_char,
        len: usize,
        name: *const c_char,
    ) -> Handle;
    fn LLVMParseIRInContext(
        context: Handle,
        buffer: Handle,
        module: *mut Handle,
        message: *mut *mut c_char,
    ) -> c_int;
    fn LLVMVerifyModule(module: Handle, action: c_int, message: *mut *mut c_char) -> c_int;
    fn LLVMDisposeModule(module: Handle);
    fn LLVMDisposeMessage(message: *mut c_char);
    fn LLVMPrintModuleToString(module: Handle) -> *mut c_char;
    fn LLVMCreatePassBuilderOptions() -> Handle;
    fn LLVMDisposePassBuilderOptions(options: Handle);
    fn LLVMRunPasses(module: Handle, passes: *const c_char, machine: Handle, options: Handle) -> Handle;
    fn LLVMOrcCreateNewThreadSafeModule(module: Handle, context: Handle) -> Handle;
    fn LLVMOrcJITDylibCreateResourceTracker(dylib: Handle) -> Handle;
    fn LLVMOrcLLJITAddLLVMIRModuleWithRT(jit: Handle, tracker: Handle, module: Handle) -> Handle;
    fn LLVMOrcResourceTrackerRemove(tracker: Handle) -> Handle;
    fn LLVMOrcReleaseResourceTracker(tracker: Handle);
    fn LLVMOrcLLJITLookup(jit: Handle, result: *mut u64, name: *const c_char) -> Handle;
    fn LLVMGetErrorMessage(error: Handle) -> *mut c_char;
    fn LLVMDisposeErrorMessage(message: *mut c_char);
}
static INIT: Once = Once::new();

pub struct Jit {
    engine: Handle,
    entries: HashMap<usize, std::result::Result<CompiledCode, String>>,
    capture: Box<Mutex<native::Capture>>,
}
impl Default for Jit {
    fn default() -> Self {
        Self::new()
    }
}
impl Jit {
    pub fn new() -> Self {
        Self {
            engine: ptr::null_mut(),
            entries: HashMap::new(),
            capture: Box::new(Mutex::new(native::Capture::default())),
        }
    }
    fn initialize(&mut self) -> std::result::Result<(), String> {
        if !self.engine.is_null() {
            return Ok(());
        }
        INIT.call_once(|| unsafe {
            LLVMInitializeX86TargetInfo();
            LLVMInitializeX86Target();
            LLVMInitializeX86TargetMC();
            LLVMInitializeX86AsmPrinter();
            LLVMInitializeX86Disassembler();
        });
        // LLJIT owns machine code until this backend is dropped.
        unsafe {
            check(LLVMOrcCreateLLJIT(&mut self.engine, ptr::null_mut()))?;
            LLVMOrcObjectTransformLayerSetTransform(
                LLVMOrcLLJITGetObjTransformLayer(self.engine),
                native::capture_object,
                (&*self.capture as *const Mutex<native::Capture>)
                    .cast_mut()
                    .cast(),
            );
        }
        Ok(())
    }
    pub fn compile(
        &mut self,
        id: usize,
        function: &Function,
        ir_dir: Option<&Path>,
        opt_level: u8,
    ) -> std::result::Result<(NativeEntry, bool), String> {
        if let Some(entry) = self.entries.get(&id) {
            return entry
                .as_ref()
                .map(|e| (e.entry, false))
                .map_err(Clone::clone);
        }
        let result = self.compile_new(id, function, ir_dir, opt_level);
        let outcome = result
            .as_ref()
            .map(|e| (e.entry, true))
            .map_err(Clone::clone);
        self.entries.insert(id, result);
        outcome
    }
    pub fn artifact(&self, id: usize) -> Option<&Result<CompiledCode, String>> {
        self.entries.get(&id)
    }
    fn compile_new(
        &mut self,
        id: usize,
        function: &Function,
        ir_dir: Option<&Path>,
        opt_level: u8,
    ) -> std::result::Result<CompiledCode, String> {
        let started = Instant::now();
        self.initialize()?;
        let (triple, layout) = unsafe {
            (
                CStr::from_ptr(LLVMOrcLLJITGetTripleString(self.engine))
                    .to_string_lossy()
                    .into_owned(),
                CStr::from_ptr(LLVMOrcLLJITGetDataLayoutStr(self.engine))
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        if opt_level > 2 { return Err("optimization level must be 0, 1 or 2".into()); }
        let scalar_lowering = opt_level >= 1 && numeric::eligible(function);
        let ir = generate_ir_level(id, function, &triple, &layout, scalar_lowering);
        if let Some(dir) = ir_dir {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join(format!("function_{id}.ll")), &ir)
                .map_err(|e| e.to_string())?;
        }
        let symbol = format!("prim_function_{id}");
        let (entry, optimized_ir) = self.load_ir_optimized(&ir, &symbol, opt_level)?;
        if let Some(dir) = ir_dir {
            std::fs::write(dir.join(format!("function_{id}.opt.ll")), &optimized_ir)
                .map_err(|e| e.to_string())?;
        }
        let address = entry as usize;
        let machine = match self.capture.lock() {
            Ok(capture) => match capture.sizes.get(&symbol) {
                Some(size) => native::copy_loaded(address, *size),
                None => Err(capture
                    .error
                    .clone()
                    .unwrap_or_else(|| "function symbol size was not emitted".into())),
            },
            Err(_) => Err("native metadata lock poisoned".into()),
        };
        Ok(CompiledCode {
            entry,
            ir,
            optimized_ir,
            opt_level,
            scalar_lowering,
            triple,
            data_layout: layout,
            symbol,
            address,
            machine,
            compile_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    }
    #[cfg(test)]
    fn load_ir(&mut self, ir: &str, symbol: &str) -> std::result::Result<NativeEntry, String> {
        self.load_ir_optimized(ir, symbol, 0).map(|(entry, _)| entry)
    }
    fn load_ir_optimized(&mut self, ir: &str, symbol: &str, opt_level: u8) -> std::result::Result<(NativeEntry, String), String> {
        let name = CString::new(symbol).map_err(|_| "invalid native symbol")?;
        unsafe {
            let context = LLVMOrcCreateNewThreadSafeContext();
            let llvm_context = LLVMOrcThreadSafeContextGetContext(context);
            let buffer = LLVMCreateMemoryBufferWithMemoryRangeCopy(
                ir.as_ptr().cast(),
                ir.len(),
                name.as_ptr(),
            );
            let mut module = ptr::null_mut();
            let mut message = ptr::null_mut();
            if LLVMParseIRInContext(llvm_context, buffer, &mut module, &mut message) != 0 {
                let error = take_message(message);
                LLVMOrcDisposeThreadSafeContext(context);
                return Err(format!("LLVM IR parse: {error}"));
            }
            if LLVMVerifyModule(module, 2, &mut message) != 0 {
                let error = take_message(message);
                LLVMDisposeModule(module);
                LLVMOrcDisposeThreadSafeContext(context);
                return Err(format!("LLVM IR verification: {error}"));
            }
            if !message.is_null() {
                LLVMDisposeMessage(message);
            }
            if opt_level > 0 {
                let passes = CString::new(format!("default<O{opt_level}>" )).unwrap();
                let options = LLVMCreatePassBuilderOptions();
                let result = check(LLVMRunPasses(module, passes.as_ptr(), ptr::null_mut(), options));
                LLVMDisposePassBuilderOptions(options);
                if let Err(error) = result {
                    LLVMDisposeModule(module);
                    LLVMOrcDisposeThreadSafeContext(context);
                    return Err(format!("LLVM optimization: {error}"));
                }
                message = ptr::null_mut();
                if LLVMVerifyModule(module, 2, &mut message) != 0 {
                    let error = take_message(message);
                    LLVMDisposeModule(module);
                    LLVMOrcDisposeThreadSafeContext(context);
                    return Err(format!("optimized LLVM IR verification: {error}"));
                }
                if !message.is_null() { LLVMDisposeMessage(message); }
            }
            let optimized_ir = take_message(LLVMPrintModuleToString(module));
            let module = LLVMOrcCreateNewThreadSafeModule(module, context);
            LLVMOrcDisposeThreadSafeContext(context);
            let tracker =
                LLVMOrcJITDylibCreateResourceTracker(LLVMOrcLLJITGetMainJITDylib(self.engine));
            let mut address = 0u64;
            let loaded = check(LLVMOrcLLJITAddLLVMIRModuleWithRT(
                self.engine,
                tracker,
                module,
            ))
            .and_then(|_| check(LLVMOrcLLJITLookup(self.engine, &mut address, name.as_ptr())))
            .and_then(|_| {
                if address == 0 {
                    Err("LLJIT returned a null entry".into())
                } else {
                    Ok(())
                }
            });
            if let Err(mut error) = loaded {
                if let Err(cleanup) = check(LLVMOrcResourceTrackerRemove(tracker)) {
                    error.push_str(&format!("; resource cleanup: {cleanup}"));
                }
                LLVMOrcReleaseResourceTracker(tracker);
                return Err(error);
            }
            LLVMOrcReleaseResourceTracker(tracker);
            // The generated signature and this function pointer use the same C ABI.
            Ok((std::mem::transmute::<usize, NativeEntry>(address as usize), optimized_ir))
        }
    }
}
impl Drop for Jit {
    fn drop(&mut self) {
        if !self.engine.is_null() {
            unsafe {
                let _ = check(LLVMOrcDisposeLLJIT(self.engine));
            }
        }
    }
}
unsafe fn take_message(message: *mut c_char) -> String {
    if message.is_null() {
        return "unknown LLVM diagnostic".into();
    }
    let value = unsafe { CStr::from_ptr(message) }
        .to_string_lossy()
        .into_owned();
    unsafe { LLVMDisposeMessage(message) };
    value
}
unsafe fn check(error: Handle) -> std::result::Result<(), String> {
    if error.is_null() {
        return Ok(());
    }
    let message = unsafe { LLVMGetErrorMessage(error) };
    let text = unsafe { CStr::from_ptr(message) }
        .to_string_lossy()
        .into_owned();
    unsafe { LLVMDisposeErrorMessage(message) };
    Err(text)
}

pub fn generate_ir(id: usize, function: &Function, triple: &str, layout: &str) -> String {
    generate_ir_level(id, function, triple, layout, false)
}
fn generate_ir_level(id: usize, function: &Function, triple: &str, layout: &str, scalar: bool) -> String {
    let opaque = env!("PRIM_LLVM_MAJOR").parse::<u32>().unwrap() >= 15;
    let context_ty = if opaque { "ptr" } else { "i8*" };
    let callback_ty = if opaque {
        "ptr"
    } else {
        "i32 (i8*, i64, i64)*"
    };
    let integer_ptr = if opaque { "ptr" } else { "i64*" };
    let operands_ty = if opaque {
        "ptr"
    } else {
        "i32 (i8*, i64, i64, i64*, i64*)*"
    };
    let result_ty = if opaque {
        "ptr"
    } else {
        "i32 (i8*, i64, i64, i64)*"
    };
    let numeric_ty = if opaque { "ptr" } else { "i32 (i8*, i64, i64, i64, i64*)*" };
    let mut ir = format!(
        "; Prim explicit JIT: native control flow and checked integer arithmetic\ntarget triple = \"{triple}\"\ntarget datalayout = \"{layout}\"\ndeclare {{i64, i1}} @llvm.sadd.with.overflow.i64(i64, i64)\ndeclare {{i64, i1}} @llvm.ssub.with.overflow.i64(i64, i64)\ndeclare {{i64, i1}} @llvm.smul.with.overflow.i64(i64, i64)\ndefine i32 @prim_function_{id}({context_ty} %ctx, i64 %frame, {callback_ty} %step, {callback_ty} %test, {operands_ty} %integers, {result_ty} %save_integer, {numeric_ty} %numeric) {{\nentry:\n  %left = alloca i64\n  %right = alloca i64\n"
    );
    if scalar { ir.push_str(&numeric::emit(function, opaque)); } else { ir.push_str("  br label %b0\n"); }
    for (pc, op) in function.code.iter().enumerate() {
        ir.push_str(&format!(
            "b{pc}:\n  ; source bytes {}..{}\n",
            op.span.start, op.span.end
        ));
        match &op.instruction{
            Instruction::Jump{target}=>ir.push_str(&format!("  br label %b{target}\n")),
            Instruction::Branch{yes,no,..}=>{
                ir.push_str(&format!("  %c{pc} = call i32 %test({context_ty} %ctx, i64 %frame, i64 {pc})\n  switch i32 %c{pc}, label %error [ i32 0, label %b{no}\n    i32 1, label %b{yes} ]\n"));
            }
            Instruction::Return{..}=>ir.push_str(&format!("  %r{pc} = call i32 %step({context_ty} %ctx, i64 %frame, i64 {pc})\n  ret i32 %r{pc}\n")),
            Instruction::Binary{op,..} if matches!(op.as_str(),"+"|"-"|"*")=>{
                let intrinsic=match op.as_str(){"+"=>"sadd","-"=>"ssub",_=>"smul"};
                ir.push_str(&format!("  %probe{pc} = call i32 %integers({context_ty} %ctx, i64 %frame, i64 {pc}, {integer_ptr} %left, {integer_ptr} %right)\n  switch i32 %probe{pc}, label %error [ i32 0, label %slow{pc}\n    i32 1, label %fast{pc} ]\nfast{pc}:\n  %a{pc} = load i64, {integer_ptr} %left\n  %rhs{pc} = load i64, {integer_ptr} %right\n  %pair{pc} = call {{i64, i1}} @llvm.{intrinsic}.with.overflow.i64(i64 %a{pc}, i64 %rhs{pc})\n  %value{pc} = extractvalue {{i64, i1}} %pair{pc}, 0\n  %overflow{pc} = extractvalue {{i64, i1}} %pair{pc}, 1\n  br i1 %overflow{pc}, label %slow{pc}, label %save{pc}\nsave{pc}:\n  %saved{pc} = call i32 %save_integer({context_ty} %ctx, i64 %frame, i64 {pc}, i64 %value{pc})\n  %saved_ok{pc} = icmp eq i32 %saved{pc}, 0\n  br i1 %saved_ok{pc}, label %b{}, label %error\nslow{pc}:\n  %slow_result{pc} = call i32 %step({context_ty} %ctx, i64 %frame, i64 {pc})\n  %slow_ok{pc} = icmp eq i32 %slow_result{pc}, 0\n  br i1 %slow_ok{pc}, label %b{}, label %error\n",pc+1,pc+1));
            }
            _=>ir.push_str(&format!("  %s{pc} = call i32 %step({context_ty} %ctx, i64 %frame, i64 {pc})\n  %ok{pc} = icmp eq i32 %s{pc}, 0\n  br i1 %ok{pc}, label %b{}, label %error\n",pc+1)),
        }
    }
    ir.push_str("error:\n  ret i32 1\n}\n");
    ir
}

pub unsafe extern "C" fn step_callback(runtime: *mut Runtime, frame: usize, pc: usize) -> i32 {
    callback(runtime, |r| r.step(frame, pc).map(|_| 0))
}
pub unsafe extern "C" fn condition_callback(runtime: *mut Runtime, frame: usize, pc: usize) -> i32 {
    callback(runtime, |r| r.condition(frame, pc).map(i32::from))
}
pub unsafe extern "C" fn integer_operands_callback(
    runtime: *mut Runtime,
    frame: usize,
    pc: usize,
    left: *mut i64,
    right: *mut i64,
) -> i32 {
    callback(runtime, |r| {
        if let Some((a, b)) = r.integer_operands(frame, pc) {
            unsafe {
                *left = a;
                *right = b;
            }
            Ok(1)
        } else {
            Ok(0)
        }
    })
}
pub unsafe extern "C" fn integer_result_callback(
    runtime: *mut Runtime,
    frame: usize,
    pc: usize,
    value: i64,
) -> i32 {
    callback(runtime, |r| r.integer_result(frame, pc, value).map(|_| 0))
}
pub unsafe extern "C" fn numeric_callback(runtime: *mut Runtime, frame: usize, action: usize, pc: usize, data: *mut i64) -> i32 {
    callback(runtime, |r| unsafe { r.numeric_callback(frame, action, pc, data) })
}
fn callback(
    runtime: *mut Runtime,
    f: impl FnOnce(&mut Runtime) -> crate::syntax::Result<i32>,
) -> i32 {
    unsafe { (*runtime).stats.native_helper_calls += 1; }
    // Native code cannot unwind through the C boundary.
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(unsafe { &mut *runtime })));
    match result {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            unsafe { (*runtime).error = Some(error) };
            -1
        }
        Err(_) => {
            unsafe {
                (*runtime).error = Some(Error::new(
                    "InternalError",
                    "runtime callback panicked",
                    Span::default(),
                ))
            };
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_materialization_releases_symbol_resources() {
        let mut jit = Jit::new();
        jit.initialize().unwrap();
        let broken = "declare i32 @prim_intentionally_missing_symbol()\ndefine i32 @memory_probe() { %x = call i32 @prim_intentionally_missing_symbol() ret i32 %x }";
        assert!(jit.load_ir(broken, "memory_probe").is_err());
        let good = "define i32 @memory_probe() { ret i32 0 }";
        assert!(jit.load_ir(good, "memory_probe").is_ok());
    }
}
