# Prim AST、运行时内省与机器码展示

当前提供 `std.ast` 的 7 个接口和 `std.inspect` 的 21 个接口。它们复用实际 Lexer/Parser、运行时闭包、Core IR 和 ORC 编译产物，不是单独模拟的一套展示模型。完整签名与 `.pri` 提示分别见 [内置参考](prim-builtin.md) 和 `builtin/`。

## 1. 一次完整演示

```bash
build/prim-rs/prim --stats --emit-llvm build/prim-rs/inspection-ir src/prim-rs/examples/inspect.pr
```

示例依次展示：

1. 函数参数签名与保留的原始源码。
2. 结构化 AST 的文本展开。
3. 实际 Core IR。
4. JIT 状态从 uncompiled 到 ready。
5. 未执行函数体时 native_calls 仍为 0。
6. LLVM IR、已加载机器码十六进制与 LLVM 反汇编。
7. 调用结果 42，以及执行后的计数。

只展示机器码时：

```prim
let square = @jit $[](x) { x * x };
std.inspect.compile(square);
let native = std.inspect.native(square);
std.print(native.address);
std.print(native.hex);
std.print(native.assembly);
std.assert(square(7) == 49);
```

`compile` 是显式准备入口，不运行 square 的函数体，也不求虚构实参。它要求函数已经由 `@jit`/`jit(f)` 标记；`--jit=off` 下报 JitDisabled。查询接口本身不会隐式编译。

## 2. 结构化 AST

```prim
let source = "let x = 1; x + 2";
let tree = std.ast.parse(source);
std.print(std.ast.dump(tree));
let binary = std.get(std.ast.find(tree, "Binary"), 0);
std.print(binary.attributes.operator);
std.print(std.ast.source_segment(source, binary));
```

节点统一为只读数据闭包：

```text
ASTNode {
  kind: str,
  span: { start, end, line, column, end_line, end_column },
  attributes: closure,
  children: indexed closure
}
```

- start/end 是 UTF-8 字节半开区间；行列从 1 开始，沿用诊断的 Unicode 标量/Tab 规则。
- children 按源码顺序排列；`std.get` 读取，`std.len` 取长度。
- walk 返回深度优先先序节点数组，包含根；find 按区分大小写的 kind 精确筛选。
- parse 只做词法和语法，不执行程序、不解析外部名字；包含未绑定名字的函数也可以生成 AST。
- AST 是观察快照，不提供把任意修改后的 AST 重新执行或替换现有函数的入口。

### 节点种类与字段

| kind | attributes | children |
| --- | --- | --- |
| Program / Block | 空 | 语句与可选 Tail |
| Tail / ExprStmt / Group | 空 | 一个表达式 |
| Let | name | 一个 Argument |
| Assign | 空 | 目标、右侧 |
| Return / Break | has_value | 零个或一个表达式 |
| Continue / Unit / Self | 空 | 空 |
| Integer / Float | raw | 空 |
| String / Boolean | value | 空 |
| Name | name | 空 |
| Unary | operator | 操作数 |
| Binary | operator | 左、右 |
| Call | arity | callee，随后每个 Argument |
| Argument | by_ref | 参数表达式 |
| Field | name | 接收者 |
| If | has_else | 条件、then Block、可选 else 节点 |
| Loop | 空 | Block |
| Data | 空 | 初始化语句 |
| Closure | capture_count、arity | Capture、Parameter、函数 Block |
| Capture / Parameter | name、by_ref | 空 |
| Decorate | name | 被装饰闭包/装饰器 |

数字保留原词素 raw，不在 AST parse 阶段做范围检查或把最小负数的幅值强塞入 i64。绑定节点的 span 对应名字 Token；by_ref 保存在 attributes 中。

Token 快照包含 kind/text/value/span。kind 为 Identifier、Integer、Float、String、Keyword、Symbol 或 Eof；text 是原词素，String 的 value 是解码值，数字 value 仍是原词素字符串，EOF value 是 unit。

