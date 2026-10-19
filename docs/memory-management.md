# Prim 内存管理与审计记录

本文对应当前 Rust 实现，涵盖语言对象、运行时根、LLVM 资源、终端资源和输出缓冲。相关代码：[`runtime.rs`](../src/prim-rs/src/runtime.rs)、[`liveness.rs`](../src/prim-rs/src/liveness.rs)、[`jit.rs`](../src/prim-rs/src/jit.rs)、[`terminal.rs`](../src/prim-rs/src/terminal.rs)。

## 1. 所有权总览

| 资源 | 所有者 | 释放时机 |
| --- | --- | --- |
| 输入源码 String | CLI 调用者 | 执行与诊断结束 |
| 留存源码 SourceUnit / Rc<String> | Runtime.program | Runtime 析构或 :reset；REPL 诊断与内省共用这个池 |
| Token、AST | Parser/编译入口 | 前端完成；CLI 显式 drop AST |
| Function、Core IR、根活跃性表 | Runtime.program | Runtime 析构 |
| Slot arena、Closure arena | Runtime | GC 清扫；Runtime 析构最终释放 |
| 字符串值 | `Rc<String>` | 最后一个 Rust 所有者释放 |
| Frame 寄存器、参数槽根、返回值根 | 调用分发器 | 调用结束；GC 清除其中死寄存器 |
| 最后脚本结果 | Runtime.last_result | 下一次 run 开始或 Runtime 析构 |
| 机器码与 LLVM module | LLJIT | Runtime/Jit 析构；失败模块立即移除 |
| 留存 LLVM IR、机器码字节副本、符号大小与统计 | Jit / Runtime | Runtime 析构或 :reset；按 CodeId 保存 |
| 反汇编上下文 | 作用域 guard | 每次反汇编结束即 LLVMDisasmDispose |
| LLVM 错误消息 | 处理错误的 FFI 包装 | 转为 Rust String 后调用对应 dispose |
| 终端属性与信号处理器备份 | Terminal | leave、run 收尾或 Drop 恢复 |
| 捕获输出 String | Runtime.output / 调用者 | take_output 转移所有权；调用者 drop |

这里有两个独立层次：Rust 管理宿主对象的资源生命周期；Prim GC 管理语言对象图的可达性。闭包循环使用整数句柄形成，不形成 `Rc<Closure>` 环。

## 2. 值、槽、闭包与句柄

```text
Value = Int(i64) | Float(f64) | Bool | Str(Rc<String>) | Unit | Closure(id)
Slot = { value: Value, writable: bool }
Closure = { fields: Vec<(String, SlotId)>, code, jit, marked_span }
```

SlotId/ClosureId 是 arena 索引，不是堆地址。arena 为 `Vec<Option<T>>`，空槽可复用；向量扩容和收缩不改变存活对象的句柄。没有把 Vec 内部元素地址交给 JIT。

句柄只在所属 Runtime 中有效。宿主 API 返回的 Value 中可能含闭包句柄，不能跨 Runtime 混用，也不能在 Runtime 已销毁后当作可解引用对象。当前公共调用接口不接受任意宿主 ClosureId 作为可执行参数。

语言侧：

- `let b = a` 新建槽并复制 Value；闭包 Value 保留身份引用。
- `let b = &a` 复用同一个 SlotId。
- 值捕获新建槽；槽捕获保留已有 SlotId。
- 引用参数再被捕获后，即使原调用结束，槽仍由逃逸闭包可达。
- `std.array` 是数字字段的数据闭包，元素槽采用同一 GC 模型。

回收后空闲索引可以复用，但回收发生前必须保证语言对象图及活跃寄存器中没有该句柄，因此不会把仍可见对象的身份改成另一个对象。

## 3. 根集合与寄存器活跃性

根包括：

1. 只读 std/jit 预置环境。
2. 每个活动 Frame 在当前 PC 的活跃寄存器。
3. 每个 Frame 的当前闭包及已产生的返回值。
4. Runtime 保存的最后脚本结果。
5. 交互会话的显式全局绑定，以及正在执行的输入单元已经声明的待导出顶层槽。

