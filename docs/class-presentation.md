---
title: Prim：一切皆闭包
subtitle: 一个统一数据与计算的语言 PoC
language: zh-CN
audience: 编程语言与编译原理课程师生
presentation_type: 语言设计与实现展示
implementation_status: 开发中
main_slides: 30
appendix_slides: 5
estimated_talk_minutes: 15
aspect_ratio: "16:9"
---

# Prim 课堂展示文案

## 使用说明

主线：用一个订单金额汇总器（add / read）比较类型化、接口化、函数式三种常见写法，指出它们都把数据和计算当作两种东西；然后介绍 Prim 的基础语法和核心模型 closure / closuring，用 Prim 重写汇总器；接着讲实现（GC、手动 JIT、`std.inspect`）；最后是三个实战程序。概念页之后一般跟一页现场运行，讲的时候可以按时间取舍。

每页栏目固定为“版式”“屏幕文案”“设计说明”“依据”：

- **版式**：一行，首个单词是版式名，其后是 `key=value` 选项。`live demo=<id>` 是现场运行页（左侧编辑器、右侧执行结果，代码见 `src/presentation/demos.json`）；`diagram=<名称>@<步>` 选择结构示意图及其出现的步骤；`card=<步>` 指定测量卡片出现的步骤。
- **屏幕文案**：页面上显示的文字。单独一行 `+++` 表示分步：之后的内容在下一次按 → 时出现。`~ 3-5` 表示从这一步开始聚焦代码第 3–5 行（多块代码用 `2:6,8` 指定第 2 块），`~ -` 取消聚焦。以 `输出：` 开头的行显示为运行结果标记。
- **设计说明**：版面意图与时长，不在页面上显示。
- **依据**：对应的实现或参考资料。

`# 序号｜名称` 是章节标题，显示在其后各页的页眉。实战与 `std.inspect` 内容来自真实执行程序；性能表来自带来源记录的预先测量，页面会标明。

---

## 01｜Prim

### 版式

cover

### 屏幕文案

**Prim**

一切皆闭包：统一数据与计算的语言 PoC

对象和函数，能不能是同一种东西？

### 设计说明

封面，15 秒。

### 依据

`src/prim-rs/`。

---

# 一｜三种常见写法

## 02｜一个订单金额汇总器

### 版式

question

### 屏幕文案

**需求：一个订单金额汇总器，对外只提供两个操作。**

```text 接口约定
add(amount)   增加一笔金额
read()        读出累计金额
```

- add(200)
- add(300)
- read() → 500

+++

**total 存在哪里？add 和 read 怎么访问到同一个 total？**

下面先用五种语言各写一遍，再看 Prim 怎么写。

### 设计说明

40 秒。调用序列一次出现（依次动画），问题单独一步。这个问题贯穿全场，第 15 页回答。

### 依据

本展示自定的功能约定。

---

## 03｜类型化：C++ 与 Java

### 版式

pair

### 屏幕文案

**先声明一个类型：状态是它的字段，操作是它的方法。**

```cpp C++ · 抽象类与派生类
struct Accumulator {
  virtual void add(long long x) = 0;
  virtual long long read() const = 0;
  virtual ~Accumulator() = default;
};
struct Sum : Accumulator {
  long long total = 0;
  void add(long long x) override {
    total += x;
  }
  long long read() const override {
    return total;
  }
};
```

+++

```java Java · interface 与 class
interface Accumulator {
  void add(long x);
  long read();
}
class Sum implements Accumulator {
  private long total = 0;
  public void add(long x) {
    total += x;
  }
  public long read() {
    return total;
  }
}
```

+++

~ 1:7,9,12 2:6,8,11

- add 和 read 通过隐式的 this 访问 total：它们能共享状态，是因为属于同一个类的同一个实例。
- 调用方只依赖 Accumulator 这个抽象类型，具体实现可以替换。

### 设计说明

40 秒。先 C++，再 Java，最后高亮 total 所在的三行。

### 依据

本展示编写的课堂片段。

---

## 04｜接口化：TypeScript 与 Rust

### 版式

pair

### 屏幕文案

**接口只规定有哪些操作，状态怎么存由实现决定。**