单次 AST 输入最多 1 MiB，树或 Token 数最多 8192；dump 输出最多 16 MiB。非法源码包装为 AstError，消息注明被解析字符串中的字节位置，外层诊断指向调用位置。walk/dump 会拒绝循环或重复节点，避免伪造树导致无限遍历。

## 3. 闭包、签名与捕获

```prim
let x = 1;
let f = $[&x](a, &b) { a + b + x };
std.print(std.inspect.signature(f).text);
let capture = std.get(std.inspect.captures(f), 0);
std.print(capture.name);
std.print(capture.mode);
std.print(capture.slot_id);
```

`describe(value)` 对任意值返回 type/kind/display/name/closure_id/code_id/arity/field_count/callable/policy。kind 对闭包细分为 object、function、builtin；没有自然名字的用户函数显示 `<function #CodeId>`，不会猜测某个变量别名是其唯一名字。

`signature(f)` 返回 name、arity、parameters、text。参数记录为 `{name,index,by_ref}`。用户函数保留实际参数名；宿主 builtin 的运行时只记录参数数量，因此使用 arg0、arg1 等位置名，编辑器 `.pri` 中更具说明性的名字不参与执行。

`captures(f)` 只接受 Prim 用户函数，按声明顺序返回 `{name,mode,slot_id,value,writable}`。mode 是声明时的 value/slot 模式；jit(f) 共享捕获槽而不重新捕获，因此原函数与 JIT 包装的 slot_id 可以相同。

`members(object)` 返回现存字段的 `{name,slot_id,type,value,writable}`，包括用户函数的捕获字段。内置代码地址、执行策略等不是用户字段，使用 inspect.jit 查询。

metadata 容器及其数组是只读快照。修改快照字段报 ReadOnlySlot；它不会改写运行时签名、计数或源码。但是 members/captures 中的 value 遵循普通 Value 复制规则：若 value 本身是用户对象，仍共享该对象身份，不能把快照理解成用户对象的递归冻结或深拷贝。

closure_id、slot_id、code_id 只用于当前 Runtime 的观察。槽/闭包回收后索引可复用，不能将这些数字当作永久标识或可解引用的指针。

## 4. 源码、位置与调用栈

Program 持有 SourceUnit，保存输入名字、全局字节基址和共享的源码 String。函数记录完整 `$[...] (...) {...}` 的 definition_span；外层 let 名字和装饰器前缀不包含在 inspect.source 的结果中。

- source(f)：原始可调用闭包字面量；宿主 builtin/数据闭包返回 unit。
- location(f)：文件名及相对该输入单元的字节/行列范围；没有源码时返回 unit。
- ast(f)：解析 source(f)，因此节点 span 相对这个片段从零开始，可以直接与 source_segment(source(f), node) 配合。
- core_ir(f)：实际函数指令、寄存器数和源码字节位置。
- stack()：顶层到当前调用的帧数组，包含 depth/code_id/closure_id/pc/mode/location。

原始源码在编译时保留，不因磁盘文件后续修改而改变。REPL 使用与错误诊断相同的 SourceUnit 池，没有再次拼接一份完整历史源码；旧闭包仍能定位到最初定义的输入。

stack 的 mode 是本次实际执行的 native/interpreted，而不是仅看 @jit 标记；`--jit=off` 时标记函数仍显示 interpreted。

## 5. JIT 元数据

`std.inspect.jit(f)` 返回只读记录：

