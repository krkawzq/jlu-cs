---
title: Prim：一切皆闭包
subtitle: 一个统一数据与计算的语言 PoC
language: zh-CN
audience: 编程语言与编译原理课程师生
presentation_type: 语言设计与实现展示
implementation_status: 开发中
main_slides: 27
appendix_slides: 5
estimated_talk_minutes: 15
aspect_ratio: "16:9"
---

# Prim 课堂展示文案

## 使用说明

整场只追一个问题：**状态放在哪里，操作怎样找到它？** 先用一个订单金额汇总器（add / read）看类型化、接口化、函数式三种常见范式的回答，指出它们都把数据与计算当作两种东西；再提出“一切皆闭包”的 PoC，由 Prim 给出第六种写法，并用 `std.inspect` 把槽、捕获和机器码直接摊开。每个概念页之后紧跟一页现场运行；最后用三段真实程序收束。

每页栏目固定为“版式”“屏幕文案”“设计说明”“依据”：

- **版式**：一行，首个单词是版式名，其后是 `key=value` 选项。`live demo=<id>` 是整页的现场运行页（左侧编辑器、右侧执行结果，代码见 `src/presentation/demos.json`）；`diagram=` 选择结构示意图。
- **屏幕文案**：页面上真正出现的文字。单独一行 `+++` 表示分步出现：之后的内容在下一次按 → 时才显示。以 `输出：` 开头的行显示为运行结果标记。
- **设计说明**：版面意图与时长，不在页面上显示。
- **依据**：对应的实现或参考资料。

`# 第X幕｜名称` 是章节标题，会显示在其后各页的页眉。实战与 `std.inspect` 内容来自真实执行程序；性能表来自带来源记录的预先测量，页面会标明。

---

## 01｜Prim

### 版式

cover

### 屏幕文案

**Prim**

一切皆闭包的语言 PoC

如果数据和计算，本是同一种东西？

### 设计说明

封面，15 秒。只留一个问题，不提前解释任何机制。

### 依据

`src/prim-rs/`。

---

# 第一幕｜常见的三种回答

## 02｜从一个很小的需求开始

### 版式

question

### 屏幕文案

**一个订单金额汇总器，只需要两个操作。**

```text 契约
add(amount)   增加一笔金额
read()        读出累计金额
```

- add(200)
- add(300)
- read() → 500

+++

**状态放在哪里？操作又怎样找到它？**

### 设计说明

40 秒。先只给契约和调用序列，再抛出贯穿全场的问题。接下来先看类型化、接口化、函数式三种常见范式怎样回答，再给出 Prim 的回答。

### 依据

本展示自定的功能契约。

---

## 03｜类型化：把状态装进类

### 版式

pair

### 屏幕文案

**状态是对象的字段，操作是类的方法。**

```cpp C++ · 抽象类
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

操作通过 this 找到状态；类把两者绑在一起。

### 设计说明

40 秒。最熟悉的回答。注意力放在 total 和 this：操作之所以能找到状态，是因为它们属于同一个类。

### 依据

本展示编写的课堂片段。

---

## 04｜接口化：契约可以离开类

### 版式

pair

### 屏幕文案

**契约只描述行为，谁来实现、怎样保存状态，都可以另说。**

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

留意 TypeScript：这里没有类。total 只是一个局部变量，被两个函数记住了。

### 设计说明

40 秒。接口与 trait 都有静态类型系统，这里不比较“有无类型”。真正的伏笔在 TypeScript：状态已经悄悄从对象挪进了函数的环境。

### 依据

[TypeScript 类型兼容性](https://www.typescriptlang.org/docs/handbook/type-compatibility)、[Rust traits](https://doc.rust-lang.org/book/ch10-02-traits.html)。

---

## 05｜函数式：让函数记住状态

### 版式

pair

### 屏幕文案

**函数记住环境，状态就活在环境里。**

```python Python · 闭包
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

