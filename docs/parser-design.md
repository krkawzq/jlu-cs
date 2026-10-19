# Prim Core 1 语法分析与名字解析

版本：2026-10-19。状态：当前 Rust 实现说明。本文给出完整 EBNF、优先级、AST、Resolver 和降低到核心 IR 的契约。语言含义见 [language-spec.md](language-spec.md)，Token 见 [lexical-spec.md](lexical-spec.md)。源码入口为 `src/prim-rs/src/syntax.rs` 与 `src/prim-rs/src/ir.rs`。

## 1. 前端阶段与唯一表示

```text
TokenBuffer
    → Parser：AST，保留源码结构
    → ir::compile：字面量范围、名字到寄存器解析、捕获与控制目标
    → Core IR + 活跃寄存器根映射
    → 解释器 / 手动 JIT 后端
```

Parser 不推断变量是否存在、对象是否有字段或装饰器是否可调用。Resolver 不执行用户程序。两个执行后端使用同一个已解析的 Core IR 和运行时语义。

实现使用**手写递归下降 + Pratt 表达式解析**，不依赖 Bison。名字解析、静态检查与 IR 降低合并在 `ir::Builder` 中，不另构造带 BindingId 的中间 AST。

## 2. EBNF 记法

- `=` 定义规则，`;` 结束规则。
- 双引号内容为源码终结符；`IDENT` 等大写名字来自 Lexer。
- `|` 为选择，`(...)` 为分组，`[...]` 为可选，`{...}` 为重复零次或多次。
- 引号内的 `"["`、`"{"` 是源码字符，不是 EBNF 元符号。
- 所有列表只在非空时允许一个尾随逗号。
- `EOF_TOKEN` 对应实际的 `TokenKind::Eof`，必须消费，不能只解析文件前缀。其他大写词法类别与实际枚举的映射见 Lexer 文档。

## 3. 完整文法

```ebnf
program              = body EOF_TOKEN ;
body                 = { statement } [ expr ] ;
block                = "{" body "}" ;

statement            = let_statement
                     | assignment_statement
                     | return_statement
                     | break_statement
                     | continue_statement
                     | expression_statement ;

let_statement        = "let" IDENT "=" initializer ";" ;
initializer          = expr | reference ;
assignment_statement = expr "=" expr ";" ;
return_statement     = "return" [ expr ] ";" ;
break_statement      = "break" [ expr ] ";" ;
continue_statement   = "continue" ";" ;
expression_statement = expr ";" ;

expr                 = logical_or ;
logical_or           = logical_and { "||" logical_and } ;
logical_and          = comparison { "&&" comparison } ;
comparison           = additive [ comparison_op additive ] ;
comparison_op        = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
additive             = multiplicative { ("+" | "-") multiplicative } ;
multiplicative       = unary { ("*" | "/" | "%") unary } ;
unary                = ("!" | "+" | "-") unary | postfix ;
postfix              = primary { call_suffix | field_suffix } ;
call_suffix          = "(" [ arguments ] ")" ;
field_suffix         = "." IDENT ;
arguments            = argument { "," argument } [ "," ] ;
argument             = expr | reference ;
reference            = "&" postfix ;

primary              = literal
                     | IDENT
                     | "self"
                     | paren_expr
                     | block
                     | if_expr
                     | loop_expr
                     | data_closure
                     | callable_closure ;
paren_expr           = "(" [ expr ] ")" ;
literal              = INT_DEC | INT_HEX | INT_OCT | INT_BIN
                     | FLOAT_DEC | STRING | "true" | "false" ;

if_expr              = "if" expr block [ "else" (block | if_expr) ] ;
loop_expr            = "loop" block ;
data_closure         = "@" "{" { statement } "}" ;
callable_closure     = bare_closure | "@" IDENT callable_closure ;
bare_closure         = "$" "[" [ captures ] "]"
                       "(" [ parameters ] ")" block ;
captures             = capture { "," capture } [ "," ] ;
capture              = [ "&" ] IDENT ;
parameters           = parameter { "," parameter } [ "," ] ;
parameter            = [ "&" ] IDENT ;
```