| 字段 | 含义 |
| --- | --- |
| code_id | 代码节点标识 |
| arity / capture_count | 参数与捕获项数量 |
| registers / instruction_count | Core IR 寄存器与指令数量 |
| policy | 该闭包的 jit / interpreted 策略 |
| enabled | 当前 Runtime 是否启用 JIT |
| state | 当前 CodeId 缓存的 uncompiled / ready / failed |
| compiled | 缓存是否已有有效入口 |
| will_use_native | 当前策略、配置和缓存是否会直接使用已准备的入口 |
| symbol | 入口符号名 |
| address | ready 时入口地址的十六进制字符串，否则 unit |
| code_size | 有可靠符号边界时的机器码字节数，否则 unit |
| triple / llvm_version | 实际目标与 LLVM 版本 |
| data_layout / ir_bytes | LLVM 目标布局与保留 IR 的字节数；未编译时 unit |
| opt_level | 实际产物使用的优化等级 0/1/2；未编译时为当前配置 |
| lowering | ready 时为 local-scalars 或 generic，否则 unit；表示生成的路径，不保证每次调用都进入快速路径 |
| optimized_ir_bytes | LLVM 优化后 IR 的字节数，未编译时 unit |
| scalar_calls / scalar_fallbacks | 成功进入局部标量路径的次数 / 执行中恢复到解释器的次数 |
| compile_ms | 本次缓存产物生成耗时，未准备时 unit |
| native_calls / interpreted_calls | 按 CodeId 累计的真实执行次数 |
| native_integer_ops | 按 CodeId 累计的原生整数快路径次数 |
| error | 编译失败原因，否则 unit |
| metadata_error | 编译成功但机器码元数据不可用的原因，否则 unit |

计数和编译产物按 CodeId 共享，不是单个闭包实例独占。两个由同一字面量创建的闭包可以有相同 address 和计数，却持有不同捕获值。即使某闭包 policy 为 interpreted，也可能查到由另一个 JIT 实例准备的同 CodeId 缓存；这不会让它自动切换执行方式。

快照不会自行更新；执行后重新调用 inspect.jit 获取新计数。

## 6. 机器码与反汇编的来源

没有根据参数类型生成伪汇编，也不扫描入口后任意固定长度的内存。实际流程是：

1. 在 ORC ObjectTransformLayer 安装只读回调，观察 LLVM 真实生成的目标文件。
2. 校验 ELF64、小端、x86_64、可执行 section 与函数 symbol 的边界，取得目标入口的 st_size。
3. LLJIT 完成链接/重定位并查到入口后，在引擎仍持有代码期间，复制这个准确范围的已加载指令字节。
4. 用 LLVM Disassembler 解码这个字节副本，打印每条指令的加载地址、原始字节和 Intel 风格文本。

`native(f)` 的记录为 `{address,size,hex,assembly,triple,symbol,origin}`，origin 固定为 loaded-memory。machine_code 只返回 hex，disassemble 只返回 assembly，llvm_ir 返回优化前的 IR 文本，`optimized_ir(f)` 返回经过所选 LLVM 管线后、实际提交给 ORC 的 IR 文本。O0 也会重新打印解析后的模块，所以两个文本可能仅在格式上不同。

读取查询没有隐式编译；未准备时抛 JitNotCompiled。编译失败抛 InspectError。若目标文件没有可靠的函数边界或 LLVM 反汇编不可用，则抛 NativeInspectionError，不猜测长度。当前目标文件读取仅支持现有 Linux x86_64 ELF 后端。

地址是本次进程地址，受 ASLR 影响。反汇编的分支操作数可能采用相对位移；行首地址是加载地址。普通 Runtime 操作仍通过回调完成，因此看到间接 call 是正常的；整数快路径会出现实际算术及溢出分支。

## 7. 生命周期与验证

源码由 Program/SourceUnit 保有；LLVM IR、机器码副本与符号元数据由 Jit 缓存保有，随 Runtime/:reset 释放。反汇编上下文使用作用域 guard 及时 dispose。AST 和 metadata 快照进入已有受管堆，按普通可达性回收；不会因为查询过某个节点就全局登记永久根。

新测试覆盖解析无副作用、节点顺序/位置、只读快照、捕获槽身份、预编译不执行函数体、真实代码长度/反汇编、实例共享代码、源级栈、失败缓存、REPL 源码及 GC 压力。`examples/inspect.pr` 是完整的可复现展示程序。
