# Prim 优化与性能测量

Prim 保留显式 `@jit`，每个 CodeId 编译一次。默认 O2，局部标量快速路径和通用路径共享同一入口；参数变化不生成额外版本。统一的闭包语义允许底层针对不逃逸的局部变量采用不同存储。

## 1. 三个优化等级

| 等级 | 降低与 LLVM 管线 |
| --- | --- |
| `-O0` | 通用控制流与整数算术降低；不运行额外 LLVM IR 优化管线 |
| `-O1` | 符合条件时增加局部标量路径；运行 `default<O1>` |
| `-O2` | 与 O1 相同的适用范围；运行 `default<O2>`，默认 |

这里的 O0 表示前端和 IR 管线策略，不表示禁用 LLVM 后端所有指令选择优化。`--jit=off` 则始终解释执行，与优化等级无关。CLI 和 Runtime 创建时选定等级；一个 Runtime 内已缓存 CodeId 不因之后修改配置重新编译。

通用 JIT 每条动态操作仍需调用运行时。局部标量路径将 int/bool/unit 的 payload/tag 保存在本机局部位置，由 LLVM 将可提升的 alloca 转为 SSA；循环归纳变量和累加器在优化后 IR 中成为 phi 值。执行期间不为这些局部变量分配 Slot。

## 2. 快速路径边界与错误语义

函数必须无捕获、无引用参数、无局部槽别名，不调用函数、不创建闭包，只包含受支持的标量操作与控制流；还限制为至多 256 个寄存器、512 条指令。共享捕获、递归、数组、字符串等继续使用通用路径。这是可检查的保守子集，不根据运行热点猜测类型。

入口检查所有值参数为 int，且没有设置 fuel。检查失败时从入口执行通用路径。内部局部变量允许变为 bool/unit；类型检查失败、整数溢出、除零时，将失败 PC 的活跃变量物化回运行时，从该指令恢复解释执行。已完成前缀不重跑，错误内容、Span 和调用栈仍由统一运行时生成。

整数加减乘使用 checked intrinsic，除余检查零与 MIN/-1，取负检查 MIN。没有移除溢出检查，也没有把失败改成机器整数回绕。

正常完成与同步错误保留精确的 Core IR steps。原生路径在回边目标及每 128 条静态指令处检查累计 steps，达到 1024 后刷新统计并检查终端中断；返回及恢复解释时也刷新。中断在安全点响应。`--fuel` 明确选择逐操作检查的通用路径，保证预算耗尽位置一致。

## 3. 内省与展示

```bash
build/prim-rs/prim -O2 --stats --emit-llvm build/prim-rs/optimization-ir \
  src/prim-rs/examples/optimization.pr
```

该例展示源码、AST、Core IR、两个 LLVM IR 阶段、真实已加载机器码及反汇编，并验证整数计算和共享捕获行为。用 `-O0` 运行相同源码可以比较通用路径。

| 接口/产物 | 含义 |
| --- | --- |
| `std.inspect.llvm_ir(f)` / `function_ID.ll` | 优化前 IR |
| `std.inspect.optimized_ir(f)` / `function_ID.opt.ll` | 实际提交给 ORC 的优化后 IR |
| `std.inspect.jit(f).opt_level` | 缓存产物的实际优化等级 |
| `.lowering` | `local-scalars` 或 `generic`；未编译为 unit |
| `.scalar_calls` | 成功进入标量路径次数 |
| `.scalar_fallbacks` | 已进入标量路径后，恢复到解释器次数；不含入口拒绝 |
| `.optimized_ir_bytes` / `.code_size` | 优化后 IR / 已加载入口机器码大小 |

查询不会隐式编译。先调用 `std.inspect.compile(f)`，或正常调用带 `@jit` 的函数。lowering 描述产物具备的能力；fuel 和参数类型仍可能让一次调用选择通用路径。

## 4. 测量方法与重现

在现有 CPU pod 中执行，开发机只做编辑：

```bash
cd /mnt/shared-storage-user/dnacoding/wangzhongqi/Code/Project/Prim
bash scripts/prim-rs/benchmark.sh 9
```