`assignment_statement` 先允许一般表达式作为左侧，随后必须经过静态 place 检查。这样能对 `a + b = c;` 给出“不是可写位置”的诊断，而非含糊的缺分号。`reference` 的目标也需要此检查。

`body` 的最后一项只有表达式可以不带分号。`let`、赋值、`return`、`break`、`continue` 永远带分号；`if`、`loop`、闭包和块用作中间语句时也必须带分号。

数据闭包初始化体没有 `body` 的可选尾表达式。`@{ let x = 1; }` 合法，`@{ 1 }` 非法。

## 4. 优先级与结合性

从低到高：

| 层级 | 运算 | 结合性/规则 |
| --- | --- | --- |
| 1 | `\|\|` | 左结合、短路 |
| 2 | `&&` | 左结合、短路 |
| 3 | `== != < <= > >=` | 同级，不允许连续比较 |
| 4 | `+ -` | 左结合 |
| 5 | `* / %` | 左结合 |
| 6 | 前缀 `! + -` | 右结合 |
| 7 | 调用 `(...)`、字段 `.name` | 从左向右串接 |
| 8 | 原子、括号、块、条件、循环、闭包 | 完整 primary |

`=` 不在表达式优先级表中。`&` 也不是通用前缀运算符；它只在 initializer/argument/capture/parameter 的指定位置出现。`@` 是数据闭包或装饰器的引导符，不是任意表达式的一元运算符。

```text
-a.x * b       → Multiply(Negate(Field(a, x)), b)
a || b && c    → Or(a, And(b, c))
f(1).g(2)      → Call(Field(Call(f, [1]), g), [2])
a < b && b < c → And(Less(a, b), Less(b, c))
```

`a < b < c`、`a == b != c` 直接报连续比较错误。`(a < b) == true` 合法；`a == (b == c)` 也合法。`a = b = c;` 不合法。

实现使用的 Pratt binding power：`|| (10,11)`、`&& (20,21)`、比较 `(30,31)`、加减 `(40,41)`、乘除模 `(50,51)`、前缀右侧下限 `60`、后缀 `70`。解析比较后，如在同一未加括号层再次遇到任意比较符号，立即拒绝。

## 5. 确定性的解析决策

### 5.1 语句与尾值

```text
parse_body(end_token):
    statements = []
    while peek != end_token:
        若为 let/return/break/continue：解析必须分号终止的语句
        否则解析 expr 为 lhs
            若下一项为 '='：解析 rhs expr，要求 ';'，保存 Assign
            若下一项为 ';'：消费并保存 ExprStmt
            若下一项为 end_token：保存 lhs 为 tail，并结束
            其他情况：报告缺少分号或非法后继
    return Body(statements, optional_tail)
```

顶层的 `end_token` 是 EOF，块的为 `}`。数据闭包使用 `parse_statements_until(RBRACE)`，最后一种无分号尾值分支禁用。

允许空 body；不接受独立分号。尾表达式后不能再接任何语句；不能根据换行推断两个表达式是两条语句。

### 5.2 花括号和 `@`

- primary 位置看到 `{`：直接解析普通块。
- 看到 `@` 后紧跟 `{`：数据闭包初始化体。
- 看到 `@` 后紧跟 `IDENT`：装饰器；继续要求 callable_closure。
- 看到 `$`：必须依次看到捕获列表、参数列表和块。
- `@jit f` 非法；对已有闭包用 `jit(f)`。
- `@a @{ ... }` 非法；数据闭包不接受装饰器前缀。
- `@a $[]() {}(1)` 的调用后缀应用于装饰后的结果；装饰器自身需要参数时先普通调用构造。

### 5.3 条件后的块

Pratt 解析条件遇到一个无法作为后缀/中缀的 `{` 即停止，由 `if_expr` 消费分支块。条件本身也可以是块，只是其运行结果仍须为 bool：

```prim
if { true } { 1 } else { 2 }
```

没有省略花括号的分支形式；`else` 由当前 `if_expr` 直接消费，因此没有无花括号语言的悬挂 else 问题。

