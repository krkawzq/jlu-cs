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
11. [异常处理](#异常处理)
12. [模块系统](#模块系统)
13. [快速参考](#快速参考)

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

let x = 1;
let y = 2;
let obj = @{
    let &y;
}
obj.x; // error, x is not defined in obj scope
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
    let a = 1;
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
let cmp = if a > b {
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
let a = loop `outer` {
    loop `inner` {
        if condition {
            break `outer` 1;  // 跳出外层循环
        } else {
            break `outer` 0;
        }
    }
};

() // unit, seen as None in python
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


// closure

$func()
{
    let x;
    let a = x;
    return a;
}

let &x;  // like let x = &x;

```

### 结构体

结构体是使用 `@` 修饰的函数，返回闭包而非普通值：

```prim


// @ is a special decorator


let a = {
    // scope
    let a = 1
} // a = 1

let a = @{
    let a = 1
} // a.a = 1;

$foo() {
    let x = 1;
    let y = 2;
    return 0;
}
let a = foo(); // a = 0


@struct







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

    $()(args) {
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

$process(data: i32 | str | unit) {
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
| `exit Status(msg)` | 抛出异常状态 |
| `@catch { ... }` | 捕获异常 |
| `@ignore { ... }` | 忽略异常 |
| `import module` | 导入模块 |
| `pub $func` | 导出公开函数 |
| `pub let x` | 导出公开变量 |

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

---

## 异常处理

### Status 类型

Prim 使用 **`Status`** 类型表示程序运行状态，这是一种基于值的错误处理机制（类似 Rust 的 Result）。

Status 对象包含两个主要字段：
- **`ok`**：布尔值，表示是否成功
- **`message`**：字符串，错误信息（如果有）

### `exit` 语句

使用 `exit` 提前退出当前作用域并返回一个 Status：

```prim
$divide(a, b) {
    if b == 0 {
        exit Status("除数不能为零");
    }
    a / b
}

let result = divide(10, 0);
// 程序不会崩溃，而是返回 Status
```

### `@catch` 块

使用 `@catch` 捕获可能产生的异常状态：

```prim
let status = @catch {
    exit Status("发生错误");
    print("这行不会执行");
}

if status.ok {
    print("执行成功");
} else {
    print("错误：" + status.message);  // 输出：错误：发生错误
}
```

### `@ignore` 块

使用 `@ignore` 创建一个忽略 exit 的上下文：

```prim
@ignore {
    exit Status("这个错误会被忽略");
    print("这行代码仍然会执行");
}
```

**用途**：在某些场景下，你希望代码即使遇到 exit 也继续执行。

### 实践示例

#### 文件读取

```prim
$read_file(path: str) {
    if !file_exists(path) {
        exit Status("文件不存在: " + path);
    }

    let content = file_read(path);
    if content == none {
        exit Status("读取文件失败");
    }

    content
}

// 使用
let status = @catch {
    let content = read_file("data.txt");
    print(content);
}

if !status.ok {
    print("错误：" + status.message);
}
```

#### 嵌套错误处理

```prim
$process_data(data: str) {
    if data.len() == 0 {
        exit Status("数据为空");
    }
    // 处理逻辑
    data.upper()
}

$main() {
    let status = @catch {
        let file_status = @catch {
            read_file("data.txt")
        };

        if !file_status.ok {
            exit Status("无法读取文件");
        }

        process_data(file_status.value);
    };

    if !status.ok {
        print("程序错误：" + status.message);
    }
}
```

#### 带返回值的错误处理

```prim
$safe_divide(a, b) {
    @catch {
        if b == 0 {
            exit Status("除数为零");
        }
        a / b
    }
}

let result = safe_divide(10, 2);
if result.ok {
    print("结果：" + result.value);  // 结果：5
} else {
    print("错误：" + result.message);
}
```

### 错误传播

错误会自动向上传播：

```prim
$step1() {
    exit Status("步骤1失败");
}

$step2() {
    step1();  // 错误会向上传播
    print("不会执行");
}

$main() {
    let status = @catch {
        step2();
    };

    print(status.message);  // 输出：步骤1失败
}
```

### 最佳实践

1. **明确的错误信息**：提供有意义的错误描述
   ```prim
   exit Status("用户ID " + user_id + " 不存在");
   ```

2. **分层处理**：在适当的层级捕获错误
   ```prim
   // 底层函数：传播错误
   $db_query(sql) {
       if !valid_sql(sql) {
           exit Status("无效的SQL");
       }
       // ...
   }

   // 上层函数：处理错误
   $get_user(id) {
       let status = @catch {
           db_query("SELECT * FROM users WHERE id=" + id)
       };

       if !status.ok {
           return default_user();
       }
       status.value
   }
   ```

3. **避免过度使用 ignore**：只在确实需要忽略错误时使用

---

## 模块系统

### 基本概念

Prim 的模块系统基于**闭包**：
- 每个文件本身就是一个**闭包**
- 默认情况下，文件内的符号是**私有**的
- 使用 `import` 导入其他文件的符号

### 导入模块

```prim
import module;              // 导入 module.prim 中的所有公开符号
import math;                // 导入 math.prim
import utils.string;        // 导入 utils.string.prim（使用 . 作为路径分隔符）
```

### 可见性规则

在 Prim 中，**所有符号默认都是私有的**，需要使用 `pub` 关键字显式导出：

```prim
// 私有函数（默认不可见）
$_internal_calc() {
    // 只能在模块内部使用
}

// 公开函数（使用 pub 导出）
pub $add(a, b) {
    a + b
}

pub let PI = 3.14159;
```

### 文件结构

**math.prim**：
```prim
// 私有函数（默认不可见）
$_internal_calc() {
    // ...
}

// 使用 pub 导出公开函数
pub $add(a, b) {
    a + b
}

pub $multiply(a, b) {
    a * b
}

// 公开常量
pub let PI = 3.14159;

// 私有常量
let _epsilon = 0.0001;
```

**main.prim**：
```prim
import math;

let result = math.add(1, 2);        // ✅ 可以访问（pub）
let pi = math.PI;                   // ✅ 可以访问（pub）
// math._internal_calc();           // ❌ 错误！私有函数不可见
// math._epsilon;                   // ❌ 错误！私有常量不可见
```

### 导入特性

#### 1. 单例导入

模块只会被导入**一次**，后续 import 会复用已加载的实例：

```prim
import math;   // 首次加载 math.prim
import math;   // 直接返回已加载的实例
```

#### 2. 循环依赖保护

Prim 自动处理循环导入，不会陷入死循环：

**a.prim**：
```prim
import b;

$foo() {
    b.bar()
}
```

**b.prim**：
```prim
import a;

$bar() {
    print("bar called");
}
```

系统会检测并正确处理这种循环依赖。

#### 3. 路径规则

```prim
import .local_module;           // 相对当前文件（当前目录）
import ..parent_module;         // 父目录
import std.io;                  // 标准库
import project.lib.database;    // 使用 . 作为路径分隔符
```

### 选择性导入

```prim
// 只导入特定符号
import math { add, multiply };

add(1, 2);           // 可用
// multiply(2, 3);   // 如果没导入则不可用

// 导入并重命名
import math { add as sum };
sum(1, 2);           // 3
```

### 可见性约定

1. **默认私有**：所有符号默认不可见
2. **显式导出**：使用 `pub` 关键字标记公开符号
3. **命名约定**（建议）：私有符号使用 `_` 前缀

```prim
// module.prim

// 公开函数
pub $public_func() {
    _private_helper()  // 调用私有函数
}

// 私有函数
$_private_helper() {
    // 只在模块内部可见
}

// 公开常量
pub let PUBLIC_CONST = 100;

// 私有常量
let _private_const = 200;
```

### 命名空间

模块自动创建命名空间：

```prim
// utils.string.prim
pub $to_upper(s: str) { /* ... */ }

// utils.array.prim
pub $to_upper(arr) { /* ... */ }

// main.prim
import utils.string;
import utils.array;

utils.string.to_upper("hello");   // 不会冲突
utils.array.to_upper([1, 2, 3]);
```

### 重新导出

模块可以重新导出其他模块的符号：

```prim
// utils.index.prim
import .string;
import .array;
import .math;

// 重新导出（将导入的模块公开）
pub let string = string;
pub let array = array;
pub let math = math;

// main.prim
import utils;

utils.string.to_upper("hello");
utils.math.add(1, 2);
```
