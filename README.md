# Prim 语言规范

Prim 是一个 JIT 友好的动态解释型语言，具有独特的设计哲学：**一切皆 Prim**。

---

## 目录

1. [核心概念](#核心概念)
2. [变量与槽系统](#变量与槽系统)
3. [闭包（Closure）](#闭包closure)
4. [Prim：可执行代码](#primprimitives)
5. [控制流](#控制流)
6. [函数与结构体](#函数与结构体)
7. [运算符重载](#运算符重载)
8. [类型系统](#类型系统)
9. [类型提示](#类型提示type-hints)
10. [内置容器类型](#内置容器类型)
11. [快速参考](#快速参考)

---

## 核心概念

Prim 的设计基于两个核心抽象：

- **Prim (Primitive)**：可执行的代码片段，负责将一个闭包转换为另一个闭包
- **Closure**：静态状态的集合，由槽（slot）和符号（symbol）组成

**设计哲学**：程序 = 可执行逻辑（Prim） + 静态状态（Closure）

---

## 变量与槽系统

### 槽（Slot）与符号（Symbol）

Prim 使用**槽系统**来管理内存和引用：

- **槽**：实际存储数据的内存空间，带有引用计数
- **符号**：变量名，绑定到槽上
- 多个符号可以绑定到同一个槽

```prim
let a = 1;        // 创建槽存储 1，符号 a 绑定到该槽
let b = &a;       // 符号 b 绑定到 a 的槽（引用）
a = 2;            // 修改槽的值
b                 // 2（b 和 a 指向同一个槽）
```

### 值语义 vs 引用语义

默认情况下，赋值是**值语义**（拷贝）：

```prim
let a = "hello";
let b = a;        // 创建新槽，拷贝 "hello"
let c = &a;       // 引用 a 的槽，不拷贝
```

| 操作 | 行为 |
|------|------|
| `let b = a` | 拷贝 a 的值到新槽 |
| `let b = &a` | 引用 a 的槽 |

### 遮盖（Shadowing）

`let` 可以重新定义同名变量，创建新的绑定：

```prim
let a = 1;
let a = "hello";  // 遮盖原变量，原槽引用计数 -1
let b = &a;       // b 绑定到 "hello" 槽
let a = "world";  // a 绑定到新槽，b 仍指向 "hello"
```

### 右值优化

字面量和临时值会被优化为原地构造：

```prim
let a = 1;        // 等价于 let a = &1;（优化掉拷贝）
```

### 删除符号

```prim
let a = 1;
del a;            // 删除符号 a，槽引用计数 -1
```

---

## 闭包（Closure）

### 基本概念

闭包是一个**符号-槽的集合**，可以封装数据和状态。

```prim
let obj = @{
    let x = 1;
    let y = 2;
};

obj.x;  // 1
obj.y;  // 2
```

- `@` 运算符：将 Prim 的返回值改为其闭包空间
- 闭包只包含内部 `let` 定义的符号

### 闭包的拷贝行为

闭包拷贝时：
- **值**：深拷贝
- **引用**：保持引用关系

```prim
let x = 1;
let y = 2;

let a = @{
    let x;      // 等价于 let x = x;（值）
    let &y;     // 等价于 let y = &y;（引用）
};

let b = a;      // 拷贝闭包
// a.x 和 b.x 指向不同的槽
// a.y 和 b.y 指向同一个槽
```

### 基本类型

基本类型（数字、字符串等）被视为**不可变的基础闭包**。

---

## Prim（Primitives）

### 一切皆 Prim

在 Prim 中，所有的代码都是 Prim：

```prim
let a = 1;           // let 语句是 Prim
a += 1;              // 表达式是 Prim
print(a);            // 函数调用是 Prim

if a > 10 {          // if 语句是 Prim
    "greater"
}

{                    // scope 是 Prim
    let b = 2;
}

loop {               // loop 是 Prim
    break;
}
```

### 捕获（Capture）：`$`

使用 `$` 可以将 Prim **延迟执行**，变成可调用的函数：

```prim
$foo(x) {
    x + 1
}

foo(10);  // 11
```

- `$` 捕获紧接着的一个 Prim
- 被捕获的 Prim 不会立即执行，而是成为函数
- 可以使用 `return` 提前返回

### 匿名 Prim

所有 Prim 最初都是匿名的，`$` 只是给它命名：

```prim
// 匿名 Prim（立即执行）
{
    print("hello");
}

// 命名 Prim（延迟执行）
$greet() {
    print("hello");
}
greet();
```

### Scope

Scope 是一个独立的作用域，可以有返回值：

```prim
let result = {
    let x = 1;
    let y = 2;
    x + y  // 返回值
};
result;  // 3
```

- Scope 可见父作用域的符号
- Scope 内定义的符号对外不可见
- 使用 `@{}` 可以暴露闭包空间

---

## 控制流

### `if` 语句

```prim
if a > b {
    "greater"
} else if a == b {
    "equal"
} else {
    "less"
}
```

### `loop` 语句

```prim
loop {
    print("hello");
}
```

#### 带标签的循环

```prim
loop `outer` {
    loop `inner` {
        if condition {
            break `outer`;  // 跳出外层循环
        }
    }
}
```

### `break` 规则

- `break` 可以指定跳出的循环标签
- `break` **不允许跨越命名 Prim**（函数边界）

---

## 函数与结构体

### 函数定义

```prim
$add(x, y) {
    x + y
}

add(1, 2);  // 3
```

### 参数传递

需要显式指定值传递还是引用传递：

```prim
$modify_value(x) {
    x += 1;      // 修改局部拷贝
}

$modify_ref(&x) {
    x += 1;      // 修改原槽
}

let a = 10;
modify_value(a);   // a 仍为 10
modify_ref(&a);    // a 变为 11
```

| 参数形式 | 语义 | 等价于 |
|---------|------|-------|
| `(x)` | 值传递 | `let x = x;` |
| `(&x)` | 引用传递 | `let x = &x;` |

### 闭包捕获规则

当 Prim 被捕获后，父作用域的符号默认**按值捕获**：

```prim
let x = 1;
$foo() {
    x;         // 错误！需要显式声明
    let &x;    // 正确：引用父作用域的 x
}
```

### 结构体

结构体是使用 `@` 修饰的函数，返回闭包而非普通值：

```prim
// 方式 1：使用装饰器
@struct $Point(x, y) {
    let x = x;
    let y = y;
}

// 方式 2：直接使用 @
$Point(x, y) @{
    let x, y;
}

let p = Point(1, 2);
p.x;  // 1
p.y;  // 2
```

#### 装饰器原理

`struct` 装饰器的定义：

```prim
$struct(prim) {
    @prim  // 将 prim 的返回值改为闭包
}
```

### 捕获的嵌套

捕获本身是 Prim，可以嵌套捕获：

```prim
$outer() $inner(x) {
    x + 1
}

outer();   // 执行 outer，定义 inner
inner(5);  // 6
```

---

## 运算符重载

### 基本运算符重载

```prim
$Point(x, y) @{
    let x, y;
    
    $+(other) {
        Point(&x + other.x, &y + other.y)
    }
}

let p1 = Point(1, 2);
let p2 = Point(3, 4);
let p3 = p1 + p2;
p3.x;  // 4
p3.y;  // 6
```

### 调用运算符 `()`

闭包默认不可调用，可以定义 `$()` 使其可调用：

```prim
$Callable(value) @{
    let value;
    
    $()() {
        print("Called with value: ");
        print(&value);
    }
}

let obj = Callable(42);
obj();  // 输出：Called with value: 42
```

---

## 内存管理

Prim 使用**引用计数**进行自动内存管理：

- 槽维护引用计数
- 引用计数为 0 时自动释放
- 拷贝和引用明确区分，避免意外行为

**注意**：当前版本不处理循环引用，需要程序员注意避免。

---

## 设计哲学总结

1. **一切皆 Prim**：统一的执行模型
2. **显式引用语义**：明确区分值和引用
3. **槽系统**：简化内存管理，JIT 友好
4. **闭包作为数据结构**：统一的数据封装方式
5. **捕获即函数**：优雅的函数定义方式

---

## 类型系统

### 动态类型与类型检查

Prim 是一个**弱类型**的动态语言。虽然变量可以随时改变类型，但 Prim 提供了类型提示和运行时检查机制。

### `isinstance` 函数

用于判断一个闭包是否由特定的 Prim 创建：

```prim
$Point(x, y) @{
    let x, y;
}

let p = Point(1, 2);
isinstance(p, Point);  // true

$foo() {
    let x = 1;
}
let a = (@foo)();
isinstance(a, foo);    // true
```

**原理**：每个闭包都记录了创建它的 Prim，`isinstance` 检查这个关系。

---

## 类型提示（Type Hints）

### 基本语法

Prim 支持**可选的类型提示**，在运行时进行检查：

```prim
let a: i32 = 1;      // 正确
let b: str = "hi";   // 正确
let c: i32 = "no";   // 运行时错误
```

### 函数参数类型

```prim
$add(x: i32, y: i32): i32 {
    x + y
}

add(1, 2);        // 正确
add("a", "b");    // 运行时错误
```

### 联合类型

使用 `|` 表示联合类型：

```prim
let value: i32 | str = 1;      // 正确
value = "hello";               // 正确
value = 3.14;                  // 错误（不是 i32 或 str）

$process(data: i32 | str | none) {
    if data == none {
        "no data"
    } else {
        data
    }
}
```

### 内置类型

| 类型 | 说明 |
|------|------|
| `i32` | 32 位有符号整数 |
| `i64` | 64 位有符号整数 |
| `u32` | 32 位无符号整数 |
| `u64` | 64 位无符号整数 |
| `f32` | 32 位浮点数 |
| `f64` | 64 位浮点数 |
| `str` | 字符串 |
| `bool` | 布尔值 |
| `none` | 空值 |
| `List` | 列表类型 |
| `Dict` | 字典类型 |
| `Tuple` | 元组类型 |

---

## 内置容器类型

### List（列表）

**变长、有序**的容器，持有元素的**引用**：

```prim
let list = [1, 2, 3, 4, 5];

// 访问元素
list[0];          // 1
list[-1];         // 5（负索引）

// 修改元素
list[0] = 10;

// 追加元素
list.push(6);

// 长度
list.len();       // 6

// 迭代
loop `i` in list {
    print(i);
}
```

#### 列表操作

```prim
let a = [1, 2, 3];
let b = [4, 5, 6];

// 拼接
let c = a + b;         // [1, 2, 3, 4, 5, 6]

// 切片
a[1:3];                // [2, 3]
a[:2];                 // [1, 2]
a[2:];                 // [3]

// 包含检查
2 in a;                // true
```

#### 引用语义

List 持有对元素的**引用**：

```prim
let x = 1;
let list = [&x];    // 列表持有 x 的引用
x = 2;
list[0];            // 2（跟随 x 的变化）
```

---

### Dict（字典）

**键值对**的容器，使用 `{}` 构造：

```prim
let dict = {
    "name": "Alice",
    "age": 30,
    "city": "Beijing"
};

// 访问
dict["name"];           // "Alice"
dict.name;              // 等价语法糖

// 修改
dict["age"] = 31;

// 添加
dict["email"] = "alice@example.com";

// 检查键是否存在
"name" in dict;         // true

// 删除
del dict["city"];

// 获取所有键
dict.keys();            // ["name", "age", "email"]

// 获取所有值
dict.values();          // ["Alice", 31, "alice@example.com"]
```

#### 可哈希性

**只有不可变类型可以作为 Key**：

```prim
let dict = {
    "str_key": 1,       // ✅ str 可哈希
    42: "value",        // ✅ 数字可哈希
    true: "bool",       // ✅ bool 可哈希
};

// ❌ 不可哈希的类型
let bad_dict = {
    [1, 2]: "list",     // 错误！list 不可哈希
    {"a": 1}: "dict",   // 错误！dict 不可哈希
    (1, 2): "tuple",    // 错误！tuple 不可哈希
};
```

#### 引用语义

Dict 持有 Key 和 Value 的**引用**：

```prim
let name = "Alice";
let age = 30;
let dict = { "name": &name, "age": &age };

name = "Bob";
dict["name"];           // "Bob"
```

---

### Tuple（元组）

**不可变、有序**的容器，使用 `()` 构造：

```prim
let tuple = (1, "hello", 3.14);

// 访问
tuple[0];               // 1
tuple[1];               // "hello"

// 长度
tuple.len();            // 3

// 元组不可修改
tuple[0] = 2;           // 错误！
```

#### 解包

Tuple 支持解包赋值：

```prim
let (x, y, z) = (1, 2, 3);
x;  // 1
y;  // 2
z;  // 3

// 忽略部分值
let (a, _, c) = (10, 20, 30);
a;  // 10
c;  // 30
```

#### 多返回值

函数返回多个值等价于返回 Tuple：

```prim
$divide(a, b) {
    (a / b, a % b)  // 返回商和余数
}

let (quotient, remainder) = divide(10, 3);
quotient;    // 3
remainder;   // 1
```

#### 单元素元组

需要尾随逗号：

```prim
let single = (1,);      // 单元素元组
let not_tuple = (1);    // 这只是 1，不是元组
```

---

## 容器对比

| 特性 | List | Dict | Tuple |
|------|------|------|-------|
| 可变性 | ✅ 可变 | ✅ 可变 | ❌ 不可变 |
| 有序性 | ✅ 有序 | ❌ 无序 | ✅ 有序 |
| 索引访问 | ✅ 数字索引 | ✅ 键索引 | ✅ 数字索引 |
| 可哈希 | ❌ | ❌ | ❌ |
| 持有方式 | 引用 | 引用（Key+Value） | 引用 |
| 构造语法 | `[...]` | `{...}` | `(...)` |

---

## 快速参考

### 语法速查

| 语法 | 含义 |
|------|------|
| `let a = x` | 值绑定（拷贝） |
| `let a = &x` | 引用绑定 |
| `@{ ... }` | 创建闭包 |
| `$name(...) { ... }` | 定义函数 |
| `$(x)` | 值参数 |
| `$(&x)` | 引用参数 |
| `del a` | 删除符号 |
| `break \`label\`` | 跳出标签循环 |
| `a: Type` | 类型提示 |
| `Type1 \| Type2` | 联合类型 |

### 容器速查

| 操作 | List | Dict | Tuple |
|------|------|------|-------|
| 构造 | `[1, 2, 3]` | `{"a": 1}` | `(1, 2, 3)` |
| 访问 | `list[0]` | `dict["key"]` | `tuple[0]` |
| 修改 | `list[0] = x` | `dict["k"] = x` | ❌ 不可变 |
| 长度 | `list.len()` | `dict.len()` | `tuple.len()` |
| 包含 | `x in list` | `k in dict` | `x in tuple` |
| 追加 | `list.push(x)` | `dict[k] = v` | ❌ 不可变 |
| 拼接 | `a + b` | ❌ | `a + b` |
| 切片 | `list[1:3]` | ❌ | `tuple[1:3]` |