### 5.4 括号与引用

`()` 产生 UnitLiteral；`(expr)` 产生 GroupExpr。保留分组节点直到静态检查完成，便于诊断整数最小值和原始 Span；之后可消除。

引用语法暂时接受 postfix，例如 `&obj.x`、`&(x)`、`&make().x`。它不接受 `&x + y` 作为完整引用实参；需要值计算时去掉 `&`，需要字段时先取得实际 place。

## 6. 实际 AST

AST 位于 `syntax.rs`，节点持有 Span，名字和字面量使用拥有所有权的 String。核心形状如下：

```text
Expr { kind: ExprKind, span }
Body { statements: Vec<Stmt>, tail: Option<Box<Expr>>, span }
Stmt { kind: StmtKind, span }
Arg { expr, by_ref: bool }
Binding { name, by_ref: bool, span }

ExprKind:
  Int / Float / Str / Bool / Unit / Name / SelfValue
  Group / Unary / Binary / Call / Field
  Block / If / Loop / Data / Closure / Decorate

StmtKind:
  Let(name, Arg) / Assign(target, value) / Expr
  Return(optional_value) / Break(optional_value) / Continue
```

所有二元运算，包括 `&&`/`||`，均使用 Binary AST；编译阶段将短路运算降低为分支。引用不是语言值，Arg/Binding 的 by_ref 表示该语法位置的传槽模式。赋值是专用语句，不属于 Binary。

字符串在 Lexer 已解码；整数/浮点仍保留原词素。Group 保留到编译阶段，支持对最小负整数字面量进行检查。

## 7. 字面量检查与名字解析

### 整数范围

`parse_integer` 去掉 `_` 后按前缀解析 u64 幅值。普通值要求不大于 i64::MAX。幅值恰为 `2^63` 且在去掉 Group 后是前缀负号的直接操作数时，生成 i64::MIN。

```text
-9223372036854775808       合法
-(9223372036854775808)     合法
9223372036854775808        IntegerLiteralOutOfRange
0 - 9223372036854775808    IntegerLiteralOutOfRange
--9223372036854775808      编译合法，执行外层取负时 IntegerOverflow
```

浮点必须能转换为有限 binary64，否则 FloatLiteralOutOfRange。当前没有独立常量折叠阶段。

### 作用域与寄存器

Builder 维护词法作用域栈，每层保存 `(name, Reg)`。Reg 是 usize；名字指向保存 Slot 句柄的寄存器。没有运行时父作用域搜索，也没有单独的 BindingId/EnvIndex 节点。

- 每个函数的寄存器 0/1 提供 std/jit 槽；名字不可遮蔽。
- 捕获与参数先加入函数最外层作用域，同层局部声明不得重名。
- let 先编译初始化式，再加入新名字；内层遮蔽可以读取外层同名值。
- 创建嵌套函数时，只给新 Builder 显式捕获和参数，不继承所有外层名字。
- 捕获遗漏与真正未定义名字统一报 UndefinedName，并提示显式捕获。
- self 编译为 SelfValue，只在用户函数及其内部普通块/数据初始化体合法。

每个闭包源码节点对应 `Program.functions` 中一个索引，作为 CodeId。Function 记录捕获名字/寄存器、参数模式/寄存器和指令数组。

### place 与控制目标

Group 被递归剥离。Name 要求可见且可写；Field 先编译接收者，运行时检查对象类别、字段存在与可写性。其他目标报 InvalidPlace。

Builder 的循环栈记录入口指令位置、结果寄存器和待修补的退出跳转；return 是否可用由当前函数/初始化边界控制。进入数据闭包时屏蔽外层循环/return 目标，仍保留词法名字与 self 的可见性。进入新的可调用闭包则建立全新的控制上下文。

非法控制转移在编译阶段拒绝，包括不可达代码。数据闭包的字段按初始化体直接 let 的声明顺序确定。

## 8. 实际 Core IR

