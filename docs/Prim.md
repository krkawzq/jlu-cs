# Prim

Prim 是一个 JIT 友好的动态解释型语言，具有独特的设计哲学：**一切皆 Prim**。

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

字面量和临时值会被优化为原地构造（因为逻辑上完全等价（没有拷贝函数等其他逻辑））：

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

支持的类型：
- `i32`：32 位有符号整数
- `i64`：64 位有符号整数
- `u32`：32 位无符号整数
- `u64`：64 位无符号整数
- `f32`：32 位浮点数
- `f64`：64 位浮点数
- `str`：字符串
- `bool`：布尔值
- `unit`：单元类型，表示没有值，其值为()
- `tuple`
- `list`
- `dict`

---

## Prim（Primitives）

### 一切皆 Prim

Prim 认为所有的可执行逻辑都是 prim，他们代表了一种状态到另一种状态的转换，他们本身代表了可执行逻辑。

prim 持有一些符号和槽，这些符号构成了prim的闭包空间；这些符号可能指向外部也可能指向内部私有的槽。

prim的特点是，他一定具有参数和返回值。实际上有些参数不是显式的，而是作为外部传入的闭包符号。

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

对于scope外的语句而言，他们的都具备返回值（表达式的值），他们符号对他们都是可见的。

而scope的行为有所不同，他同样具备返回值，返回值是最后一个分号后的语句的值。然而，外部符号对于scope默认都是不可见的，需要显式的声明。

### 捕获

prim可以被看作一些定义后立刻执行的函数，实际上我们没有函数的概念，我们将所有的可执行逻辑抽象为“prim”，函数是一种被延迟执行的prim。

一般的，所有的prim都是匿名的，我们可以捕获一个prim并绑定到一个名称上，这样他将会变成一个延迟执行的函数。

使用 $name(args) 的语法来进行捕获一个prim。

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

需要注意的是，只有scope是可以被捕获的。捕获本身也是一个prim。

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
- `break` 内部scope无法直接break到外部scope

---

## 函数与结构体

prim中没有函数与结构体的概念，prim认为一切都是prim和closure。函数是一种被延迟执行的prim，结构体是一个特殊的closure。

### 函数定义

```prim
$add(x, y) {
    x + y
}

add(1, 2);  // 3
```

### 参数传递

参数的传递行为等价于一条let语句。

因此在调用函数时，传入引用和传入值是有区别的。

实际上在函数参数中，可以显示指定是否使用引用

```prim

$func(x, &y) {
    x; // 这是一个值
    y; // 这是一个引用
}

let a = 1;
let b = 2;

func(a, b); // 调用方传递值
func(&a, &b); // 调用方传递引用

```

### 结构体

结构体是一个特殊的closure。

结构体的定义需要使用装饰器@struct，装饰器是一类特殊的prim，他们接受prim并返回prim。

@struct装饰器将会修饰一个prim，让他返回该prim执行后的闭包空间。

```prim

@struct $Point(x, y) {
    let x = x;
    let y = y;
}

let p = Point(1, 2);
p.x;  // 1
p.y;  // 2

```

@struct 和 @ 闭包符号的不同之处在于，装饰器本身是一个prim，是可调用的。且装饰器的使用需要在捕获时，而@用于修饰一个匿名scope直接获得闭包空间。

```prim

$func() {
    let x = 1;
}

func = struct(func); // func成为一个结构体

f = func()
f.x
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

**注意**：暂时不支持循环引用的处理

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


