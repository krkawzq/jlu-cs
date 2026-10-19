# Prim Rust 手动 JIT 与运行时

状态：已实现，Linux x86_64 / LLVM 14 CPU pod 验证。代码位于 [`src/prim-rs`](../src/prim-rs/README.md)。语言语义见 [language-spec.md](language-spec.md)。

## 1. 执行模型

```prim
let square = @jit $[](x) { x * x };
std.print(square(7));
```

`@jit` 是内置普通装饰器，等价于 `jit(closure)`。装饰只标记执行策略，第一次调用才编译。未装饰函数无论调用多少次都解释执行；没有热点检测、trace、OSR 或按实参类型产生多个特化版本。

`jit(f)` 返回与 `f` 共享捕获槽、具有新身份的可调用闭包，原闭包保持原执行策略。已经标记的闭包再次 `jit` 返回自身。数据闭包、基础值和宿主内置函数不是合法 JIT 目标。

调用流程：检查 callee 与参数模式 → 从左到右求实参 → 准备本机入口 → 绑定参数并建立 Frame → 执行函数体。编译失败时，实参已经发生的副作用保留，函数体不执行。默认无解释回退。

## 2. 动态库路线

实现直接动态链接系统 `libLLVM`，通过 LLVM C API 使用 ORC LLJIT：

```text
Prim 源码
  → Rust Lexer / Parser / Resolver
  → 寄存器 Core IR
      ├─ 解释器执行
      └─ @jit 首次调用
          → LLVM IR 文本
          → LLVMParseIRInContext + LLVMVerifyModule
          → LLVMRunPasses（O1/O2）+ 再次验证
          → LLVMOrcLLJITAddLLVMIRModuleWithRT
          → LLVMOrcLLJITLookup
          → 当前进程调用机器码入口
```

不启动外部 Clang，不生成中间共享库，不编译 LLVM 源码，不静态链接 LLVM。LLVM 动态库是可执行文件的运行时依赖，必须存在于运行机器。

[`build.rs`](../src/prim-rs/build.rs) 使用 `llvm-config --libdir` 与 `--link-shared --libnames` 确定链接位置。`LLVM_CONFIG` 环境变量可以选择工具链。Rust 项目没有第三方 crate，所有 Cargo 命令可以 `--offline --locked` 执行。

当前实测 LLVM 14 使用 typed pointer IR；代码也为 LLVM 15–18 提供 opaque pointer IR 输出分支，但尚未在这些版本上验证。目标 triple 与 datalayout 从实际 LLJIT 实例取得，不硬编码宿主布局。