当前闭包的整个捕获环境在调用期间存活，即使某个捕获在本次路径中没有读取。这是有意的保守根，不会改变闭包的持久状态。

### 3.1 活跃性方程

编译完成后对每个函数计算：

```text
OUT[i] = union(IN[s])，s 为指令 i 的后继
IN[i]  = USE[i] union (OUT[i] - DEF[i])
```

Jump、Branch 和 Return 定义控制流后继。使用逆向工作队列求不动点，因此循环回边、分支合并以及 break/continue 都在分析中。

分析阶段使用位集，完成后只保存排序的活跃寄存器编号。GC 根据 `Function.live_in[Frame.pc]` 扫描寄存器，并将不活跃的 Cell 设为 Empty。这样既移除死 Slot/Closure 句柄，也释放死字符串的 Rc 所有者。

### 3.2 调用期间的根

调用前，callee 和实参已经保存在调用者寄存器中。被调用函数执行期间，调用者 Frame.pc 保持在 Call 指令入口；此位置的根表包含 callee、实参及返回后仍需使用的值。

这是嵌套解释/JIT 调用中最关键的约束：不能在进入被调用者之前，提前把调用者切换到“Call 已完成”的根集合。

字段赋值也先将目标 SlotId 保存在寄存器中，再运行 RHS；即使 RHS 改写接收者变量并触发 GC，原目标槽仍然是根。

### 3.3 返回交接

Return 先写入 `Frame.result`。之后即使寄存器根集合为空，返回值仍由 result 根保护。dispatcher 取出返回值、弹出 Frame，再把它写入调用者结果寄存器。这个交接过程中不触发额外分配/收集。

脚本结果在顶层 Frame 弹出后保存到 `last_result`，随后才执行最终 GC。Rust 调用者持有返回字符串的 Rc 时，字符串可独立于 Runtime 存活；闭包句柄则仍依赖所属 Runtime。

## 4. 收集算法与安全点

收集流程：

1. 清除 Frame 中的死寄存器，收集活跃根。
2. 用工作栈及 visited 集合迭代追踪：槽到值，闭包到字段槽。
3. 对字符串记录唯一 Rc 对象及内容字节数，用于诊断。
4. 清扫未标记的 Slot/Closure，放入空闲索引表。
5. 截去 arena 尾部连续空洞，并同步清理已越界的空闲索引。
6. 容量明显超过当前长度时收缩 arena/空闲索引向量，更新统计。

自动 GC 在运行时操作完成、结果已进入寄存器后，依据累计分配数量触发。默认阈值 1024，可用 `--gc-threshold N` 调整。`std.gc.collect()` 和 `std.gc.stats()` 也提供显式安全点；程序结束会再次收集。

构造一个数组、闭包或参数帧的内部步骤不自动触发 GC；所有结果先发布到根位置，再检查自动收集条件。std.gc 内置调用没有语言对象实参，执行时调用者仍停在 Call 入口。

原生整数算术仅在机器栈/寄存器中保留 i64 标量，不是受管指针。整数结果回调写入 Value 后更新 Frame.pc。解释分支、本机条件回调和动态操作回调都维护同一 PC/根映射。

## 5. 空间回收与统计口径

`live_slots`、`live_closures` 是标记后存活数量，不是 arena 的 Vec 长度或 capacity。可达的高索引对象会阻止缩短到该索引以下；中间空洞保留在空闲表供后续分配复用。

因此：

- 不可达对象被清扫，不代表 allocator 会立即把 RSS 归还操作系统。
- 低存活数与较高 arena capacity 可以同时存在，尤其还有高索引对象可达时。
- arena 元素本身的物理地址可以随 Vec 扩容改变；运行时只保存句柄，不能声称槽的物理地址始终固定。
- 字符串统计只计算可达 Value::Str 的内容，不含源码、字段名、程序常量、LLVM 或 allocator 元数据。
- 自动 GC 以分配阈值触发；无分配的空闲阶段可以主动调用 std.gc.collect，不保证每个语句结束都完成清扫。

