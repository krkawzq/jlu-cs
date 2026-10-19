// Prim runtime value kinds. These declarations do not introduce runtime bindings.
type int;
type float;
type bool;
type str;
type unit;
type closure;

// A callable closure with an unspecified signature.
type callable;

// An unconstrained dynamic value.
type any;

// Numeric union accepted by conversion and math builtins.
type Number = int | float;

// Record returned by std.term.size().
type TerminalSize = @{
    let width: int;
    let height: int;
};

// UTC calendar fields returned by std.time.parts(); weekday 0 is Sunday.
type TimeParts = @{
    let year: int;
    let month: int;
    let day: int;
    let hour: int;
    let minute: int;
    let second: int;
    let millisecond: int;
    let weekday: int;
};

// Record returned by std.gc.stats(). Fields are writable runtime slots.
type GCStats = @{
    let slots: int;
    let closures: int;
    let slot_capacity: int;
    let closure_capacity: int;
    let collections: int;
    let output_bytes: int;
    let strings: int;
    let string_bytes: int;
};

// Read-only syntax and inspection records. Sequence fields use indexed closures.
type ASTSpan = @{
    let start: int;
    let end: int;
    let line: int;
    let column: int;
    let end_line: int;
    let end_column: int;
};
type ASTNode = @{
    let kind: str;
    let span: ASTSpan;
    let attributes: closure;
    let children: closure;
};
type ASTToken = @{
    let kind: str;
    let text: str;
    let value: str | unit;
    let span: ASTSpan;
};
type SourceLocation = @{
    let file: str;
    let start: int;
    let end: int;
    let line: int;
    let column: int;
    let end_line: int;
    let end_column: int;
};
type InspectDescription = @{
    let type: str;
    let kind: str;
    let display: str;
    let name: str | unit;
    let closure_id: int | unit;
    let code_id: int | unit;
    let arity: int | unit;
    let field_count: int;
    let callable: bool;
    let policy: str;
};
type InspectParameter = @{
    let name: str;
    let index: int;
    let by_ref: bool;
};
type InspectSignature = @{
    let name: str;
    let arity: int;
    let parameters: closure;
    let text: str;
};
type InspectCapture = @{
    let name: str;
    let mode: str;
    let slot_id: int;
    let value: any;
    let writable: bool;
};
type InspectMember = @{
    let name: str;
    let slot_id: int;
    let type: str;
    let value: any;
    let writable: bool;
};
type InspectFrame = @{
    let depth: int;
    let code_id: int;
    let closure_id: int | unit;
    let pc: int;
    let mode: str;
    let location: SourceLocation | unit;
};
type InspectBuiltin = @{
    let name: str;
    let arity: int;
};
type JitInfo = @{
    let code_id: int;
    let arity: int;
    let capture_count: int;
    let registers: int;
    let instruction_count: int;
    let policy: str;
    let enabled: bool;
    let state: str;
    let compiled: bool;
    let will_use_native: bool;
    let symbol: str;
    let address: str | unit;
    let code_size: int | unit;
    let triple: str | unit;
    let data_layout: str | unit;
    let ir_bytes: int | unit;
    let optimized_ir_bytes: int | unit;
    let opt_level: int;
    let lowering: str | unit;
    let scalar_calls: int;
    let scalar_fallbacks: int;
    let llvm_version: str;
    let compile_ms: float | unit;
    let native_calls: int;
    let interpreted_calls: int;
    let native_integer_ops: int;
    let error: str | unit;
    let metadata_error: str | unit;
};
type NativeCode = @{
    let address: str;
    let size: int;
    let hex: str;
    let assembly: str;
    let triple: str;
    let symbol: str;
    let origin: str;
};