```scheme Scheme · 闭包
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

这里没有类，也没有接口：total 活在 make_sum 留下的环境中。但闭包只是函数的附属，对象要靠返回函数、消息分派来模拟。

### 设计说明

40 秒。Python 用 nonlocal 让两个函数共享 total；Scheme 只剩闭包，用消息分派模拟对象。闭包表达对象是 SICP 里的老思路，这里不当作新发现：函数式里闭包始终依附于函数，对象是模拟出来的，不是语言里的同一种构造。

### 依据

[SICP 3.1.1 局部状态](https://sicp.sourceacademy.org/chapters/3.1.1.html)、[Python nonlocal](https://docs.python.org/3/reference/simple_stmts.html#the-nonlocal-statement)。

---

## 06｜换一个前提：一切皆闭包

### 版式

cards

### 屏幕文案

**三种常见范式，一个共同前提。**

- 类型化｜状态是字段，操作是方法，靠 this 相连。
- 接口化｜契约独立于实现，状态交给实现者。
- 函数式｜函数记住环境，对象靠返回函数来模拟。

+++

三者都把“数据”和“计算”当作两种东西。

+++

**我们提出一个 PoC：一切皆闭包。**

closure 是数据，closuring 是构造数据的过程；对象是立即的 closuring，函数是延迟的 closuring。Prim 是这一哲学的实现。

### 设计说明

40 秒。转折页，也是全场的论点。三张卡片依次回顾，指出共同前提：数据与计算是两种构造，靠类、接口或消息分派连起来。再提出 Prim 的 PoC：二者本是同一种 closuring，只是执行时机不同。

### 依据

前三页的对照。

---

# 第二幕｜一切皆闭包

## 07｜closure 与 closuring

### 版式

code diagram=closure-kinds

### 屏幕文案

```prim 两种闭包
let offset = 3;
let add = $[offset](x) { x + offset };

let point = @{
    let x = 1;
    let y = 2;
};

std.print(add(39));
std.print(point.x + point.y);
```

**closure 是数据，closuring 是构造数据的过程。**

- $[捕获](参数) { … }：延迟的 closuring，每次调用才执行。
- @{ … }：立即的 closuring，执行完留下的环境就是对象。

输出：42、3

+++

前三种范式里的两种东西，在这里是同一种构造：区别只在 closuring 何时执行。

### 设计说明

45 秒。先分清 closure（静态的数据，即一组名字到槽的环境）与 closuring（执行语句、由 let 逐个建立槽的过程），再介绍两种构造语法；let、赋值和分号顺带一提。示意图并列两种闭包：同样的环境与槽，一个在构造时已经执行完，一个推迟到调用时执行。

### 依据

[语言规范](language-spec.md)、[语法速查](syntax-guide.md)、`src/prim-rs/src/runtime.rs`。

---

## 08｜现场运行：两种闭包

### 版式

live demo=closures

### 屏幕文案

**std.inspect.describe 报告：add 是 function，point 是 object。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 closures 示例。

---

## 09｜名字指向槽，槽里放值

### 版式

code diagram=slots

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

**共享发生在哪一层？在槽这一层。**

- b = a：新开一个槽，复制值。
- c = &a：c 与 a 指向同一个槽。

输出：2、1、2

+++

后面的捕获与对象，都只是这条规则的延伸。

### 设计说明

45 秒。先建立“名字 → 槽 → 值”的模型，后面不再引入新规则。

### 依据

[语言规范：值、槽与环境](language-spec.md)。

---

## 10｜捕获：把依赖写在明处

### 版式

code diagram=capture

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

**延迟的 closuring，要在构造时写明带走什么。**

- [amount]：构造时复制，得到私有槽。
- [&amount]：直接共享外部的槽。

输出：200、300

+++

不必猜：std.inspect.captures 直接报出每个捕获的模式与槽号。

### 设计说明

60 秒。捕获列表的理由：函数体要到之后某次调用才执行，那时外层作用域可能已经改变，所以依赖必须在构造时写明；@{} 立即执行，外层还在，不需要捕获。先看两个结果，再用 inspect 证明：snapshot 拿到的是新槽，shared 拿到的是 amount 本身的槽。值捕获的私有槽可写，并在多次调用间保留。

### 依据

[语言规范：显式捕获](language-spec.md)、[内省接口](introspection.md)、`src/prim-rs/src/ir.rs`。

---

## 11｜现场运行：看见槽号

### 版式

live demo=capture

### 屏幕文案

**snapshot 拿到一个新槽，shared 拿到的是 amount 本身的槽。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 capture 示例。

---

## 12｜回到汇总器：Prim 的回答

### 版式

code diagram=accumulator

### 屏幕文案

```prim 闭包组合出对象
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