`std.gc.stats()` 返回 `slots/closures/strings/string_bytes/slot_capacity/closure_capacity/collections/output_bytes`。它先收集，读取快照，再分配统计结果闭包，因此结果闭包自身不计入该快照。

## 6. 错误路径与 FFI

语言错误在两个后端都使用 Result/显式状态码。dispatcher 无论成功或出错都会弹出本次 Frame；顶层清理帧后再次 GC。错误只保存消息与源码 Span，不保存语言对象图。

机器码通过 opaque Runtime 指针与 Frame 索引调用 Rust helper，不暴露 Rust 引用、Vec、String 内部布局。回调捕获 Rust panic，防止 unwind 穿过 C ABI；程序中的正常类型/算术错误不会触发 panic。

LLVM 资源交接：

| 操作 | 所有权处理 |
| --- | --- |
| 创建 memory buffer → ParseIRInContext | buffer 交给 LLVM 解析接口消费 |
| 解析失败 | dispose 解析消息与 ThreadSafeContext |
| 验证失败 | dispose 消息、Module、ThreadSafeContext |
| 创建 ThreadSafeModule | Module 交给 TSM；TSM 持有 Context 的共享所有权，释放本地 Context 引用 |
| AddLLVMIRModuleWithRT | TSM 所有权交给 ORC，以函数专属 ResourceTracker 跟踪 |
| 模块加入/入口查询失败 | remove tracker 资源，再 release tracker 引用 |
| 成功 | release 本地 tracker 引用，机器码继续由 LLJIT 持有 |
| Runtime 析构 | dispose LLJIT，释放所有代码/module 资源 |

失败原因按 CodeId 缓存在当前 Runtime 内，避免重复编译失败函数。成功机器码也保留到 Runtime 销毁，这是明确的缓存生命周期，不是随每个闭包实例 GC 卸载。缓存大小受当前 Program 的函数数量限制。

## 7. 长时间交互资源

CLI 设置 `capture_output=false`，终端绘制直接输出，不把每一帧拼进 Runtime.output。Rust 嵌入接口默认捕获输出；调用者应按需使用 `take_output()` 转移并清空缓冲区，或关闭捕获。持续保留捕获文本会按输出量增长，这是显式持有的数据。

Terminal 不持有每帧画面，只保留 termios 快照、旧信号处理器和极少量待处理键字节。leave、run 收尾、Drop 都恢复终端；伪终端测试检查完整属性数组与光标/备用屏幕退出序列。

SIGINT/SIGTERM 处理器只设置原子中断标志；恢复在正常 Rust 控制流中进行。SIGKILL 或进程崩溃无法保证执行清理，不应把它们写成 RAII 可以处理的退出方式。

文件内置使用作用域内的 File，完成后自动关闭。不在 Runtime 中缓存打开的文件句柄。

## 8. 本轮审计发现与修复

| 发现 | 实际影响 | 修复 |
| --- | --- | --- |
| 所有帧寄存器都作为根 | 已离开块的数组、对象和字符串可一直保留到函数结束 | CFG 活跃性分析；GC 只追踪活跃寄存器并清空死 Cell |
| arena/空闲表保留历史峰值容量 | 大量对象回收后仍持有大向量分配 | 截去尾部空洞，容量过大时收缩向量 |
| ORC 加入模块后入口查询失败 | 失败 module 资源保留到整个引擎销毁 | 使用 ResourceTracker，失败立即移除资源 |
| 每次创建 Frame 克隆整个 Function | 复制代码与元数据，增加临时分配和峰值 | 只复制参数/捕获布局，不复制指令和根表 |
| 实时输出与输出捕获混在一起 | 长时间绘图会累计完整历史画面 | CLI 关闭捕获；库提供可关闭捕获及 take_output |