```typescript TypeScript · 结构化接口
interface Accumulator {
  add(x: number): void;
  read(): number;
}
function makeSum(): Accumulator {
  let total = 0;
  return {
    add: x => { total += x; },
    read: () => total
  };
}
```

```rust Rust · trait 与 impl
trait Accumulator {
    fn add(&mut self, x: i64);
    fn read(&self) -> i64;
}
struct Sum { total: i64 }
impl Accumulator for Sum {
    fn add(&mut self, x: i64) {
        self.total += x;
    }
    fn read(&self) -> i64 {
        self.total
    }
}
```

+++

~ 1:1-4 2:1-3

- TypeScript 按结构判断兼容：任何带 add / read 的对象都满足 Accumulator。Rust 用 trait 声明行为，再为具体类型 impl。两者都在编译期检查。

+++

~ 1:6,8,9

- 注意 TypeScript 这一版没有类：total 是 makeSum 的局部变量，makeSum 返回之后，返回的两个函数仍然引用着它。

### 设计说明

40 秒。接口与 trait 都依托静态类型系统，这里不比较“有无类型”。第二步的 TypeScript 是到函数式写法的过渡。

### 依据

[TypeScript 类型兼容性](https://www.typescriptlang.org/docs/handbook/type-compatibility)、[Rust traits](https://doc.rust-lang.org/book/ch10-02-traits.html)。

---

## 05｜函数式：Python 与 Scheme

### 版式

pair

### 屏幕文案

**函数可以引用定义时所在环境里的变量，状态就保存在这个环境里。**

```python Python · 嵌套函数与 nonlocal
def make_sum():
    total = 0
    def add(x):
        nonlocal total
        total += x
    def read():
        return total
    return add, read

add, read = make_sum()
add(200)
add(300)
assert read() == 500
```

```scheme Scheme · 闭包与消息分派
(define (make-sum)
  (let ((total 0))
    (lambda (message)
      (cond
        ((eq? message 'add)
         (lambda (x)
           (set! total (+ total x))))
        ((eq? message 'read)
         (lambda () total))))))
(define s (make-sum))
((s 'add) 200)
((s 'add) 300)
((s 'read)) ; 500
```

+++

~ 1:2,4,5,7 2:2,7,9

- Python 用 nonlocal 让 add 和 read 写同一个 total；Scheme 的 total 由 let 引入，被返回的 lambda 引用。这里已经没有类和接口了。

+++

~ 2:3-5,8

- 但“对象”是模拟出来的：Python 返回两个函数，Scheme 返回一个按消息名分派的函数。闭包仍然只是函数的附属品。

### 设计说明

40 秒。用闭包表达对象是 SICP 里就有的做法，这里不当作新东西；要说明的是在函数式语言里，对象不是语言里的一种构造。

### 依据

[SICP 3.1.1 局部状态](https://sicp.sourceacademy.org/chapters/3.1.1.html)、[Python nonlocal](https://docs.python.org/3/reference/simple_stmts.html#nonlocal)。

---

## 06｜三种写法的共同前提

### 版式

cards

### 屏幕文案

**回顾三种写法：**

- 类型化｜状态是字段，操作是方法，靠 this 关联。
- 接口化｜接口约定操作，状态由实现者保存。
- 函数式｜函数引用外层环境；对象用多个函数或消息分派来模拟。

+++

三种写法都把数据和计算当作两种东西：一边是字段、结构体、环境，一边是方法、函数，再用类、接口或返回值把它们连起来。

+++

**Prim 的出发点：对象和函数都是 closure，区别只在构造它的过程什么时候执行。**

接下来先用两页介绍基础语法，再展开这个模型。

### 设计说明

40 秒。三张卡片是前三页的小结；第二步指出共同前提；第三步提出 Prim 的论点。

### 依据

前三页的对照。

---

# 二｜Prim 的语法与核心模型

## 07｜基础语法（一）：值、绑定与运算

### 版式

code

### 屏幕文案

```prim 值与绑定
let n = 42;
let pi = 3.14;
let ok = true;
let name = "Prim";
let nothing = ();

n = n + 1;
let msg = name + " " + std.str(n);
std.print(msg);
std.print(std.type(pi));
std.print(7 / 2);
std.print(-7 % 3);
```

**五种基本类型：int、float、bool、str、unit；其余一切都是 closure。**

+++

~ 1-5

- 动态类型，没有类型注解；std.type(v) 返回类型名。
- int 是 64 位有符号整数，溢出直接报错；bool 不是整数；() 是唯一的空值，没有 null。

+++

~ 7

- let 新建一个槽并初始化，= 写入已有的槽。同一作用域不能重复 let，内层作用域可以遮蔽外层。

+++

~ 8-12

- 没有隐式转换：1 + 1.0、"a" + 1 都是运行时错误；拼接前用 std.str 转成字符串。
- 整数 / 和 % 向零截断：7 / 2 为 3，-7 % 3 为 -1。

输出：Prim 43、float、3、-1

每条语句以分号结束，// 开始行注释。

### 设计说明

40 秒。快速过一遍，重点是动态类型、无隐式转换、let 与赋值的区别（后面讲槽时会用到）。

### 依据

[语言规范 §8 基础类型与运算](language-spec.md)、[语法速查](syntax-guide.md)。

---

## 08｜基础语法（二）：块、条件与循环

### 版式

code

### 屏幕文案

```prim 控制流都是表达式
let y = {
    let x = 10;
    x * 2
};
let sign = if y > 0 { 1 } else if y < 0 { -1 } else { 0 };

let i = 0;
let total = 0;
let result = loop {
    if i == 5 { break total; };
    i = i + 1;
    if i == 3 { continue; };
    total = total + i;
};
std.print(y);
std.print(sign);
std.print(result);
```

**块、if 和 loop 都是表达式，都有值。**

+++

~ 1-4

- { … } 立即执行；最后一个不带分号的表达式是块的值，没有就是 ()。

+++

~ 5

- if 的条件必须是 bool，不做真值转换；没有 else 且条件为假时值为 ()。

+++

~ 9-14

- loop 是唯一的循环：break 值 结束循环并作为 loop 的值，continue 进入下一轮。没有 for 和 while。

+++

~ 15-17

- return 只在函数里用，提前返回。块、if、loop 都能直接读写外层变量；只有定义函数时才需要写捕获列表，原因见第 13 页。

输出：20、1、12

### 设计说明

40 秒。重点是“控制流都是表达式”和 loop / break 带值；最后一条为捕获列表埋伏笔。

### 依据

[语言规范 §4 作用域与块、§7 控制流](language-spec.md)。

---

## 09｜现场运行：基础语法

### 版式

live demo=basics

### 屏幕文案

**前两页的代码合在一起。可以改几处再运行，例如把 1 + 1.0 写进去看报错。**

### 设计说明

可选，20 秒。

### 依据

`src/presentation/demos.json` 中的 basics 示例。

---

## 10｜closure 与 closuring

### 版式

code diagram=closure-kinds@3

### 屏幕文案

```prim 两种 closure
let offset = 3;
let add = $[offset](x) { x + offset };

let point = @{
    let x = 1;
    let y = 2;
};

std.print(add(39));
std.print(point.x + point.y);
```

**closuring：在一个新作用域里执行语句，用 let 逐个建立槽。closure：这个过程留下的环境。**

+++

~ 4-7

- @{ … } 立即执行 closuring，留下的环境就是对象，let 出的名字成为字段（point.x）。

+++

~ 2

- $[捕获](参数) { … } 把 closuring 推迟到每次调用时执行，这就是函数。

+++

~ -

- 函数值本身也是 closure：构造时已经固定的捕获环境就是它的字段（add.offset）。

输出：42、3

对象是已经执行完的 closuring，函数是推迟执行、可以反复执行的 closuring。

### 设计说明

50 秒。全场的核心概念页。先讲 @{}（立即），再讲 $[]（延迟），最后合起来。示意图在第三步出现：两个 closure 结构相同，只差代码入口。

### 依据

[语言规范 §1.1 closure 与 closuring](language-spec.md)、`src/prim-rs/src/runtime.rs`。

---

## 11｜现场运行：两种 closure

### 版式

live demo=closures

### 屏幕文案

**std.inspect.describe 报告 add 的种类是 function，point 是 object。**

### 设计说明

可选，20 秒。

### 依据

`src/presentation/demos.json` 中的 closures 示例。

---

## 12｜值、槽与别名

### 版式

code diagram=slots@3

### 屏幕文案

```prim 复制与别名
let a = 1;
let b = a;
let c = &a;
a = 2;
std.print(a);
std.print(b);
std.print(c);
```

**名字指向槽，槽里存值。复制值和共享槽是两回事。**

+++

~ 2

- let b = a：新建一个槽，把 a 当前的值复制进去。

+++

~ 3

- let c = &a：c 不是新槽，它和 a 指向同一个槽。

+++

~ 4-7

- & 只能出现在 let 初始化、捕获列表和调用实参里，不能当作值返回或存起来，所以哪些名字共享槽，在源码里都看得到。
- closure 按身份复制：let o2 = o1 之后两者是同一个对象，不做深拷贝。

输出：2、1、2

### 设计说明

40 秒。这页的模型后面一直在用：捕获、对象字段、JIT 共享的都是槽。

### 依据

[语言规范 §3 值、槽与环境](language-spec.md)。

---

## 13｜捕获列表

### 版式

code diagram=capture@3

### 屏幕文案

```prim 值捕获与槽捕获
let amount = 200;
let snapshot = $[amount]() { amount };
let shared = $[&amount]() { amount };
amount = 300;
std.print(snapshot());
std.print(shared());

let s = std.get(std.inspect.captures(snapshot), 0);
let r = std.get(std.inspect.captures(shared), 0);
std.print(s.mode + " → 槽 #" + std.str(s.slot_id));
std.print(r.mode + " → 槽 #" + std.str(r.slot_id));
```

**函数体要到调用时才执行，那时外层作用域可能已经变了，甚至已经不存在，所以定义函数时要声明带走哪些变量。**

+++

~ 2

- [amount]：值捕获。构造时新建一个私有槽，复制当时的值；之后外层再改，互不影响。

+++

~ 3

- [&amount]：槽捕获。和外层共用同一个槽，双方的修改彼此可见。

+++

~ 4-6

输出：200、300

+++

~ 8-11

- std.inspect.captures 返回每个捕获的名字、模式和槽号：snapshot 拿到的是一个新槽，shared 拿到的就是 amount 的槽。
- @{ } 是立即执行的，外层作用域还在，所以不需要捕获列表。

### 设计说明

50 秒。先讲为什么需要捕获（延迟执行），再分别讲两种模式，最后用 inspect 看槽号。值捕获的私有槽可写，并在多次调用之间保留。

### 依据

[语言规范 §1.1、§5.2 显式捕获](language-spec.md)、[内省接口](introspection.md)、`src/prim-rs/src/ir.rs`。

---

## 14｜现场运行：捕获与槽号

### 版式

live demo=capture

### 屏幕文案

**snapshot 拿到一个新槽，shared 拿到的是 amount 自己的槽。可以把 amount = 300 改成别的值再运行。**

### 设计说明

可选，20 秒。

### 依据

`src/presentation/demos.json` 中的 capture 示例。

---

## 15｜汇总器的 Prim 写法

### 版式

code diagram=accumulator@4

### 屏幕文案

```prim 构造器、对象与方法
let Accumulator = $[]() {
    @{
        let total = 0;
        let add = $[&total](amount) {
            total = total + amount;
        };
        let read = $[&total]() { total };
    }
};
let orders = Accumulator();
orders.add(200);
orders.add(300);
std.print(orders.read());
```

**回到第 2 页的汇总器。没有类，也没有接口。**

+++

~ 1,9

- 外层的 $[]() { … } 是一个函数，每调用一次就构造一个新的汇总器。

+++

~ 2-8

- 函数体是一个 @{ … }：每次调用都立即执行一次 closuring，留下 total、add、read 三个字段。

+++

~ 4,7

- add 和 read 是在这次 closuring 中创建的函数，都用 [&total] 捕获了同一个 total 槽。

+++

~ 10-13

输出：500

- total 存在 @{ } 留下的槽里，add 和 read 通过槽捕获访问它，这就是第 2 页问题的答案。
- 和 Scheme 版相比，不需要消息分派：@{ } 本身就是带字段的对象，add 和 read 既是字段也是函数。

### 设计说明

60 秒。构造器是两种 closuring 的嵌套：延迟的 closuring 里执行一次立即的 closuring。字段目前对外可见，不是私有成员，有人问可以说明。

### 依据

[语言规范 §1.1、§6 数据闭包与字段](language-spec.md)。

---

## 16｜现场运行：汇总器的内部结构

### 版式

live demo=accumulator

### 屏幕文案

**members 列出 orders 每个字段的槽号；captures 显示 add 和 read 捕获的都是 total 的那个槽。**

### 设计说明

30 秒。建议现场运行：三处槽号相同，是对上一页结论的直接验证。

### 依据

`src/presentation/demos.json` 中的 accumulator 示例。

---

# 三｜实现：内存、JIT 与内省

## 17｜闭包逃逸与 GC

### 版式

code diagram=gc@3

### 屏幕文案

```prim 逃逸的捕获槽
let make = $[]() {
    let total = 0;
    $[&total](amount) {
        total = total + amount;
        total
    }
};
let add = make();
std.gc.collect();
std.print(add(200));
std.print(add(300));
```

**make 返回之后，它的局部槽 total 还被返回的函数引用着，不能释放。**

+++

~ 1-7

- 槽和闭包分配在运行时的受管池里，用整数句柄引用，不放在调用栈上；GC 不移动对象。

+++

~ 8-11

- 手动调用 std.gc.collect() 之后，add 仍然能正常读写 total。

输出：200、500

+++

~ -

- 回收算法是标记清扫：从活跃帧和全局环境出发标记，不可达的循环引用也能回收。
- 根的判定结合了指令位置和寄存器活跃性，已经不再使用的临时值不会被当作根。
- 字符串单独用 Rc 管理。

### 设计说明

45 秒。

### 依据

[内存管理](memory-management.md)、`src/prim-rs/src/runtime.rs`、`src/prim-rs/src/liveness.rs`。

---

## 18｜现场运行：逃逸的槽

### 版式

live demo=lifetime

### 屏幕文案

**make 已经返回，GC 也运行过了，add 仍然能读写 total；右侧 [stats] 里可以看到 gc_collections。**

### 设计说明

可选，20 秒。

### 依据

`src/presentation/demos.json` 中的 lifetime 示例。

---

## 19｜手动 JIT

### 版式

code diagram=jit-shared@3

### 屏幕文案

```prim 解释执行与本机代码共享槽
let total = 0;
let add = $[&total](amount) {
    total = total + amount;
};
let native = jit(add);
std.print(std.inspect.jit(native).state);
native(200);
add(300);
std.print(total);
std.print(std.inspect.jit(native).state);
```

**jit(f) 返回一个新闭包：执行方式换成本机代码，捕获的还是原来那些槽。**

+++

~ 5

- 是否 JIT 由程序显式决定（jit(f) 或 @jit），没有热点计数。

+++

~ 6-10

- 第一次调用时编译，之后复用同一个入口；std.inspect.jit 能查到缓存状态和调用次数。

输出：uncompiled、500、ready

+++

~ -

- native 和 add 读写同一个 total 槽：机器码只对应函数体，槽在调用时作为环境传入，不写进代码，所以同一个函数字面量的所有实例共用一份机器码。
- 后端是 LLVM ORC LLJIT。整数 + - * 走带溢出检查的快路径，其他操作调用运行时 helper；纯局部整数运算的函数还会走局部标量路径，变量全部放进寄存器。

### 设计说明

50 秒。第三步是实现要点，可以按时间取舍。

### 依据

[JIT 实现](jit-runtime.md)、`src/prim-rs/src/jit.rs`、`src/prim-rs/src/numeric.rs`。

---

## 20｜现场运行：解释与 JIT

### 版式

live demo=jit-shared

### 屏幕文案

**分别选“解释”和“JIT”运行：输出一样，但只有 JIT 模式下 state 会变成 ready。**

### 设计说明

30 秒。

### 依据

`src/presentation/demos.json` 中的 jit-shared 示例。

---

## 21｜从源码到机器码

### 版式

pipeline

### 屏幕文案

**std.inspect 能拿到一个函数在每一层的表示。下面四块是页面加载时实际运行得到的。**

- 源码｜std.inspect.source
- AST｜std.ast.dump
- Core IR｜std.inspect.core_ir
- 机器码｜std.inspect.disassemble

+++

imul 后面紧跟 jno：整数乘法溢出时跳回运行时报错，其余情况直接在寄存器里算完。

### 设计说明

50 秒。四块依次出现。机器码只截取 imul 附近几行，完整内容见下一页。

### 依据

[内省与机器码展示](introspection.md)、[JIT 与运行时](jit-runtime.md)。

---

## 22｜现场运行：完整的四层输出

### 版式

live demo=pipeline

### 屏幕文案

**完整输出：源码、AST、Core IR 和全部反汇编。std.inspect.compile 只准备入口，不执行函数体。**

### 设计说明

可选，20 秒。

### 依据

`src/presentation/demos.json` 中的 pipeline 示例。

---

## 23｜builtin 与标准环境

### 版式

code

### 屏幕文案

```prim 宿主能力也是 closure
let text = std.fs.read_text("orders.csv");
let started = std.time.now_ms();
let values = std.array(8, 0);
std.set(values, 0, 200);
std.term.clear();
std.print(std.inspect.describe(std.print).kind);
```

**std 本身也是一个 closure：文件、时间、终端、内省这些宿主能力，都以函数的形式放在里面。**

+++

~ 1-5

- 数据：std.fs、std.text、std.array / get / set（数组也是数据闭包）。
- 运行：std.time、std.random、std.gc。
- 交互：std.print、std.term（按键、光标、颜色）。
- 内省：std.ast、std.inspect。

+++

~ 6

- builtin 和用户函数走同一套参数检查；describe(std.print) 的结果是 builtin，它也是一个可调用的 closure。

下面三个实战程序只用到这些 builtin。

### 设计说明

30 秒。

### 依据

[内置接口](prim-builtin.md)、`src/prim-rs/src/builtins.rs`、`builtin/std.pri`。

---

# 四｜实战

## 24｜实战一：矩阵乘法

### 版式

bench card=1

### 屏幕文案

**80×80 整数矩阵乘法 C = A × B，再对 C 加权求和；每次调用 51.2 万次乘加。**

- A、B 的元素按公式直接算出，内核里只有局部整数运算、循环和比较。
- 内核满足局部标量路径的条件（无捕获、无调用、无别名），变量都在寄存器里。
- 两种模式的结果都和 Python 写的参考实现逐项比对。

+++

+++

首次调用包含编译时间，表中单独列出；热调用之后只剩机器码本身的开销。

### 设计说明

60 秒。第一步出现测量卡片（柱形动画），第二步补充首次调用的说明。卡片可以现场“重新测量”。

### 依据

`src/presentation/scenarios/jit.pr`、`src/presentation/benchmark.py`、`src/presentation/native.py`、`build/presentation/evidence/benchmarks.json`。

---

## 25｜现场运行：矩阵乘法

### 版式

live demo=jit

### 屏幕文案

**先用“解释”运行，再用“JIT”运行，对比 result 和 warm_total_ms。解释模式大约要 2–3 秒。**

### 设计说明

40 秒。建议现场运行。

### 依据

`src/presentation/demos.json` 中的 jit 示例。

---

## 26｜实战二：订单处理

### 版式

orders card=1

### 屏幕文案

**读 CSV、过滤、按类别汇总、写出报告：一个完整的小型数据处理流程。**

- OrderStore 是一个数据闭包，四列数据各用一个 std.array 保存。
- load 逐字符解析 orders.csv，summarize 按八个类别累加金额，两者都捕获这四列。
- 输入是固定种子生成的 600 条合成订单，结果和独立计算逐项比对。

+++

+++

这个程序主要是字符串和数组操作，大部分要调用运行时 helper，所以 JIT 带来的提升很小。

### 设计说明

50 秒。这一段展示的是表达能力。

### 依据

`src/presentation/scenarios/orders.pr`、`orders.csv` 与实测记录。

---

## 27｜现场运行：orders.pr

### 版式

live demo=orders

### 屏幕文案

**运行后右上角可以下载生成的报告。**

### 设计说明

可选，20 秒。

### 依据

`src/presentation/demos.json` 中的 orders 示例。

---

## 28｜实战三：终端贪吃蛇

### 版式

snake

### 屏幕文案

**游戏逻辑完全由服务器上的 Prim 进程执行。**

- state 是一个数据闭包，保存蛇头、长度、方向、分数和食物位置。
- reset、spawn_food、step 捕获 state 和两个数组；step 用 @jit 编译。
- 用 std.term 读按键、移动光标、绘制画面；浏览器只负责转发按键和显示终端输出。

方向键 / WASD 移动，P 暂停，R 重启，Q 退出。

### 设计说明

40 秒。右侧是一次真实运行的截图。

### 依据

`src/prim-rs/examples/snake.pr`、`src/prim-rs/src/terminal.rs`。

---

## 29｜现场运行：贪吃蛇

### 版式

live demo=snake

### 屏幕文案

**点“运行”，再点一下终端：方向键 / WASD 移动，P 暂停，R 重启，Q 退出。**

### 设计说明

40 秒。

### 依据

`src/presentation/demos.json` 中的 snake 示例。

---

# 总结｜Prim

## 30｜总结

### 版式

cards

### 屏幕文案

**total 存在哪里？add 和 read 怎么访问到它？**

- 一种构造｜对象是执行完的 closuring，函数是推迟执行的 closuring；汇总器、数组、std 都是 closure。

+++

- 显式的槽｜let、& 和捕获列表决定槽是复制还是共享，std.inspect 可以直接查看。

+++

- 可运行的实现｜Rust 前端、解释器、GC、LLVM ORC JIT；矩阵乘法上 JIT 比解释快两个数量级。

+++

还没做的：静态类型检查、更多 JIT 优化、模块系统。

**谢谢，欢迎提问。**

### 设计说明

40 秒。三张卡片逐张出现，分别对应第二、第三、第四部分。

### 依据

本展示的各部分。

---

# 附录｜答疑备用

## A1｜性能测量的方法

### 版式

protocol

### 屏幕文案

- 相同输入，结果与独立参考逐项核对；整数不溢出。
- 每种模式多个独立进程，每个进程重复调用五次，取中位数。
- 首次调用、热调用和进程总耗时分开记录。
- 保存执行程序、源码与数据的摘要，以及 CPU 与 LLVM 版本。

Prim 内部计时精度为 1 ms；低于分辨率的结果不计算倍数。

### 设计说明

答疑用。右侧显示实测记录中的协议与机器信息。

### 依据

`src/presentation/benchmark.py` 与实测记录。

---

## A2｜能表达同样的行为，不代表有同样的保证

### 版式

table

### 屏幕文案

| 本次验证了 | 不能由此推出 |
| --- | --- |
| 闭包能组合有状态组件 | 自动获得静态接口检查 |
| 不同实现提供相同操作 | 自动获得所有权与借用检查 |
| 解释与 JIT 结果一致 | 所有程序都能加速 |
| 数据处理与游戏能运行 | 工程生态已经成熟 |

Prim 的类型提示只服务于编辑器，不改变运行时的动态类型语义。

### 设计说明

答疑用。

### 依据

[类型提示与接口](type-hints.md)、`src/prim-language-server/`。

---

## A3｜网页、语言服务与执行进程

### 版式

text diagram=tooling

### 屏幕文案

- 编辑器是 Monaco，复用 Prim 扩展的 grammar，并连接真实的 Prim LSP。
- 终端是 xterm.js，经 WebSocket 与服务器上的 PTY 双向通信。
- Prim 程序在独立进程中执行，输出不是预置文本。
- PPTX 由同一网页逐页截图生成。

### 设计说明

答疑用。

### 依据

`src/presentation/`、`src/prim-language-server/`。

---

## A4｜JIT 生成的机器码

### 版式

native

### 屏幕文案

**std.inspect.compile 只准备入口，不执行函数体。**

- std.inspect.native 给出入口地址、字节数、hex 和反汇编。
- machine_code / disassemble 与 native 的结果逐字节核对。
- origin = loaded-memory：读取的是 ORC 链接后已加载到内存的指令。

预编译后 native_calls 为 0，验证调用一次后为 1；这些查询不计入性能测量。

### 设计说明

答疑用。右侧是矩阵内核中 imul 附近的指令。

### 依据

[内省与机器码展示](introspection.md)、`src/presentation/native.py`。

---

## A5｜现场运行：读取机器码

### 版式

live demo=jit-native

### 屏幕文案

**--inspect 分支打印矩阵内核已加载入口的元数据、hex 与反汇编。**

### 设计说明

答疑用。

### 依据

`src/presentation/demos.json` 中的 jit-native 示例。
