// prim-rs read-only standard library.
let std: @{
    // Write a value followed by a newline.
    let print: $[](value: any) -> unit;
    // Write a value without a trailing newline.
    let write: $[](value: any) -> unit;
    // Flush standard output.
    let flush: $[]() -> unit;
    // Read one line; return unit when input ends.
    let read_line: $[]() -> str | unit;
    // Write a prompt, then read one line; return unit when input ends.
    let input: $[](prompt: str) -> str | unit;
    // Write a value followed by a newline to standard error.
    let eprint: $[](value: any) -> unit;
    // Write a value to standard error without a trailing newline.
    let ewrite: $[](value: any) -> unit;
    // Flush standard error.
    let eflush: $[]() -> unit;
    // Read standard input to EOF; return an empty string when input is empty.
    let read_all: $[]() -> str;

    // Check a bool condition; terminate the program when false.
    let assert: $[](condition: bool) -> unit;
    // Check value equality; terminate with both values when they differ.
    let assert_eq: $[](actual: any, expected: any) -> unit;
    // Terminate the program with a user error.
    let fail: $[](message: str) -> unit;
    // Return the runtime type name of a value.
    let type: $[](value: any) -> str;
    // Convert a value to a string.
    let str: $[](value: any) -> str;
    // Convert an int or float to an int.
    let int: $[](value: Number) -> int;
    // Convert an int or float to a float.
    let float: $[](value: Number) -> float;
    // Parse a string as an int; return unit on failure.
    let parse_int: $[](text: str) -> int | unit;
    // Parse a string as a finite float; return unit on failure.
    let parse_float: $[](text: str) -> float | unit;

    // Return the number of Unicode scalars in a string or fields in a closure.
    let len: $[](value: str | closure) -> int;
    // Create a fixed-length data closure with non-negative integer field names.
    let array: $[](length: int, initial: any) -> closure;
    // Read an existing field by string or non-negative integer key.
    let get: $[](object: closure, key: int | str) -> any;
    // Update an existing field without adding fields; read-only fields raise an error.
    let set: $[](object: closure, key: int | str, value: any) -> unit;
    // Check whether an object contains a field.
    let has: $[](object: closure, key: int | str) -> bool;

    // prim-rs standard library module: gc.
    let gc: @{
        // Trigger a garbage collection.
        let collect: $[]() -> unit;
        // Return slot, closure, string, and collection statistics after a collection.
        let stats: $[]() -> GCStats;
    };

    // prim-rs standard library module: time.
    let time: @{
        // Return milliseconds elapsed since the runtime started.
        let now_ms: $[]() -> int;
        // Return the current Unix timestamp in milliseconds.
        let unix_ms: $[]() -> int;
        // Pause for 0 to 86400000 milliseconds.
        let sleep: $[](milliseconds: int) -> unit;
        // Format a Unix timestamp as UTC "YYYY-MM-DD HH:MM:SS".
        let format_utc: $[](unix_ms: int) -> str;
        // Split a Unix timestamp into UTC calendar fields.
        let parts: $[](unix_ms: int) -> TimeParts;
    };

    // prim-rs standard library module: random.
    let random: @{
        // Set the random number seed.
        let seed: $[](seed: int) -> unit;
        // Return a random integer in [lo, hi).
        let int: $[](lo: int, hi: int) -> int;
        // Return a random float in [0, 1).
        let float: $[]() -> float;
        // Return true or false with equal probability.
        let bool: $[]() -> bool;
        // Return a random float in [lo, hi).
        let float_range: $[](lo: float, hi: float) -> float;
        // Return a uniformly chosen field from a non-empty closure.
        let choice: $[](items: closure) -> any;
        // Shuffle closure fields in place.
        let shuffle: $[](items: closure) -> unit;
        // Return an array of count random ints in 0..255.
        let bytes: $[](count: int) -> closure;
        // Return a standard normal random float.
        let normal: $[]() -> float;
    };

    // prim-rs standard library module: text.
    let text: @{
        // Read one character by Unicode scalar index.
        let at: $[](text: str, index: int) -> str;
        // Slice [start, end) by Unicode scalar index.
        let slice: $[](text: str, start: int, end: int) -> str;
        // Repeat a string.
        let repeat: $[](text: str, count: int) -> str;
        // Remove leading and trailing whitespace.
        let trim: $[](text: str) -> str;
        // Check whether the string contains a substring.
        let contains: $[](text: str, pattern: str) -> bool;
        // Check whether the string starts with a prefix.
        let starts_with: $[](text: str, prefix: str) -> bool;
        // Check whether the string ends with a suffix.
        let ends_with: $[](text: str, suffix: str) -> bool;
        // Replace every matching substring.
        let replace: $[](text: str, from: str, to: str) -> str;
        // Return the first Unicode scalar index of a substring, or unit when absent.
        let index_of: $[](text: str, needle: str) -> int | unit;
        // Return the last Unicode scalar index of a substring, or unit when absent.
        let last_index_of: $[](text: str, needle: str) -> int | unit;
        // Split by a separator; an empty separator splits into Unicode scalars.
        let split: $[](text: str, separator: str) -> closure;
        // Split into lines on LF or CRLF; a trailing newline adds no empty line.
        let lines: $[](text: str) -> closure;
        // Join string fields with a separator.
        let join: $[](parts: closure, separator: str) -> str;
        // Convert to uppercase using Unicode mappings.
        let upper: $[](text: str) -> str;
        // Convert to lowercase using Unicode mappings.
        let lower: $[](text: str) -> str;
        // Pad on the left with one Unicode scalar until the scalar width is reached.
        let pad_start: $[](text: str, width: int, fill: str) -> str;
        // Pad on the right with one Unicode scalar until the scalar width is reached.
        let pad_end: $[](text: str, width: int, fill: str) -> str;
        // Return the Unicode scalar value at an index.
        let code_at: $[](text: str, index: int) -> int;
        // Build a one-character string from a Unicode scalar value.
        let from_code: $[](code: int) -> str;
        // Format a float with a fixed number of decimal digits.
        let fixed: $[](value: float, digits: int) -> str;
        // Parse an int in base 2..36; return unit on failure.
        let parse_int_radix: $[](text: str, base: int) -> int | unit;
    };

    // prim-rs standard library module: math.
    let math: @{
        // Return the absolute value of an int or float, preserving its kind.
        let abs: $[](value: Number) -> Number;
        // Return the smaller of two numbers of the same kind.
        let min: $[](a: Number, b: Number) -> Number;
        // Return the larger of two numbers of the same kind.
        let max: $[](a: Number, b: Number) -> Number;
        // Clamp a number to [lo, hi], preserving its kind.
        let clamp: $[](value: Number, lo: Number, hi: Number) -> Number;
        // Round a float down; return a float.
        let floor: $[](value: float) -> float;
        // Round a float up; return a float.
        let ceil: $[](value: float) -> float;
        // Round a float to the nearest integer; return a float.
        let round: $[](value: float) -> float;
        // Return the square root of a float.
        let sqrt: $[](value: float) -> float;
        // Raise a float to a float power.
        let pow: $[](base: float, exponent: float) -> float;
        // Return the sine of a float.
        let sin: $[](value: float) -> float;
        // Return the cosine of a float.
        let cos: $[](value: float) -> float;
        // Return the tangent of a float.
        let tan: $[](value: float) -> float;
        // Return the arc sine of a float.
        let asin: $[](value: float) -> float;
        // Return the arc cosine of a float.
        let acos: $[](value: float) -> float;
        // Return the arc tangent of a float.
        let atan: $[](value: float) -> float;
        // Return the arc tangent of y / x using both signs.
        let atan2: $[](y: float, x: float) -> float;
        // Return e raised to a float.
        let exp: $[](value: float) -> float;
        // Return the natural logarithm of a float.
        let ln: $[](value: float) -> float;
        // Return the logarithm of a value in a given base.
        let log: $[](value: float, base: float) -> float;
        // Return the base-2 logarithm of a float.
        let log2: $[](value: float) -> float;
        // Return the base-10 logarithm of a float.
        let log10: $[](value: float) -> float;
        // Round a float toward zero; return a float.
        let trunc: $[](value: float) -> float;
        // Return sqrt(x*x + y*y) without intermediate overflow.
        let hypot: $[](x: float, y: float) -> float;
        // Return the ratio of a circle's circumference to its diameter.
        let pi: $[]() -> float;
        // Return the sign of a number: -1, 0, or 1.
        let sign: $[](value: Number) -> int;
    };

    // prim-rs standard library module: bits.
    let bits: @{
        // Bitwise AND.
        let and: $[](a: int, b: int) -> int;
        // Bitwise OR.
        let or: $[](a: int, b: int) -> int;
        // Bitwise XOR.
        let xor: $[](a: int, b: int) -> int;
        // Bitwise complement.
        let not: $[](value: int) -> int;
        // Shift left by 0..63 bits, discarding high bits.
        let shl: $[](value: int, bits: int) -> int;
        // Shift right by 0..63 bits, preserving the sign.
        let shr: $[](value: int, bits: int) -> int;
        // Count set bits.
        let count_ones: $[](value: int) -> int;
        // Count leading zero bits in the 64-bit representation.
        let leading_zeros: $[](value: int) -> int;
        // Count trailing zero bits in the 64-bit representation.
        let trailing_zeros: $[](value: int) -> int;
    };

    // prim-rs standard library module: seq.
    let seq: @{
        // Copy fields [start, end) into a new array.
        let slice: $[](items: closure, start: int, end: int) -> closure;
        // Reverse fields in place.
        let reverse: $[](items: closure) -> unit;
        // Sort homogeneous int, float, or str fields in place.
        let sort: $[](items: closure) -> unit;
        // Set every field to a value.
        let fill: $[](items: closure, value: any) -> unit;
        // Return the first position equal to a value, or unit when absent.
        let index_of: $[](items: closure, value: any) -> int | unit;
        // Copy fields into a new array with independent slots.
        let copy: $[](items: closure) -> closure;
        // Concatenate two closures into a new array.
        let concat: $[](left: closure, right: closure) -> closure;
    };

    // prim-rs standard library module: env.
    let env: @{
        // Return the number of script arguments.
        let argc: $[]() -> int;
        // Return a script argument as a string; return unit when the index is out of range.
        let arg: $[](index: int) -> str | unit;
        // Read an environment variable; return unit when it is missing.
        let get: $[](name: str) -> str | unit;
        // Return the current working directory.
        let cwd: $[]() -> str;
        // Return all script arguments as an array.
        let args: $[]() -> closure;
        // Return the home directory; return unit when it is unavailable.
        let home: $[]() -> str | unit;
        // Return the platform temporary directory.
        let temp_dir: $[]() -> str;
    };

    // prim-rs standard library module: process.
    let process: @{
        // Return the current process id.
        let pid: $[]() -> int;
        // Exit immediately with a status in 0..255.
        let exit: $[](code: int) -> unit;
    };

    // prim-rs standard library module: fs.
    let fs: @{
        // Read a UTF-8 text file.
        let read_text: $[](path: str) -> str;
        // Create or overwrite a text file.
        let write_text: $[](path: str, text: str) -> unit;
        // Create or append to a text file.
        let append_text: $[](path: str, text: str) -> unit;
        // Check whether a path exists.
        let exists: $[](path: str) -> bool;
        // Return sorted entry names in a directory.
        let list_dir: $[](path: str) -> closure;
        // Create a directory and missing parents.
        let mkdir_all: $[](path: str) -> unit;
        // Remove a file.
        let remove_file: $[](path: str) -> unit;
        // Remove an empty directory.
        let remove_dir: $[](path: str) -> unit;
        // Rename or move a file or directory.
        let rename: $[](from: str, to: str) -> unit;
        // Copy a file, replacing the destination.
        let copy_file: $[](from: str, to: str) -> unit;
        // Return a file size in bytes.
        let file_size: $[](path: str) -> int;
        // Check whether a path is a regular file.
        let is_file: $[](path: str) -> bool;
        // Check whether a path is a directory.
        let is_dir: $[](path: str) -> bool;
    };

    // prim-rs standard library module: term.
    let term: @{
        // Enter interactive terminal mode.
        let enter: $[]() -> unit;
        // Restore the terminal mode.
        let leave: $[]() -> unit;
        // Check whether stdin and stdout are terminals.
        let is_tty: $[]() -> bool;
        // Return a data closure containing width and height.
        let size: $[]() -> TerminalSize;
        // Clear the screen and move the cursor to the top-left corner.
        let clear: $[]() -> unit;
        // Move the cursor to the top-left corner.
        let home: $[]() -> unit;
        // Move the cursor to zero-based coordinates.
        let move: $[](x: int, y: int) -> unit;
        // Show or hide the cursor.
        let cursor: $[](visible: bool) -> unit;
        // Set colors to -1 (default) or 0 through 15.
        let color: $[](foreground: int, background: int) -> unit;
        // Reset terminal colors and styles.
        let reset: $[]() -> unit;
        // Wait for a key and return a string; return unit on timeout.
        let poll_key: $[](timeout_ms: int) -> str | unit;
        // Set 24-bit colors; each channel is packed as 0xRRGGBB or -1 for default.
        let color_rgb: $[](foreground: int, background: int) -> unit;
        // Ring the terminal bell.
        let bell: $[]() -> unit;
        // Set the terminal window title.
        let title: $[](text: str) -> unit;
    };

    // prim-rs standard library module: sys.
    let sys: @{
        // Return the target operating system name.
        let os: $[]() -> str;
        // Return the target CPU architecture name.
        let arch: $[]() -> str;
    };

    // Syntax parsing is side-effect free and does not perform name resolution.
    let ast: @{
        // Return readonly tokens with decoded values and source spans, including EOF.
        let tokens: $[](source: str) -> closure;
        // Parse source into a readonly ASTNode tree; never execute it.
        let parse: $[](source: str) -> ASTNode;
        // Format a tree as an indented human-readable dump.
        let dump: $[](tree: ASTNode) -> str;
        // Return nodes in depth-first preorder.
        let walk: $[](tree: ASTNode) -> closure;
        // Return nodes matching the exact kind name.
        let find: $[](tree: ASTNode, kind: str) -> closure;
        // Extract a node's byte range from its original source string.
        let source_segment: $[](source: str, node: ASTNode) -> str;
        // Check the shallow AST record shape.
        let is_node: $[](value: any) -> bool;
    };

    // Function, closure, stack and native-code inspection. Metadata is readonly.
    let inspect: @{
        let is_callable: $[](value: any) -> bool;
        let is_function: $[](value: any) -> bool;
        let is_builtin: $[](value: any) -> bool;
        // Tests the explicit policy, even when --jit=off is active.
        let is_jit: $[](value: any) -> bool;
        let describe: $[](value: any) -> InspectDescription;
        let members: $[](object: closure) -> closure;
        let signature: $[](function: callable) -> InspectSignature;
        let captures: $[](function: callable) -> closure;
        // Returns the original callable literal, without its outer decorator.
        let source: $[](object: closure) -> str | unit;
        let location: $[](object: closure) -> SourceLocation | unit;
        // AST spans are relative to inspect.source(function).
        let ast: $[](function: callable) -> ASTNode;
        let core_ir: $[](function: callable) -> str;
        let stack: $[]() -> closure;
        let builtins: $[]() -> closure;
        // Query cache state and per-CodeId counters without triggering compilation.
        let jit: $[](function: callable) -> JitInfo;
        // Compile an explicitly JIT-marked closure without running its body.
        let compile: $[](function: callable) -> callable;
        // The following methods require a compiled CodeId; queries never compile implicitly.
        let llvm_ir: $[](function: callable) -> str;
        let optimized_ir: $[](function: callable) -> str;
        let machine_code: $[](function: callable) -> str;
        let disassemble: $[](function: callable) -> str;
        let native: $[](function: callable) -> NativeCode;
    };
};