**同一个契约：对象与方法都由 closuring 构造。**

- 每次调用 Accumulator，立即执行一次 @{}，留下 total、add、read。
- add 与 read 是其中创建的延迟 closuring，都捕获 total 的槽。

输出：500

+++

下一页现场运行：members 与 captures 打印出槽号——三处 total，是同一个槽。

### 设计说明

60 秒。全场的高潮：回到第 2 页的问题。构造器就是两种 closuring 的嵌套：延迟的 closuring 里执行一次立即的 closuring。对比第 05 页的 Scheme：这里不需要消息分派来模拟对象，@{} 本身就是带字段的数据，add 与 read 既是字段也是函数。状态在 total 的槽里，操作通过捕获找到它。下一页的现场运行版本额外打印 inspect 结果，展示槽号相同。字段当前对外可见，不宣传为私有成员。

### 依据

[语言规范：数据闭包与字段](language-spec.md)、[内省接口](introspection.md)。

---

## 13｜现场运行：摊开汇总器

### 版式

live demo=accumulator

### 屏幕文案

**members 列出每个字段的槽；add、read 捕获的正是 total 那一个。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 accumulator 示例。

---

# 第三幕｜让它真的跑起来

## 14｜构造函数返回了，状态为什么还在？

### 版式

code diagram=gc

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

**只要还有闭包抓着它，槽就不能释放。**

- 槽与闭包放在受管池里，用索引相连。
- 标记清扫：从活跃帧出发，回收不可达的环。

输出：200、500

+++

字符串用 Rc；槽与闭包这张图交给 GC。

### 设计说明

45 秒。make 已经返回，GC 也跑过了，total 依然可用。根来自活跃帧中的指令位置与活跃寄存器，不是把历史临时值都留住。

### 依据

[内存管理](memory-management.md)、`src/prim-rs/src/runtime.rs`、`src/prim-rs/src/liveness.rs`。

---

## 15｜现场运行：逃逸的槽

### 版式

live demo=lifetime

### 屏幕文案

**make 已经返回，GC 也运行过，total 仍然可用。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 lifetime 示例。

---

## 16｜换一个入口：手动 JIT

### 版式

code diagram=jit-shared

### 屏幕文案

