# Prim Core 1 语法速查

版本：2026-10-19。本文是当前实现的语法导航，不另维护一份独立文法。

- [语言设计与语义](language-spec.md)：闭包、槽、捕获、调用、控制流与装饰器。
- [词法规则](lexical-spec.md)：关键字、数字、字符串和标点。
- [Lexer 实现契约](lexer-design.md)：Token、Span、状态机与诊断。
- [完整 EBNF 与 Parser](parser-design.md)：语法的唯一详细定义，以及 AST/Resolver。
- [手动 JIT](jit-runtime.md)：`@jit`、LLVM IR、动态 LLVM 与 ORC 本机执行。
- [完整内置接口](prim-builtin.md)：标准输入输出、文本、数学、位运算、序列、随机数、时间、文件、进程、交互终端、系统信息、AST/运行时检查与 GC。
- [内存管理](memory-management.md)：句柄、活跃根、回收和审计证据。

## 核心写法

| 意图 | 写法 |
| --- | --- |
| 新建值槽 | `let x = expr;` |
| 共享已有槽 | `let alias = &x;` |
| 写入槽 | `x = expr;` |
| 立即执行的块 | `{ statements; tail }` |
| 数据闭包 | `@{ let field = expr; }` |
| 可调用闭包 | `$[captures](parameters) { body }` |
| 值捕获/槽捕获 | `$[x, &y]() { ... }` |
| 值参数/引用参数 | `$[](x, &y) { ... }`，调用 `f(value, &slot)` |
| 当前闭包递归 | `self(args)` |
| 字段访问 | `obj.field` |
| 通用装饰器 | `@decorator $[]() { ... }` |
| 手动本机执行 | `@jit $[](x) { ... }` 或 `jit(existing_closure)` |
| 条件值 | `if cond { a } else { b }` |
| 循环值 | `loop { break result; }` |
| 提前返回 | `return expr;` |
| 下一轮循环 | `continue;` |
| 无值 | `()` |

表中 `...` 是说明性省略，不是合法 Token。

## 三个必须区分的构造

```prim
let value = { let x = 1; x };
let object = @{ let x = 1; };
let function = $[]() { let x = 1; x };
std.assert(value == 1);
std.assert(object.x == 1);
std.assert(function() == 1);
```

普通块立即计算尾值；数据闭包立即构造环境；可调用闭包保存代码和显式捕获，调用时才执行。

`@jit` 是手动装饰器，首次调用编译；没有热点计数或自动 JIT。解释与本机代码共用同一槽、环境和动态类型语义。

## 简化边界

分号终止所有语句；只有普通 body 的最后一个表达式可以没有分号。`@{}` 初始化体不能有尾表达式。空块、空数据闭包和空可调用闭包都合法。

没有类型注解、列表/字典/元组字面量、标签、隐式深拷贝、隐式捕获、类或运算符重载。多结果返回数据闭包，独立状态重新构造，代码复用通过闭包传参和普通装饰器完成。

固定数组使用 `std.array(n, initial)`、`std.get(array, i)` 和 `std.set(array, i, value)`；它仍是数据闭包。完整示例见 `src/prim-rs/examples/snake.pr`。