LLJIT C API 定义了模块加入、符号查询与 JIT 资源所有权，参考 [LLVM LLJIT C API](https://llvm.org/doxygen/group__LLVMCExecutionEngineLLJIT.html)。

## 3. 统一中间表示

[`ir.rs`](../src/prim-rs/src/ir.rs) 生成带源码 Span 的寄存器指令：常量、值复制、槽创建/读取/写入、字段定位、闭包创建、调用检查、调用、分支、跳转和返回。

变量名字在编译阶段解析为寄存器。捕获列表与参数列表确定环境布局；运行时不沿外部作用域逐层搜索名字。语句与表达式共享同一套语义降低，解释与本机路径使用相同 Function/CodeId。

每次求值可调用闭包字面量会创建独立实例；同源码节点共享 CodeId。机器码只关联 CodeId，不保存某个实例的槽地址或捕获数值。因此同一个计数器构造器生成的多个对象可以共享代码，同时保持独立环境。

### 3.1 从槽看 JIT 友好

Prim 的可变状态只有一种载体：槽。名字、字段、参数和捕获最终都落在槽上，而槽的创建与共享方式在源码中全部显式，编译器不需要猜测。以下面的程序为例（`prim ir` 输出，已精简）：

```prim
let x = 10;
let snap  = $[x]()  { x };
let share = $[&x]() { x = x + 1; x };
let alias = &x;
```

```text
顶层                                       snap (fn1)        share (fn2)
r2 = Const 10                              r3 = Load r2      r3 = Load r2
r3 = NewSlot r2       ; let x              Return r3         r5 = r3 + 1
r4 = MakeClosure fn1, [(r3, 值捕获)]                         Store r2, r5
r6 = MakeClosure fn2, [(r3, 槽捕获)]                         r6 = Load r2
r8 = Copy r3          ; let alias = &x                       Return r6
```

1. **名字在编译期解析为持有槽句柄的寄存器。** 每次读写都是对固定寄存器的 `Load`/`Store`；运行时没有作用域链、名字查找或字典环境。捕获列表与参数列表决定函数的环境布局，函数体能访问的槽集合在编译期就已封闭。
2. **槽只有两种来源。** 新槽由 `let`、值参数和值捕获通过 `NewSlot` 创建；共享槽只能由 `let x = &y`、`[&y]` 和 `&` 参数三处建立，降低后只是句柄复制（上例的 `Copy r3`），不产生指针值。`&` 不能被返回、存储或参与运算，所以别名关系全部在源码中可见：一个槽若没有出现在这三处，就不可能被其他代码改写。
3. **捕获模式只在构造时起作用，不进入函数体。** 值捕获与槽捕获的区别只体现在 `MakeClosure` 的标志上：值捕获新建私有槽，槽捕获复用外层句柄。函数体看到的都是捕获寄存器中的槽句柄，`snap` 与 `share` 读取 `x` 生成同一条 `Load`。因此机器码以 CodeId 为单位，只编译一次 closuring 过程；同一字面量的所有实例共享机器码，各自的环境在调用时传入，捕获值与槽地址都不会写进机器码。
4. **槽句柄稳定，生命周期与调用栈无关。** 通用路径的参数槽、局部槽和捕获槽分配在受管 arena 中，以整数句柄引用，GC 不移动对象。会被共享或逃逸的槽始终保持这种表示；局部标量路径只提升已确认无共享、无逃逸的局部变量，恢复解释执行时再物化为槽。
5. **环境布局固定。** closure 的字段集合构造完成后不再增删，函数环境就是它的捕获项；不存在运行时增删属性引起的布局变化。
6. **值的种类在运算处检查。** 槽可以保存任意种类的值，但语言没有隐式转换、隐式真值或静默溢出，每个运算的合法输入与错误都由语义唯一确定。本机代码在运算处检查种类：整数走带溢出检查的 LLVM intrinsic，其他种类调用对应的单步 helper。通用路径在槽种类改变后仍然正确。局部标量路径检查失败时会物化活跃状态，从当前指令恢复解释执行，不重新编译，也不重放已完成前缀。
7. **执行策略不在槽中。** `jit(f)` 返回共享原捕获槽的新闭包；执行策略是闭包元数据，不是字段。解释器与本机代码读写同一批槽，同一份状态可以在两条路径之间交替推进，结果保持一致。

O1/O2 已对保守的无别名局部标量子集实现 SSA/栈变量提升，省去循环内的 `NewSlot`/`Load`/`Store` 运行时回调。通用路径继续通过运行时访问受管槽。适用条件与恢复语义见下节。

## 4. 本机代码实际执行什么

[`jit.rs`](../src/prim-rs/src/jit.rs) 提供通用降低与局部标量降低。默认 `-O2`；`-O0` 使用下面的通用路径：

| 结构 | 本机路径 |
| --- | --- |
| `if`、`&&`、`\|\|` | LLVM 条件分支，只执行所需路径 |
| `loop`、break、continue | LLVM 跳转、回边和退出块 |
| return | 写入 Frame 返回值并返回状态码 |
| 整数 `+ - *` | 检查两个操作数种类，然后直接执行 LLVM 带溢出检查的算术 intrinsic |
| 其他动态运算 | 调用单步运行时 helper，执行对应的一条操作 |
| 字段/槽/闭包/调用 | 共用运行时 helper 与 dispatcher |

整数路径使用 `llvm.sadd.with.overflow.i64`、`llvm.ssub.with.overflow.i64` 和 `llvm.smul.with.overflow.i64`，不使用未经证明的 `nsw`。溢出时转入该操作的统一错误检查；操作数为 float/str 等时执行动态慢路径。后续调用即使类型改变，也不要求重新编译。[LLVM 溢出运算说明](https://llvm.org/docs/LangRef.html#arithmetic-with-overflow-intrinsics)。

本机入口中实际存在函数的控制流及算术，不是将整个函数交回解释器执行。动态慢路径经单步 helper 分发。`native_integer_ops` 只在实际完成本机整数快路径后递增。

O1/O2 会为符合条件的函数额外生成局部标量入口：无捕获、无引用参数、不创建闭包、不调用其他函数，最多 256 个寄存器、512 条 Core IR 指令，且不存在局部槽别名。整数/布尔/unit 常量、局部读写、分支循环、比较、布尔检查、整数取负和 `+ - * / %` 在机器栈或寄存器中执行；LLVM 优化将可提升的局部 alloca 转为 SSA。

入口要求值参数均为整数，且没有设置 fuel；否则直接进入同一产物的通用路径。执行中的类型/溢出/除零检查失败时，只将当前 PC 的活跃标量恢复为 Frame 值或 Slot，从该指令继续解释执行，不重跑已完成前缀。函数没有对外共享槽，因此局部槽物化不改变可观察的别名关系。语言错误仍由统一解释执行路径产生。

标量路径累积逻辑 steps 和实际整数算术次数，在回边目标及每 128 条静态指令处检查是否达到 1024 steps，达到后回调刷新计数并检查终端中断；返回和恢复解释时也刷新。中断按安全点响应，不能把异步中断的具体 PC 当作各模式必须一致的语义。fuel 使用通用逐操作检查。

O1/O2 通过动态 libLLVM C API 运行 `default<O1>` / `default<O2>`，没有外部编译进程。优化前后 IR 与机器码都保留用于内省；详见 [优化与测量](optimization.md)。

## 5. 调用 ABI

本机入口使用 C calling convention；实际签名见 `NativeEntry`：

```text
entry(runtime_pointer, frame_index,
      step_callback, condition_callback,
      integer_operands_callback, integer_result_callback,
      numeric_callback) -> i32
```

- `runtime_pointer` 是不透明运行时地址。
- `frame_index` 指定当前已建立的调用帧。
- `step_callback` 完成一条动态操作；条件回调返回 false/true/错误。
- 整数操作数回调只将 i64 标量复制到本机栈临时位置，不向机器码暴露受管值的内部布局。
- 整数结果回调保存结果并更新统计；异常/溢出保留统一语义。
- numeric_callback 使用仅含 i64 的临时 packet，完成入口检查、标量状态物化、计数刷新和返回，不暴露 Rust Value 布局。
- 入口成功返回 0；其他状态表示错误，诊断保存在运行时中。

回调指针作为参数传入，不需要把 Rust 符号批量导出给动态链接器，也不向机器码暴露 `Vec`、`String` 或 Rust 引用的布局。Runtime/Frame 的操作仅通过受控回调执行。

所有回调使用 `catch_unwind` 防止 Rust panic 跨越 C ABI。语言错误使用显式状态和源码位置。嵌套解释/JIT 调用都通过统一 dispatcher；`self` 指向当前实际调用的闭包，包括其执行策略。

`--jit=off` 是用户显式指定的对照模式，会在 stderr 标记 JIT 已禁用；默认不会在本机函数失败后重新解释执行，避免重复输出或写槽。

## 6. 堆与 GC

运行时值为 `Int/Float/Bool/Str/Unit/Closure`。字符串使用不可变引用共享；槽与闭包使用运行时拥有的 arena，以整数句柄引用，地址不暴露给语言程序。

Slot 保存 Value 与可写标记；Closure 保存有序字段槽、User/Builtin/Data 代码类别、JIT 策略和装饰位置。值捕获新建槽，引用捕获共享槽；复制闭包值保留身份引用。

GC 为单线程、非移动标记清扫：

1. 从预置环境、活动 Frame 当前 PC 的活跃寄存器、当前闭包和返回值、最后的脚本结果出发。
2. 追踪槽到值、闭包到字段槽的边。
3. 回收不可达槽与闭包，包括循环结构，将空闲句柄放入复用队列。

自动收集发生在已完成一条操作、结果已经放入帧根之后，以及脚本结束时；std.gc 内置另提供 Call 入口根保护下的显式收集；多步闭包构造内部不触发收集，因此尚未发布的临时状态不会丢失。被调用帧弹出之后，其返回值在下一次收集前写入调用者寄存器。

当前使用 `liveness.rs` 的控制流活跃性结果：每个 Frame 维护 PC，GC 只追踪该位置的活跃寄存器并清空死 Cell。调用者在被调用函数执行时保持 Call 入口的根集合；返回值经 Frame.result 交接。详见 [内存管理](memory-management.md)。

## 7. 缓存与资源所有权

每个 Runtime 持有一个懒创建的 LLJIT，引擎仅在首次实际 JIT 调用时初始化。按 CodeId 保存编译成功入口或失败原因。

```text
未编译 → 生成 IR / 验证 / 加入 ORC / 查询符号 → 可调用入口
                                      ↘ 缓存编译错误
```

LLJIT 接收 module 的所有权，使用每函数 ResourceTracker 管理。模块加入或入口查询失败时立即 remove 资源并 release tracker；成功入口由引擎保持到 Runtime 销毁。测试确认失败符号移除后，同名有效模块仍可加入。Runtime drop 时释放 LLJIT。

缓存当前进程内入口及用于内省的 LLVM IR、机器码字节副本和目标元数据，不保存机器码到磁盘，也不跨进程复用地址。`--emit-llvm DIR` 仅导出可审查的 IR 文本，不是持久化代码缓存。不存在共享库 manifest、动态卸载、缓存目录竞争等额外协议。

## 8. 诊断与可观测性

```bash
build/prim-rs/prim --jit-log --stats --emit-llvm build/prim-rs/ir src/prim-rs/examples/jit.pr
```

日志记录函数 CodeId、LLVM 版本和编译耗时；统计区分：

- `compiled`：本次真正编译成功的函数数量。
- `native_calls`：实际进入本机入口的次数。
- `native_integer_ops`：本机整数快路径执行次数。
- `interpreted_calls`：用户闭包解释调用次数。
- `compile_ms`：累计编译耗时；额外使用 `--time` 可取得每个输入单元的 `elapsed_ms`（解析、编译和执行墙上时间）。
- GC 次数、脚本结束后存活槽/闭包数、运行时操作数。

没有调用的 `@jit` 函数不会生成 IR，不会增加编译计数。编译/导出/LLVM 验证错误指向装饰位置；本机执行中的语言错误指向原始操作位置，并附源级调用栈。

## 9. 验证与演示证据

CPU pod 验证覆盖语言语义、168 个内置接口、闭包/数组循环、根活跃性、错误路径、LLVM 资源与终端恢复。最新测试及 Valgrind 证据分别记录在 [内存管理](memory-management.md) 和 [内置接口](prim-builtin.md)，不把一次通过视为所有程序的形式证明。

[`demo.sh`](../scripts/prim-rs/demo.sh) 执行完整演示。一次实际运行的 `jit.pr` 输出：

```text
500500
500500
3628800
compiled=2 native_calls=19 native_integer_ops=4034
```

相同程序使用 `--jit=off` 输出相同结果，编译/本机调用计数均为零。`primes.pr` 输出 100 以内 25 个素数。`error.pr` 在打印一次之后报告除零，并定位到源码；不会从头重放函数体。

真实日志和导出 IR 位于 `build/prim-rs/demo.log` 与 `build/prim-rs/ir/`。这些生成产物不进入源码版本控制。测试代码与示例程序随项目保存，可在新 CPU worker 上重新执行。

本节演示证明的是手动编译、实际本机调用与语义一致性。这些程序以闭包、字段、输出和终端交互为主，不是数值密集负载，整次运行只有几毫秒，无法摊薄 JIT 编译与管理开销，因此不作为性能结论。性能将在正常数值场景下重新评测，并分别报告首次编译成本与入口缓存后的执行时间。

贪吃蛇的 `step` 使用手动 JIT，方向键和绘图通过共享内置接口执行；`-- --self-test` 可在不进入终端模式时验证游戏逻辑。

## 10. 运行时查询与汇报

`std.inspect.jit(f)` 查询策略、缓存状态、编译耗时、加载地址、函数字节数和按 CodeId 汇总的执行计数。`std.inspect.compile(f)` 可显式预编译 @jit 函数，不执行函数体；其他查询不隐式编译。

ORC 生成对象时读取经过边界校验的 ELF64 函数 st_size，链接后按该长度复制入口字节。`machine_code` 返回 hex，`disassemble` 使用 LLVM 解码实际加载字节，`native` 返回地址/大小/hex/assembly/triple/symbol/origin。目标元数据不可用时明确报错，不猜测长度。

示例和接口语义见 [introspection.md](introspection.md)。