```prim 解释与本机共享状态
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

**jit(add) 换掉的是执行入口，不是环境。**

- 显式标记，首次调用才编译，之后复用入口。
- 两个闭包共享同一个 total 槽。

输出：uncompiled、500、ready

+++

机器码只编译 closuring 过程；槽在调用时传入，不写进代码。

没有热点阈值：哪些函数进入本机代码，由程序自己决定。

### 设计说明

45 秒。inspect.jit 的 state 从 uncompiled 变为 ready，是真实缓存状态。从槽的角度讲：名字在编译期解析为持有槽句柄的寄存器；捕获模式只决定构造时新建槽还是共享句柄，函数体都是同一组 Load/Store。所以 native 与 add 读写同一个 total 槽，同一份机器码可以服务同一字面量的所有实例。

### 依据

[JIT 实现](jit-runtime.md)、`src/prim-rs/src/jit.rs`。

---

## 17｜现场运行：切换解释与 JIT

### 版式

live demo=jit-shared

### 屏幕文案

**分别用“解释”和“JIT”运行：结果相同，只有 JIT 下 state 变为 ready。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 jit-shared 示例。

---

## 18｜往下看：一个函数变成了什么？

### 版式

pipeline

### 屏幕文案

**同一个 square，四层表示，全部由 std.inspect 现场取出。**

- 源码｜std.inspect.source
- AST｜std.ast.dump
- Core IR｜std.inspect.core_ir
- 机器码｜std.inspect.disassemble

+++

乘法走带溢出检查的快路径；其他动态操作回调运行时，帧信息继续服务 GC。

### 设计说明

60 秒。四个面板依次出现，内容由页面加载时运行 pipeline 示例得到，不是预制文本。机器码只截取 imul 与 jno 附近的几行；完整反汇编见下一页现场运行。

### 依据

[内省与机器码展示](introspection.md)、[JIT 与运行时](jit-runtime.md)、`src/prim-rs/src/jit.rs`。

---

## 19｜现场运行：完整的四层输出

### 版式

live demo=pipeline

### 屏幕文案

**终端里是完整的源码、AST、Core IR 与反汇编。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 pipeline 示例。

---

## 20｜builtin：接到外部世界

### 版式

code

### 屏幕文案

```prim 宿主能力也是闭包
let text = std.fs.read_text("orders.csv");
let started = std.time.now_ms();
let values = std.array(8, 0);
std.set(values, 0, 200);
std.term.clear();
std.print(std.inspect.describe(std.print).kind);
```

**语言还需要文件、时间和终端。**

- 数据：文件、文本、数组。
- 运行：时钟、随机数、GC。
- 交互：输出、按键、终端绘制。

+++

它们都是宿主提供的可调用闭包，参数检查与用户函数一致。接下来三段程序就用它们。

### 设计说明

30 秒。过渡页，不逐项列 API。最后一行 describe(std.print) 的结果是 builtin，呼应“一切皆闭包”。

### 依据

[内置接口](prim-builtin.md)、`src/prim-rs/src/builtins.rs`、`builtin/std.pri`。

---

# 第四幕｜真实运行

## 21｜实战一：矩阵乘法，解释与 JIT

### 版式

bench

### 屏幕文案

**80×80 整数矩阵乘法：每次调用 51.2 万次乘加。**

- 两种模式先核对 result 与 checksum，再看耗时。
- 内核只用局部整数，JIT 把它整段放进寄存器与机器栈。
- 首次调用包含编译成本，单独列出。

+++

同一份源码，换一个执行入口，差距是数量级的。

### 设计说明

70 秒。A、B 的元素按公式即时生成，内核满足局部标量路径的条件（无捕获、无调用、无别名），这是 JIT 收益最大的形态；结果与独立 Python 参考逐项核对。右侧卡片为预先测量的中位数，可现场“重新测量”。下一页现场切换解释 / JIT。

### 依据

`src/presentation/scenarios/jit.pr`、`src/presentation/benchmark.py`、`build/presentation/evidence/benchmarks.json`。

---

## 22｜现场运行：矩阵乘法

### 版式

live demo=jit

### 屏幕文案

**先用“解释”运行，再用“JIT”运行：result 相同，warm_total_ms 相差两个数量级。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 jit 示例。

---

## 23｜实战二：用闭包管理一份订单

### 版式

orders

### 屏幕文案

**读取订单 → 筛选 → 按类别汇总 → 写出报告。**

- OrderStore：数据闭包保存四列数据，load 与 summarize 捕获它们。
- 逐字符解析 CSV，按八个类别聚合，写出报告文件。
- 有效行数、总金额、八个类别与独立参考逐项一致。

+++

这一段验证的是表达能力：一个真实的数据处理流程，只用闭包组织。

### 设计说明

70 秒。输入是固定种子的 600 条合成订单。这里以字符串与数组操作为主，大部分走运行时 helper，JIT 收益有限，表中如实列出。下一页现场运行 orders.pr。

### 依据

`src/presentation/scenarios/orders.pr`、`orders.csv` 与实测记录。

---

## 24｜现场运行：orders.pr

### 版式

live demo=orders

### 屏幕文案

**Prim 读取同一份 orders.csv，筛选、汇总并写出报告。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 orders 示例。

---

## 25｜实战三：在真实终端里玩贪吃蛇

### 版式

snake

### 屏幕文案

**游戏逻辑完全由服务器上的 Prim 执行。**

- state：数据闭包保存方向、分数、食物。
- update：捕获 state；step 用显式 JIT。
- 浏览器只负责转发按键、显示终端。

方向键 / WASD 移动 · P 暂停 · R 重启 · Q 退出

### 设计说明

70 秒。下一页现场运行 snake.pr，移动几步、暂停、重启。左侧记录图是一次真实 PTY 运行的截图。

### 依据

`src/prim-rs/examples/snake.pr`、`src/prim-rs/src/terminal.rs`。

---

## 26｜现场运行：贪吃蛇

### 版式

live demo=snake

### 屏幕文案

**运行后点击终端：方向键 / WASD 移动，P 暂停，R 重启，Q 退出。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 snake 示例。

---

# 尾声｜回到问题

## 27｜一切皆闭包，走到了这里

### 版式

cards

### 屏幕文案

**状态放在哪里？操作怎样找到它？**

- 统一组合｜对象是立即的 closuring，函数是延迟的 closuring。
- 明确依赖｜显式捕获与槽共享，关系可解释，也可 inspect。
- 真实执行｜解释器、ORC JIT、数据处理与游戏，共用一套运行时。

+++

**静态保证与性能优化，是下一段路。谢谢。**

### 设计说明

30 秒。用开场的问题收尾，三张卡片各对应一幕。

### 依据

本展示的三段实战与 [语言规范](language-spec.md)。

---

# 附录｜答疑备用

## A1｜性能结果的口径

### 版式

protocol

### 屏幕文案

- 相同输入，结果与独立参考逐项核对；整数不溢出。
- 每种模式多个独立进程，每进程重复调用五次，取中位数。
- 分开记录首次、热调用与进程耗时。
- 保存执行程序、源码与数据摘要，以及 CPU 与 LLVM 版本。

Prim 内部计时精度为 1 ms；低于分辨率的结果不计算加速比。

### 设计说明

答疑用。右侧显示实测记录中的协议与机器信息。

### 依据

`src/presentation/benchmark.py` 与实测 manifest。

---

## A2｜表达同一行为，不等于拥有相同保证

### 版式

table

### 屏幕文案

| 本次验证了 | 不能由此推出 |
| --- | --- |
| 闭包能组合有状态组件 | 自动获得静态接口检查 |
| 不同实现提供相同操作 | 自动获得所有权与借用检查 |
| 解释与 JIT 结果一致 | 所有程序都能加速 |
| 数据处理与游戏能运行 | 工程生态已经成熟 |

Prim 的类型提示服务于编辑器，不改变运行时的动态类型语义。

### 设计说明

答疑用。不以“有类型 / 无类型”划分语言。

### 依据

[类型提示与接口](type-hints.md)、`src/prim-language-server/`。

---

## A3｜网页、语言服务与执行进程

### 版式

text diagram=tooling

### 屏幕文案

- Monaco 复用 Prim 扩展的 grammar，并连接真实 LSP。
- xterm.js 经 WebSocket 与服务器 PTY 双向通信。
- Prim 程序在独立进程执行，输出不是预置文本。
- PPTX 由同一网页逐页截图生成。

### 设计说明

答疑用。两条独立通道：编辑器 ↔ LSP；终端 ↔ PTY ↔ 执行程序。

### 依据

`src/presentation/`、`src/prim-language-server/`。

---

## A4｜JIT 生成的真实机器码

### 版式

native

### 屏幕文案

**std.inspect.compile 只准备入口，不执行函数体。**

- std.inspect.native 给出地址、大小、hex 与反汇编。
- machine_code / disassemble 与 native 逐字节核对。
- origin = loaded-memory：读的是已加载的入口。

预编译后 native_calls 为 0，验证调用后为 1；查询不计入性能测量。

### 设计说明

答疑用。右侧为记录中 imul 附近的真实指令；完整产物见下一页现场运行。

### 依据

[内省与机器码展示](introspection.md)、`src/presentation/native.py`。

---

## A5｜现场运行：读取机器码

### 版式

live demo=jit-native

### 屏幕文案

**--inspect 分支把已加载入口的 hex 与反汇编直接打印出来。**

### 设计说明

整页实时编辑器：左侧代码，右侧真实执行结果。按“运行”或 Ctrl+Enter 执行。

### 依据

`src/presentation/demos.json` 中的 jit-native 示例。

---

