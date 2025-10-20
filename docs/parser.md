# Prim 语法分析器规范（完整版）

> 本文档详细描述 Prim 语言的语法规则，包括完整的 BNF 定义、运算符优先级、AST 结构、边界情况处理和实现细节。

---

## 目录
1. [核心设计原则](#核心设计原则)
2. [完整 BNF 语法](#完整-bnf-语法)
3. [运算符优先级](#运算符优先级)
4. [AST 节点设计](#ast-节点设计)
5. [边界情况与特殊规则](#边界情况与特殊规则)
6. [实现指南](#实现指南)
7. [测试用例](#测试用例)

---

## 核心设计原则

### 1. 表达式优先
Prim 中几乎所有语句都是表达式，可以产生值：
- ✅ **有返回值**: `let`, `del`, `if`, `loop`, scope, 运算表达式
- ❌ **无返回值**: `break`, `return` (它们改变控制流，不产生值)

### 2. Block vs Scope 的区别

**重要概念区分**：
- **BlockExpr**: 只用于 `if` 和 `loop` 的主体，语法层面强制要求
- **ScopeExpr**: 普通作用域，用于函数体、匿名闭包、独立的 `{...}` 表达式

两者的返回值行为相同：
```prim
{ let x = 1; x }      // use_tail=true, 返回 x (1)
{ let x = 1; x; }     // use_tail=false, 返回 null (有分号)
{ let x = 1; }        // use_tail=false, 返回 null
```

**语法区别**：
- `if cond {...}` 和 `loop {...}` 中的 `{...}` 强制解析为 **BlockExpr**
- 其他位置的 `{...}` 根据内容判断：
  - `{}` → 空字典 (`DictExpr`)
  - `{expr: expr}` → 字典 (`DictExpr`)
  - `{stmt; ...}` → 作用域 (`ScopeExpr`)

### 3. 引用 (Reference) 的独立性

**关键设计决策**: `RefExpr` 是完全独立的节点类型，**不能隐式转换为表达式**。

允许使用引用的位置：
```prim
let x = &y;              // ✅ let 右侧
func(&x, &y);            // ✅ 函数参数
[&x, &y, expr];          // ✅ list 元素
(&x, &y, expr);          // ✅ tuple 元素
{"k": &v};               // ✅ dict 值
return &x;               // ✅ return 值
break &x;                // ✅ break 值
a = &b;                  // ✅ 赋值右侧
```

不允许的情况：
```prim
&func();                 // ❌ 独立语句（语义不明确）
let x = &(&y);           // ❌ 引用的引用
```

**实现方式**: 在需要支持引用的位置，AST children 可以混合 `Expr` 和 `RefExpr` 节点。

### 4. Let 语句的双重语义

**导入 vs 定义** - 通过 `is_import` 标记区分：

| 语法 | `is_import` | 语义 | AST 结构 |
|------|------------|------|----------|
| `let x;` | `true` | 导入外部变量 | `children: [target_list]` |
| `let x, y;` | `true` | 导入多个外部变量 | `children: [target_list]` |
| `let x = expr;` | `false` | 定义新变量 | `children: [target_list, rhs]` |
| `let x, y = expr;` | `false` | 解包赋值 | `children: [target_list, rhs]` |

**重要**: `let x;` 不是"声明未初始化变量"，而是**闭包捕获机制**，引用外部同名变量。

### 5. 赋值是二元运算符

赋值 `=` 是右结合的二元运算符，与其他运算符类似：
```prim
a = b = c;    // 等价于 a = (b = c)
```

在 AST 中，赋值使用 `BinaryExpr` 节点（不需要单独的 `AssignExpr`）。

---

## 完整 BNF 语法

### 程序结构

```bnf
program          ::= stmt_list_opt END

stmt_list_opt    ::= ε | stmt_list

stmt_list        ::= stmt
                   | stmt_list ";" stmt
                   | stmt_list ";"          // 允许尾随分号

stmt             ::= let_stmt
                   | del_stmt
                   | break_stmt
                   | return_stmt
                   | expr_stmt

expr_stmt        ::= expr
```

**注意**: 分号是语句分隔符，不是终止符。

### Let 语句

```bnf
let_stmt         ::= "let" let_target_list                    // 导入
                   | "let" let_target_list "=" expr_or_ref    // 定义

let_target_list  ::= let_target
                   | let_target_list "," let_target

let_target       ::= ident type_hint_opt                      // 普通变量
                   | "&" ident type_hint_opt                  // 引用变量

type_hint_opt    ::= ε | ":" type_hint

type_hint        ::= ident
                   | type_hint "|" ident                      // 联合类型
```

**AST 结构**:
```cpp
LetStmt {
    is_import: bool,  // true: 导入, false: 定义
    children: [
        LetTargetList,
        rhs (optional)  // 只有 is_import=false 时存在
    ]
}

LetTarget {
    token: identifier,
    is_ref: bool,
    children: [type_hint (optional)]
}
```

**语义示例**:
```prim
// 导入外部变量（闭包捕获）
let x;                    // is_import=true, 引用外部 x
let x, y;                 // is_import=true, 引用外部 x 和 y
let &z;                   // is_import=true, z 是外部变量的引用

// 定义新变量
let a = 42;               // is_import=false, 定义 a
let b: i32 = 10;          // is_import=false, 带类型提示
let c, d = (1, 2);        // is_import=false, 解包赋值
let &e = &x;              // is_import=false, e 是 x 的引用
```

### Del 语句

```bnf
del_stmt         ::= "del" ident_list

ident_list       ::= ident
                   | ident_list "," ident
```

**限制**: 只能删除标识符，不能删除表达式或引用。

```prim
del x;              // ✅
del x, y, z;        // ✅
del x + y;          // ❌ 语法错误
del &x;             // ❌ 语法错误
```

### Break 语句

```bnf
break_stmt       ::= "break"
                   | "break" label
                   | "break" expr_or_ref
                   | "break" label expr_or_ref

label            ::= LABEL                  // `label_name`
```

**AST 结构**:
```cpp
BreakStmt {
    token: label (optional),
    children: [value (optional)]  // Expr 或 RefExpr
}
```

**示例**:
```prim
break;                      // 返回 null
break `outer`;              // 跳出标签为 `outer` 的循环
break 42;                   // 返回 42
break `outer` (1, 2);       // 跳出 `outer` 并返回 tuple
break &x;                   // 返回引用
```

### Return 语句

```bnf
return_stmt      ::= "return"
                   | "return" expr_or_ref
```

**AST 结构**:
```cpp
ReturnStmt {
    children: [value (optional)]  // Expr 或 RefExpr
}
```

**重要**: 不支持逗号分隔的多返回值，必须使用 tuple:
```prim
return x, y;        // ❌ 语法错误
return (x, y);      // ✅ 返回 tuple
```

---

## 表达式层次结构

### 优先级层次（从低到高）

```bnf
expr             ::= assignment_expr

// 1. 赋值（右结合）
assignment_expr  ::= logical_or_expr
                   | logical_or_expr "=" assignment_expr

// 2. 逻辑或
logical_or_expr  ::= logical_and_expr
                   | logical_or_expr "||" logical_and_expr

// 3. 逻辑与
logical_and_expr ::= equality_expr
                   | logical_and_expr "&&" equality_expr

// 4. 相等性
equality_expr    ::= relational_expr
                   | equality_expr "==" relational_expr
                   | equality_expr "!=" relational_expr

// 5. 关系比较
relational_expr  ::= additive_expr
                   | relational_expr "<" additive_expr
                   | relational_expr ">" additive_expr
                   | relational_expr "<=" additive_expr
                   | relational_expr ">=" additive_expr

// 6. 加减
additive_expr    ::= multiplicative_expr
                   | additive_expr "+" multiplicative_expr
                   | additive_expr "-" multiplicative_expr

// 7. 乘除模
multiplicative_expr ::= unary_expr
                      | multiplicative_expr "*" unary_expr
                      | multiplicative_expr "/" unary_expr
                      | multiplicative_expr "%" unary_expr

// 8. 一元运算符（右结合）
unary_expr       ::= postfix_expr
                   | "!" unary_expr
                   | "+" unary_expr
                   | "-" unary_expr

// 9. 后缀运算符
postfix_expr     ::= primary_expr
                   | postfix_expr "(" expr_or_ref_list_opt ")"    // 函数调用
                   | postfix_expr "[" expr "]"                     // 索引
                   | postfix_expr "." ident                        // 字段访问

// 10. 基本表达式
primary_expr     ::= literal
                   | ident
                   | tuple_expr
                   | list_expr
                   | dict_expr
                   | scope_expr
                   | if_expr
                   | loop_expr
                   | unnamed_prim
                   | named_prim
                   | "(" expr ")"          // 括号表达式（直接解包）
                   | "(" ")"               // unit/null
```

### 字面量

```bnf
literal          ::= INT_DEC | INT_HEX | INT_OCT | INT_BIN
                   | FLOAT_DEC
                   | STRING
                   | "true" | "false" | "null"
```

**注意**: `()` 在 `primary_expr` 中处理，解析为 `null` 字面量。

---

## 容器表达式

### Tuple 表达式

```bnf
tuple_expr       ::= "(" expr_or_ref "," ")"                          // 单元素
                   | "(" expr_or_ref_list_nonempty ")"                // 多元素
                   | "(" expr_or_ref_list_nonempty "," ")"            // 尾随逗号

expr_or_ref_list_nonempty ::= expr_or_ref "," expr_or_ref
                            | expr_or_ref_list_nonempty "," expr_or_ref
```

**特殊规则**:
- `()` → `Literal` (null)
- `(expr)` → 直接解包为 `expr`（不是 tuple）
- `(expr,)` → `TupleExpr`，`trailing_comma=true`
- `(a, b)` → `TupleExpr`，`trailing_comma=false`

**实现提示**: 在 parser 中处理括号消除：
```cpp
// primary_expr 规则
"(" ")" { $$ = create_literal_null(); }
"(" expr ")" { $$ = $2; }  // 直接解包，不创建 tuple
```

### List 表达式

```bnf
list_expr        ::= "[" expr_or_ref_list_opt "]"

expr_or_ref_list_opt ::= ε | expr_or_ref_list

expr_or_ref_list ::= expr_or_ref
                   | expr_or_ref_list "," expr_or_ref
                   | expr_or_ref_list ","              // 允许尾随逗号
```

**示例**:
```prim
[]                  // 空列表
[1, 2, 3]          // 普通列表
[&x, y, &z]        // 混合引用和值
[1, 2, 3,]         // 尾随逗号（可选）
```

### Dict 表达式

```bnf
dict_expr        ::= "{" "}"                          // 空字典
                   | "{" dict_pair_list "}"
                   | "{" dict_pair_list "," "}"       // 尾随逗号

dict_pair_list   ::= dict_pair
                   | dict_pair_list "," dict_pair

dict_pair        ::= expr ":" expr_or_ref
```

**重要**: `{}` 在**任何位置**都解析为空字典（除了 if/loop 后会报错）。

```prim
{}                          // 空字典
{"key": "value"}           // 普通字典
{1: &x, "name": y}         // 键必须是表达式，值可以是引用
{"a": 1, "b": 2,}          // 尾随逗号
```

**AST 结构**:
```cpp
DictExpr {
    children: [DictPair, DictPair, ...]
}

DictPair {
    children: [key_expr, value_expr_or_ref]
}
```

---

## Block 和 Scope 表达式

### Scope 表达式

```bnf
scope_expr       ::= "{" stmt_list "}"
```

**语义**:
- 普通作用域，用于函数体、匿名闭包、独立的 `{...}` 表达式
- 外部符号默认不可见，需要 `let` 显式导入
- 返回值由 `use_tail` 决定

**AST 结构**:
```cpp
ScopeExpr {
    use_tail: bool,    // 是否使用最后表达式作为返回值
    children: [stmt1, stmt2, ...]
}
```

**返回值规则**:
```prim
{ let x = 1; x }        // use_tail=true, 返回 1
{ let x = 1; x; }       // use_tail=false, 返回 null (有分号)
{ let x = 1; }          // use_tail=false, 返回 null
{ 42 }                  // use_tail=true, 返回 42
```

**实现方式**: 检查 `stmt_list` 的最后一项：
- 如果是 `ExprStmt` 且后面没有分号 → `use_tail=true`
- 否则 → `use_tail=false`

### Block 表达式

```bnf
block_expr       ::= "{" stmt_list "}"
```

**语义**:
- 只用于 `if` 和 `loop` 的主体
- 语法强制要求，不能为空
- 返回值规则与 `ScopeExpr` 完全相同

**区别总结**:
| 特性 | BlockExpr | ScopeExpr |
|------|-----------|-----------|
| 用途 | if/loop 主体 | 函数体、闭包、独立表达式 |
| 是否可为空 | ❌ 不能 | ✅ 可以（但会变成空字典） |
| 返回值 | 相同 | 相同 |
| 作用域 | 新作用域 | 新作用域 |

---

## 控制流表达式

### If 表达式

```bnf
if_expr          ::= "if" expr block_expr
                   | "if" expr block_expr "else" block_expr
                   | "if" expr block_expr "else" if_expr
```

**AST 结构**:
```cpp
IfExpr {
    children: [
        cond_expr,
        then_block,
        else_expr (optional)  // BlockExpr 或另一个 IfExpr
    ]
}
```

**示例**:
```prim
if x > 0 { "positive" }
if x > 0 { "positive" } else { "non-positive" }
if x > 0 { "positive" } else if x < 0 { "negative" } else { "zero" }
```

**错误示例**:
```prim
if x > 0 {}             // ❌ block 不能为空
if x > 0 x + 1          // ❌ 必须使用 block
```

### Loop 表达式

```bnf
loop_expr        ::= "loop" block_expr
                   | "loop" label block_expr

label            ::= LABEL                    // `label_name`
```

**AST 结构**:
```cpp
LoopExpr {
    token: label (optional),
    children: [body_block]
}
```

**示例**:
```prim
loop { if done { break; } }
loop `outer` { loop `inner` { break `outer` 42; } }
```

**返回值**: loop 的返回值是 `break` 语句的值。

---

## Prim 表达式（函数）

### 匿名 Prim (Unnamed Prim)

```bnf
unnamed_prim     ::= "@" scope_expr
                   | decorators "@" scope_expr

decorators       ::= "@" ident
                   | decorators "@" ident
```

**AST 结构**:
```cpp
UnnamedPrim {
    children: [
        DecoratorList,  // 可能为空
        ScopeExpr
    ]
}
```

**示例**:
```prim
@{ let x = 1; x }                    // 简单闭包
@decorator @{ let x = 1; x }         // 带装饰器
@dec1 @dec2 @dec3 @{ ... }           // 多个装饰器
```

**语义**: `@` 使 scope 的返回值变为其闭包空间（而不是最后表达式的值）。

### 命名 Prim (Named Prim / Function)

```bnf
named_prim       ::= "$" ident "(" param_list_opt ")" type_hint_opt impl
                   | decorators "$" ident "(" param_list_opt ")" type_hint_opt impl

impl             ::= scope_expr
                   | "@" scope_expr

param_list_opt   ::= ε | param_list

param_list       ::= param
                   | param_list "," param
                   | param_list ","                // 允许尾随逗号

param            ::= ident type_hint_opt
                   | "&" ident type_hint_opt
```

**AST 结构**:
```cpp
NamedPrim {
    token: function_name,
    children: [
        DecoratorList,
        ParamList,
        TypeHint (optional),
        impl_scope
    ]
}

Param {
    token: param_name,
    is_ref: bool,
    children: [TypeHint (optional)]
}
```

**示例**:
```prim
// 基本函数
$add(x: i32, y: i32): i32 { x + y }

// 引用参数
$modify(&value, delta) { value = value + delta; }

// 闭包实现
$Point(x, y) @{
    let x, y;
    $distance() { ((x * x) + (y * y)) }
}

// 带装饰器
@inline $fast_add(a, b) { a + b }
```

**impl 的两种形式**:
1. `scope_expr`: 普通函数体，返回最后表达式的值
2. `@ scope_expr`: 返回闭包空间（类似结构体）

---

## 引用表达式

```bnf
ref_expr         ::= "&" ref_target

ref_target       ::= ident
                   | literal
                   | call_expr
                   | tuple_expr
                   | list_expr
                   | dict_expr
                   | "(" expr ")"
```

**AST 结构**:
```cpp
RefExpr {
    children: [target]
}
```

**示例**:
```prim
&x                      // 变量引用
&42                     // 字面量引用
&func()                 // 函数调用结果的引用
&(x + y)               // 表达式引用
&[1, 2, 3]             // 列表引用
```

**不允许**:
```prim
&(&x)                   // ❌ 引用的引用
```

---

## 运算符优先级

### Bison 声明（从低到高）

```yacc
%right "="                              // 1. 赋值（右结合）
%left "||"                              // 2. 逻辑或
%left "&&"                              // 3. 逻辑与
%left "==" "!="                         // 4. 相等性
%left "<" ">" "<=" ">="                 // 5. 关系比较
%left "+" "-"                           // 6. 加减
%left "*" "/" "%"                       // 7. 乘除模
%right "!" UNARY_PLUS UNARY_MINUS       // 8. 一元运算符（右结合）
%left "(" ")" "[" "]" "."               // 9. 后缀运算符
```

### 优先级表

| 优先级 | 运算符 | 结合性 | 说明 |
|--------|--------|--------|------|
| 1 | `=` | 右结合 | 赋值 |
| 2 | `||` | 左结合 | 逻辑或 |
| 3 | `&&` | 左结合 | 逻辑与 |
| 4 | `==`, `!=` | 左结合 | 相等性 |
| 5 | `<`, `>`, `<=`, `>=` | 左结合 | 关系比较 |
| 6 | `+`, `-` | 左结合 | 加减 |
| 7 | `*`, `/`, `%` | 左结合 | 乘除模 |
| 8 | `!`, `+`, `-` (unary) | 右结合 | 一元运算 |
| 9 | `()`, `[]`, `.` | 左结合 | 后缀运算 |

### 示例

```prim
-x * y              // 等价于 (-x) * y
a = b = c           // 等价于 a = (b = c)
x + y * z           // 等价于 x + (y * z)
a && b || c         // 等价于 (a && b) || c
!x && y             // 等价于 (!x) && y
x.y[0]()            // 等价于 ((x.y)[0])()
```

---

## AST 节点设计

### 节点类型完整列表

```cpp
enum class NodeType {
    // 字面量和标识符
    Literal,        // 42, "str", true, false, null, ()
    Identifier,     // x, foo
    
    // 运算符表达式
    BinaryExpr,     // a + b, a && b, a = b
    UnaryExpr,      // !a, -a, +a
    
    // 后缀表达式
    CallExpr,       // func(args)
    IndexExpr,      // arr[idx]
    FieldExpr,      // obj.field
    
    // 容器
    TupleExpr,      // (a, b, c)
    ListExpr,       // [a, b, c]
    DictExpr,       // {k: v, ...}
    DictPair,       // k: v
    
    // 块和控制流
    BlockExpr,      // if/loop 中的 {...}
    ScopeExpr,      // 普通的 {...}
    IfExpr,         // if cond {...} else {...}
    LoopExpr,       // loop {...}
    
    // 语句
    LetStmt,        // let x = expr
    DelStmt,        // del x, y
    BreakStmt,      // break
    ReturnStmt,     // return
    ExprStmt,       // expr;
    
    // Prim（函数）
    UnnamedPrim,    // @{...}
    NamedPrim,      // $name(...) {...}
    Param,          // 函数参数
    
    // 引用
    RefExpr,        // &expr
    
    // Let Target
    LetTarget,      // let 的目标
    
    // 辅助节点
    TypeHint,       // i32 | str | null
    
    // 列表节点
    StmtList,       // 语句列表
    ExprList,       // 表达式/引用列表
    LetTargetList,  // let 目标列表
    IdentList,      // 标识符列表
    ParamList,      // 参数列表
    DecoratorList,  // 装饰器列表
    
    // 根节点
    Program,        // 整个程序
};
```

### 关键字段说明

```cpp
struct ASTNode {
    NodeType type;
    std::vector<ASTNode> children;
    const Token* token = nullptr;
    
    // 辅助字段
    bool is_ref = false;         // Param/LetTarget 是否是引用
    bool use_tail = false;       // Block/Scope 是否使用尾部表达式
    bool trailing_comma = false; // Tuple 尾随逗号
    bool is_import = false;      // Let 是导入还是定义
};
```

---

## 边界情况与特殊规则

### 1. 空 `{}` 的处理

**规则**: `{}` 永远解析为空字典，除了在 if/loop 后会报错。

```prim
let x = {};             // ✅ 空字典
{};                     // ✅ 空字典表达式语句
if cond {}              // ❌ 语法错误：block 不能为空
loop {}                 // ❌ 语法错误：block 不能为空
$func() {}              // ❌ 语法错误：函数体不能为空
@{}                     // ❌ 语法错误：闭包不能为空
```

**实现**: 在 parser 中对 if/loop/prim 后的 block 进行非空检查。

### 2. 括号消除

**规则**: `(expr)` 直接解包为 `expr`，不创建额外节点。

```prim
()                      // Literal(null)
(42)                    // Literal(42)，不是 tuple
(x + y)                // BinaryExpr，不是 tuple
(x,)                   // TupleExpr(trailing_comma=true)
(x, y)                 // TupleExpr(trailing_comma=false)
```

**实现**:
```yacc
primary_expr
    : "(" ")" { $$ = create_literal_null(); }
    | "(" expr ")" { $$ = $2; }  // 直接返回 expr
    | tuple_expr { $$ = $1; }
    ;
```

### 3. Let 的导入与定义

**判断规则**: 是否有 `=` 右侧。

```prim
let x;                  // is_import=true, 无 rhs
let x, y;               // is_import=true, 无 rhs
let x = 42;             // is_import=false, rhs=Literal(42)
let x, y = (1, 2);      // is_import=false, rhs=TupleExpr
```

**语义差异**:
- **导入**: 引用外部作用域的同名变量（运行时检查存在性）
- **定义**: 创建新变量（可能遮盖外部变量）

### 4. 引用的混合使用

**规则**: 在允许引用的位置，children 可以混合 `Expr` 和 `RefExpr`。

```prim
let x = &y;             // children: [LetTargetList, RefExpr]
[1, &x, 3]              // children: [Literal, RefExpr, Literal]
func(a, &b, c)          // children: [callee, Identifier, RefExpr, Identifier]
```

**不需要 ExprOrRef 包装节点**，直接混合即可。

### 5. 装饰器的两种用法

**用法 1**: 装饰匿名 prim
```prim
@dec1 @dec2 @{...}      // UnnamedPrim
```

**用法 2**: 装饰命名 prim（等价于 Python）
```prim
@decorator
$func() {...}

// 等价于
$func() {...}
func = decorator(func);
```

**AST 表示**: 装饰器列表存储在 Prim 节点内部。

### 6. 类型提示的变长数组

```prim
let x: i32 = 1;
let y: i32 | str = "hello";
let z: i32 | str | null = null;
```

**AST 结构**:
```cpp
TypeHint {
    children: [Identifier("i32"), Identifier("str"), Identifier("null")]
}
```

### 7. 函数体的两种形式

```prim
// 形式 1: 普通函数体
$func() { expr }

// 形式 2: 闭包函数体（返回闭包空间）
$func() @{ let x; }
```

**区别**:
- 形式 1: 返回表达式的值
- 形式 2: 返回闭包对象（类似结构体）

### 8. 尾随逗号

**允许尾随逗号的位置**:
```prim
(a, b, c,)              // Tuple
[a, b, c,]              // List
{a: b, c: d,}           // Dict
func(a, b,)             // Call
$func(a, b,) {}         // Param list
let a, b, = (1, 2);     // Let target list（语法上允许，语义上可能奇怪）
```

**实现**: 在 grammar 规则中添加 `| list ","` 的可选分支。

### 9. 多返回值

**不支持**: `return x, y;`

**必须使用 tuple**: `return (x, y);`

```prim
$divide(a, b) {
    (a / b, a % b)      // ✅ 返回 tuple
}

$bad() {
    return 1, 2;        // ❌ 语法错误
}
```

### 10. 运算符重载（TODO）

暂不支持特殊函数名：
```prim
$+(other) { ... }       // TODO: 加法运算符重载
$[](index) { ... }      // TODO: 索引运算符重载
$() (args) { ... }      // TODO: 调用运算符重载
```

---

## 实现指南

### 推荐的实现方式

1. **递归下降 Parser**: 手写递归下降，更灵活
2. **Bison/Yacc**: 如果熟悉工具链，可以用 Bison

### Parser 实现步骤

1. **词法分析**: 使用 re2c 生成 lexer（已完成）
2. **语法分析**: 实现 parser，构建 AST
3. **语义检查**: 检查类型、作用域等
4. **代码生成**: 生成字节码或解释执行

### AST 构建辅助函数（示例）

```cpp
// 创建节点的辅助函数
ASTNode create_literal(const Token* tok);
ASTNode create_identifier(const Token* tok);
ASTNode create_binary_expr(const Token* op, ASTNode left, ASTNode right);
ASTNode create_let_stmt(ASTNode targets, ASTNode* rhs, bool is_import);
ASTNode create_scope_expr(ASTNode stmt_list, bool use_tail);
// ...
```

### 错误恢复策略

遇到语法错误时：
1. 报告错误位置和原因
2. 跳过到下一个同步点（如分号、右括号）
3. 继续解析后续内容
4. 收集所有错误，一次性报告

常见错误点：
- 括号不匹配
- 缺少分号
- if/loop 后缺少 block
- 函数体为空
- let 后缺少目标或表达式

---

## 测试用例

### 基本表达式

```prim
// 字面量
42;
3.14;
"hello";
true;
false;
null;
();

// 运算符
1 + 2 * 3;
(1 + 2) * 3;
a = b = c;
x && y || z;
!flag;
```

### Let 语句

```prim
// 定义
let x = 42;
let y: i32 = 10;
let a, b = (1, 2);

// 导入
let outer_var;
let x, y;

// 引用
let &ref = &x;
let &imported_ref;
```

### 容器

```prim
// Tuple
(1, 2, 3);
(x,);
();

// List
[1, 2, 3];
[&x, y, &z];
[];

// Dict
{"name": "Alice", "age": 30};
{};
```

### 控制流

```prim
// If
if x > 0 { "pos" } else { "neg" };
if a { b } else if c { d } else { e };

// Loop
loop { break; };
loop `outer` { loop `inner` { break `outer` 42; } };
```

### 函数

```prim
// 基本函数
$add(x, y) { x + y };

// 引用参数
$modify(&value, delta) { value = value + delta; };

// 闭包
@{ let x = 1; x };

// 结构体
$Point(x, y) @{ let x, y; };
```

### 复杂示例

```prim
// 向量运算
$Vector(x, y) @{
    let x, y;
    
    $+(other) {
        Vector(x + other.x, y + other.y)
    }
    
    $len() {
        ((x * x) + (y * y))
    }
};

// 高阶函数
$map(list, f) {
    let result = [];
    loop {
        if list.is_empty() { break result; };
        result.push(f(list[0]));
    }
};
```

---

## 总结

### 语法特点

1. **表达式优先**: 几乎所有结构都是表达式
2. **明确的 Block vs Scope**: 语法层面区分用途
3. **引用独立性**: `RefExpr` 不能隐式转换
4. **Let 的双重语义**: 导入外部 vs 定义新变量
5. **统一的运算符**: 赋值也是二元运算符
6. **灵活的函数**: 普通函数和闭包的统一

### 设计优势

- **无歧义**: 每个语法结构有明确解析
- **可扩展**: 预留语法空间（运算符重载）
- **可读性**: 符合直觉的语法
- **实用性**: 支持常见编程模式

### 实现建议

1. 先实现核心语法（表达式、语句）
2. 再实现容器（tuple、list、dict）
3. 最后实现函数和闭包
4. 逐步添加装饰器、类型提示等高级特性

---

**文档版本**: 1.0  
**最后更新**: 2024  
**状态**: 完整规范