临时根保留复现：在块内创建 20,000 元素数组，退出块后主动 GC，再读取统计。

| 指标 | 修复前 | 修复后 |
| --- | ---: | ---: |
| 相对基线额外存活槽 | 20,008 | 1 |
| Slot arena capacity | 32,768 | 128 |

修复后的 1 个额外槽是仍用于计算差值的基线局部绑定。日志为 `build/prim-rs/memory-audit-before.txt` 与 `memory-audit-after.txt`；可运行 [`examples/memory.pr`](../src/prim-rs/examples/memory.pr) 复查当前行为。

## 9. 验证证据

CPU pod 上的内存用例覆盖：

- 函数仍运行时回收已离开块的 20,000 元素数组与 4 MB 字符串。
- 分支、循环回边、引用参数逃逸与嵌套解释/JIT 调用的根保活。
- RHS 改写接收者并触发 GC 时，原赋值目标槽仍正确。
- 后续实参触发 GC 时，先前实参保持有效。
- 正常返回、语言错误、重复 run 和 Runtime 析构。
- 循环引用回收、输出捕获关闭/转移、返回字符串的 Rc 生命周期。
- 连续创建/销毁 30 个 JIT Runtime，以及长循环持续分配并主动 GC。
- ORC 材化失败后释放符号资源，同名有效 module 随后可以正常加入。

Valgrind 3.18.1 对 8 项内存用例的实际记录（新增长循环用例之前的审计批次）：

```text
definitely lost: 0 bytes
indirectly lost: 0 bytes
possibly lost: 48 bytes
still reachable: 184820 bytes
ERROR SUMMARY: 0 errors（错误退出口径只计 definite/indirect 与内存访问错误）
```

48 字节在单独运行纯字符串测试时也出现；调用栈指向 Rust 测试框架的 `std::thread::current::init_current` / mpmc 测试结果通道 TLS。没有把它掩盖为“全部内存归零”，也没有用 suppression 隐藏。

独立 `prim snake.pr -- --self-test` 进程的 Valgrind 记录：

```text
definitely lost: 0 bytes
indirectly lost: 0 bytes
possibly lost: 0 bytes
still reachable: 184820 bytes
ERROR SUMMARY: 0 errors
```

仍可达字节与“丢失”不同；这里没有将它们一概宣称为 LLVM，也没有声称进程 RSS 会回到启动前。结果证明这些已执行路径没有被 Valgrind 检出的丢失块或内存访问错误，不等同于对所有程序的形式证明。

日志：`build/prim-rs/valgrind-memory.log`、`valgrind-harness.log`、`valgrind-snake.log`，均未使用泄漏 suppression。终端清理另由真实 PTY 检查验证。

## 10. 增量会话的内存边界

Session 每次只编译新输入，向 Program 追加函数；已有函数 CodeId 不变，已编译入口可继续使用。顶层输入从会话全局表取得槽，完成后仅导出实际执行过的顶层声明。未执行的声明不会成为假根。

会话全局绑定是明确持有的根，即使当前输入没有读取也必须保留，否则下一次输入将丢失变量。根活跃性仍用于清除临时表达式和嵌套块局部值。后续输入 let 遮蔽旧绑定时，旧槽若仍被闭包捕获则继续存活。

`:del NAME` 删除该会话绑定并释放最后结果根，然后 GC；`:reset` 销毁旧 Runtime、全局表、历史源码、Core IR 与 LLJIT。会话累计源码默认限制 16 MiB，可用 --max-source-bytes 修改。源码与代码保留用于旧闭包和诊断，不能把这部分线性会话历史当作对象泄漏。

会话测试确认：没有历史重放、旧捕获槽存活、失败后的部分副作用可见、未执行声明不导出、编译失败撤销新增函数、跨输入错误定位与 reset 释放。

独立 REPL 进程执行创建 JIT 闭包、删除变量、GC 和 reset 后，Valgrind 同样报告 definite/indirect/possible lost 均为 0、内存错误为 0；退出时 still reachable 为 193,012 字节。日志为 `build/prim-rs/valgrind-repl.log`，与不含测试框架的程序路径对应。最终 release 测试共 59 项，另有真实 PTY 交互检查。