基准覆盖整数累加、递归 Fibonacci、共享捕获计数器和数组求和。每个案例对照解释器及 O0/O1/O2，校验每次结果。prepare 时间单列；cold 时间为同一 Runtime 首次 run，包含首次编译；compile_ms 为编译调用累计耗时。随后同一 Runtime 执行 9 次，确认没有新增编译，保留全部原始样本并报告中位数。

warm run 仍包含顶层闭包创建、调用、最终 GC 等实际运行时开销，不是只计机器指令的微基准。helper/分配/steps 是各次 warm run 的平均事件计数。slot_allocations 包含 arena 复用；gc_ms 不等于整个程序内存管理成本。native_code_bytes 是已知符号大小之和，不是所有 LLVM 内存。

输出为 `build/prim-rs/optimization-benchmark.jsonl`、环境信息和源码 SHA-256 清单。报告中的 O0 是当前实现的通用 JIT 对照，不代表修改前二进制；解释器已经移除了调度时的重复指令克隆，缓存命中也跳过编译计时逻辑。

## 5. CPU 实测

本次在现有 GPU=0 / 32 CPU pod 上执行，宿主 CPU 型号为 Intel Xeon Platinum 8558，Rust 1.95.0、LLVM 14.0.0，release 构建。计时阶段没有并行启动本任务的测试或 Valgrind；共享集群仍存在频率与调度波动，未绑定 CPU，按固定模式顺序执行，较小差异不作稳定加速结论。

| 案例 | 解释器 warm ms | JIT O0 warm ms | JIT O1 warm ms | JIT O2 warm ms | O2 compile ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| 整数累加（20,000 次） | 5.4298 | 5.5268 | 0.0429 | 0.0372 | 14.560 |
| 递归 Fibonacci(18) | 3.6061 | 3.4412 | 3.4997 | 3.4446 | 7.114 |
| 共享捕获（5,000 次） | 2.5675 | 2.6850 | 2.6994 | 2.9692 | 9.330 |
| 数组求和（10,000 项） | 5.9453 | 5.5470 | 5.5342 | 6.1091 | 9.420 |

整数案例 O2 warm 中位数约为解释器的 **146.1 倍速度**，相对通用 JIT O0 约 **148.7 倍**。回调由 340,012 次降到 291 次，所有模式仍执行 300,023 个逻辑 steps。每次 warm run 的槽分配事件由 4 次降到 2 次。O2 入口机器码为 1,113 字节，O0 为 640 字节，快速与通用路径并存有代码体积成本。

这些比值仅适用于此整数循环与本次环境。O2 首次 run 为 14.607 ms，其中编译约 14.560 ms；解释器首次 run 为 5.324 ms，因此只调用一次并不划算。捕获和数组案例未进入标量路径，收益有限，甚至可能略慢。

原始数据：[逐次计时与计数](benchmarks/optimization-benchmark.jsonl)、[环境](benchmarks/optimization-environment.txt)、[源码与基准可执行文件 SHA-256](benchmarks/optimization-sources.sha256)。

## 6. 验证范围

差分测试对比解释器与三个优化等级的返回值、输出、steps、错误类型/内容/Span/trace，覆盖循环、continue、提前返回、局部类型变化、入口参数类型变化、共享捕获、别名、溢出、除零、MIN/-1、fuel 和恢复执行。CLI 测试验证优化 flags、两份 IR 文件和内省字段。

终端测试通过真实 PTY 向原生无限循环发送 SIGTERM，验证安全点中断、退出码和终端属性恢复。Valgrind 检查包含 LLVM 优化、代码缓存、内省、反汇编和 Runtime 析构的完整演示路径；这些证据覆盖当前测试程序，不构成任意程序的形式证明。

本轮验证：81 项 Rust release 测试通过；LSP 35 项通过、1 项可选前端探针测试跳过，词法 scope 检查通过；真实 PTY 检查通过。优化展示路径 Valgrind definite/indirect/possible lost 均为 0，内存错误为 0，退出时 still reachable 为 184,868 字节。日志位于 `build/prim-rs/optimization-tests.log`、`optimization-lsp-tests.log`、`optimization-interactive.log` 和 `valgrind-optimization.log`。

局部状态物化后的 TypeError 路径另以 `--gc-threshold=1` 经 Valgrind 验证：definite/indirect/possible lost 与内存错误均为 0，日志 `build/prim-rs/valgrind-scalar-resume.log`。