```text
Function {
  code: Vec<Op>, registers: usize,
  captures: Vec<(String, Reg)>, capture_modes: Vec<bool>, definition_span: Span,
  parameters: Vec<(Binding, Reg)>,
  live_in: Vec<Vec<Reg>>,
  global_inputs: Vec<(String, Reg)>, global_outputs: Vec<(String, Reg)>, span
}
Op { instruction, span }
```

指令集合：

```text
Const / Copy / NewSlot / Load / Store / Field / SelfValue
Unary / Binary / BoolCheck
MakeData / MakeClosure
CheckCall / Call / CheckCallable
Jump / Branch / Return
```

解释器直接执行此 IR，JIT 将同一指令序列编译为 LLVM 基本块。没有额外字节码 VM，也没有 phi 指令：分支通过 Copy 写入同一个结果寄存器，在合并位置使用它。

### 捕获与调用

MakeClosure 记录源槽寄存器及 Value/Slot 模式。值捕获运行时新建槽，引用捕获复用已有句柄。每次调用新建参数槽，捕获槽保留在闭包实例中。

CheckCall 在实参表达式之前执行；后续从左到右求值实参，再 Call。callee 的值在求实参前已经存入寄存器，因此实参里的重新赋值不会改变本次调用对象。

Field 定位赋值目标后再执行 RHS，目标槽在 RHS 调用期间是 GC 根。装饰器降低为普通调用及 CheckCallable，不在 Parser 中执行。

### 控制流

if 与短路生成 Branch/Jump；loop 生成入口、回边及修补后的 break 出口。break 值复制进循环结果寄存器；return 生成 Return。编译器可能保留不可达的后续指令，但执行流程不会进入它们，没有虚构的返回路径。

### GC 根活跃性

`liveness.rs` 根据指令 uses/definition 和控制流后继求逆向不动点，得到每个指令入口的活跃寄存器集合。解释器、本机回调和整数快路径维护 Frame.pc；GC 只追踪对应活跃寄存器，并清空死寄存器。被调用函数运行期间，调用者停在 Call 入口，其 callee、实参和后续还需使用的值保持为根。

详细安全点和生命周期见 [memory-management.md](memory-management.md)。

## 9. 错误策略

当前前端遇到第一个错误立即返回，不生成错误 AST、不补虚拟分号、不做多错误同步恢复。表达式解析设置 256 层递归限制；它不是整个源码规模的内存配额。

常见实际类别：SyntaxError、UndefinedName、DuplicateBinding、ReservedBinding、InvalidPlace、InvalidSelf、InvalidControlTarget、IntegerLiteralOutOfRange、FloatLiteralOutOfRange。语法错误通过消息区分缺分号、缺结束符、连续比较等情况，不为每种语法失败定义独立枚举。

`&-x` 等不是合法 place 的输入可能先形成候选节点，再在静态检查中报 InvalidPlace；本文 EBNF 描述可接受程序，不要求所有非法程序在同一个子阶段报错。

## 10. 检查入口

```bash
build/prim-rs/prim --tokens src/prim-rs/examples/hello.pr
build/prim-rs/prim --ast --check src/prim-rs/examples/closures.pr
build/prim-rs/prim --ir --check src/prim-rs/examples/snake.pr
```

测试覆盖正反例、捕获与控制边界、求值顺序、解释/JIT 一致性和 GC 压力。未装饰函数没有编译次数，装饰函数实际调用后产生 ORC 本机入口；不存在外部编译器进程作为测试条件。

## 11. REPL 增量编译

`ir::compile_fragment` 为新输入创建独立顶层函数，导入会话全局槽并记录新顶层声明。旧函数和 CodeId 不改写，不重新执行历史指令。编译失败时撤销本次追加的函数；运行时失败保留已执行的写入和声明。会话语义、源码限制与 reset 见 [CLI 手册](cli.md)。

## 12. 内置 AST 观察接口

`std.ast.parse/tokens` 直接使用本文 Parser/Lexer，只观察语法而不进行名字解析或执行；结果投影为只读 kind/span/attributes/children 记录。`std.inspect.ast(f)` 解析保留的原始闭包字面量，返回片段相对的源码范围。节点与字段说明见 [introspection.md](introspection.md)。