## 11. 复现命令

在 CPU worker 上执行：

```bash
scripts/prim-rs/build.sh
build/prim-rs/prim --stats src/prim-rs/examples/memory.pr
CARGO_TARGET_DIR=/tmp/prim-rs-target cargo test --release --offline --locked --test memory
uv run --offline --no-project python scripts/prim-rs/test_interactive.py
valgrind --leak-check=full --show-leak-kinds=definite,indirect,possible \
  --errors-for-leak-kinds=definite,indirect,possible --error-exitcode=99 \
  build/prim-rs/prim src/prim-rs/examples/snake.pr -- --self-test
```

查看内存时同时使用语言堆统计、生命周期测试和进程级工具，避免把仍被程序持有的对象、机器码缓存、allocator 保留页与真正泄漏混为一谈。

## 12. 内省快照与本机代码观察

AST 和 inspect 记录使用普通受管数据闭包，字段槽只读；不建立额外永久根。members/captures 中的 value 仍按普通闭包身份共享规则复制，保留快照也可能有意保留原对象。

Program 新增 SourceUnit 池以及函数 definition_span/capture_modes。REPL 的源级诊断和函数源码内省共用这个池，避免再次保留完整拼接历史；普通 prepare API 拷贝调用者提供的源码，确保调用者释放字符串后内省仍有效。

ORC ObjectTransformLayer 回调只借用原 buffer，读取 ELF 函数大小而不更换/释放 buffer。回调状态放在稳定 Box<Mutex<...>> 中，寿命覆盖 LLJIT；析构先结束引擎，再释放回调状态和缓存。入口查询成功后，只复制符号确定的已加载函数范围，不提供任意地址读取 API。

反汇编使用字节副本及每次调用的 LLVM 上下文 guard。程序缓存保留原始 IR/机器码副本，规模按已编译 CodeId 增长，在 Runtime/:reset 时释放；managed GC 统计不把这些宿主缓存算作 Slot/Closure。


## 13. 局部标量 JIT 与优化产物

O1/O2 对不逃逸且无别名的局部变量使用机器栈/SSA 标量，支持 int、bool、unit。这里的 payload/tag 都不是受管句柄，无需加入 GC 根。值参数在进入 Frame 时仍有私有 Slot；符合条件的函数没有捕获、引用参数、闭包创建或函数调用，因此原生循环内不会触发受管分配或 GC。

类型检查、溢出或除零需要恢复通用语义时，按照失败 PC 的 live_in 映射复制活跃寄存器，给活跃局部槽分配真实 Slot；完成恢复后才调用解释器。物化过程本身不触发 GC，随后每条解释指令使用既有根映射。恢复后的槽不与外部环境或其他局部槽共享。

本机 packet 在调用栈上，大小为 `2 + 2 * registers` 个 i64，仅在回调期间借用，不保留指针。Rust 只读取对应 action 已初始化的字。返回与错误退出会释放本机栈。标量路径限制 256 个寄存器、512 条指令，较大函数使用通用降低。

LLVM PassBuilder options 每次运行后 dispose，失败时销毁 Module 和 ThreadSafeContext。缓存同时持有优化前和优化后 IR，按 CodeId 增长，随 Runtime/:reset 释放；这是宿主编译缓存，不计入 managed heap。`slot_allocations/closure_allocations` 是累计事件，`native_code_bytes` 仅统计已知符号大小，均不能替代 RSS 或泄漏工具。

本轮优化演示的 Valgrind 结果：definitely/indirectly/possibly lost 均为 0，ERROR SUMMARY 为 0；still reachable 为 184,868 字节。它包含 O2 优化与 LLVM 模块打印、原生代码观察及 Runtime 销毁。完整日志在 `build/prim-rs/valgrind-optimization.log`，本轮共 81 项 Rust release 测试通过。
