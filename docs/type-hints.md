# Prim 类型提示与 `.pri` 接口文件

`.pri` 类似 Python 的 `.pyi`：它描述值、函数参数、返回值和对象字段，供编辑器使用。`.pr` 继续由 `prim-rs` 解释或 JIT 执行，语法与运行时行为不因接口文件改变。

**`.pri` 不会被执行，不创建运行时变量，不隐式导入代码，也不向 `.pr` 添加类型注解语法。** 类型提示不是完整的静态类型检查；实际执行仍以 `src/prim-rs` 为准。

## 1. 文件组织与加载

```text
Prim/
├── builtin/
│   ├── types.pri       # 基础类型、Number、TerminalSize、GCStats
│   ├── std.pri         # std 及其子模块
│   └── jit.pri         # jit
├── examples/
│   ├── type_hints.pr
│   └── type_hints.pri
└── src/prim-language-server/
    └── builtin/        # 构建生成的打包副本
```

语言服务器按以下规则加载：

1. 标准库默认使用扩展随附的 `builtin/*.pri`，安装后不需要 Rust 源码。
2. 打开的工作区根目录若有 `builtin/std.pri`，使用该根目录 `builtin/` 中的 `.pri` 声明。多根工作区按当前文件所在的最具体根目录选择。
3. 对 `foo.pr`，额外读取同目录、同名的 `foo.pri`。它为实际存在的顶层绑定提供提示；声明一个名字不会让 `.pr` 中未定义的名字变成合法变量。
4. 编辑器中未保存的 `.pri` 内容优先于磁盘。打开、修改、关闭接口文档，以及磁盘上的新增、修改、删除，都会使依赖文档重新分析。

接口中的类型名可以引用标准库类型或同一 `.pri` 中的类型别名，支持前向引用。不扫描整个工作区，不把其他目录中任意同名 `.pri` 的声明混入当前脚本，也不提供 `import` 语法。

标准库声明若缺少 Rust 已注册函数、参数数量不一致或包含语法错误，会显示接口诊断，并暂用扩展随附的标准库声明。普通同名 `.pri` 若有错误，其提示会暂时停用，`.pr` 继续按自身代码分析。

## 2. 基础语法

### 类型声明

```prim-interface
type int;
type float;
type bool;
type str;
type unit;
type closure;
type callable;
type any;
```

这些基础类型已由 `builtin/types.pri` 提供，用户接口无需重复声明。

| 类型 | 含义 |
| --- | --- |
| `int` | 运行时有符号 64 位整数 |
| `float` | 运行时浮点数 |
| `bool` | `true` 或 `false` |
| `str` | 字符串 |
| `unit` | 唯一值 `()` |
| `closure` | 闭包值，可能只有数据，也可能可调用 |
| `callable` | 可调用闭包，未指定参数签名 |
| `any` | 动态值，未约束种类 |

可以声明不透明的自定义类型：`type ExternalValue;`。它有可跳转的类型名字，但没有可推断的字段结构。

### 类型别名与联合类型

```prim-interface
type NumericValue = int | float;
type OptionalText = str | unit;
```

`|` 表示值可能属于其中一种类型。`str | unit` 常用于读取输入、查询环境变量等可能没有结果的函数。联合类型不会带来隐式转换；例如 `int | float` 不表示 `prim-rs` 接受混合种类的算术操作。

括号可以组合类型。递归类型别名目前不支持，会给出 `RecursiveType` 诊断。

### 值声明

```prim-interface
let limit: int;
let message: str | unit;
```

声明必须以分号结束，不能有初始化式。`let limit: int = 100;` 不属于 `.pri` 语法。运行时初始化仍写在 `.pr` 中：`let limit = 100;`。

### 函数签名

```prim-interface
let add: $[](left: int, right: int) -> int;
let update: $[](&target: int, delta: int) -> unit;
let read: $[]() -> str | unit;
```

函数类型沿用 `$[](...)` 的外形：

- `[]` 必须为空。接口描述调用约定，不声明运行时捕获环境。
- 每个参数写作 `name: Type`，引用参数写作 `&name: Type`。
- `->` 后为返回类型，没有函数体。
- 参数按位置匹配实现；参数数量和 `&` 模式不一致时，在 `.pr` 中给出 `InterfaceMismatch` warning，并放弃该不匹配签名的提示。
- 参数名字主要用于显示；实现中的实际参数名仍用于函数体绑定和跳转。

函数类型可以作为参数或返回值：

```prim-interface
let apply: $[](function: $[](value: int) -> int, input: int) -> int;
let make: $[]() -> ($[](value: int) -> int);
type OptionalFunction = ($[](value: int) -> int) | unit;
```

`$[](value: int) -> int | unit` 表示函数返回联合类型；若要表示“函数或 unit”，应给整个函数类型加括号。

### 对象字段

```prim-interface
type Point = @{
    let x: float;
    let y: float;
};

let origin: Point;
let distance_squared: $[](point: Point) -> float;
```

`@{ ... }` 描述闭包的字段结构，不构造对象。字段本身也可以是函数类型或另一个对象类型。`std.pri` 使用这种结构描述标准库及其子模块。

`//` 和 `/* ... */` 注释与 `.pr` 一致。紧贴声明前的注释会出现在悬停说明中。

## 3. 同名 `.pr`／`.pri` 示例

`examples/type_hints.pri`：

```prim-interface
type Point = @{
    let x: float;
    let y: float;
};
let length_squared: $[](point: Point) -> float;
let point: Point;
let result: float;
```

对应的 `examples/type_hints.pr`：

```prim
let length_squared = $[](point) {
    point.x * point.x + point.y * point.y
};
let point = @{ let x = 3.0; let y = 4.0; };
let result = length_squared(point);
std.assert(result == 25.0);
std.print(result);
```

编辑器可在 `point.` 后补全 `x`、`y`，在悬停及签名中显示 `Point`、`float`，并从类型使用处跳到接口声明。删除 `.pri` 后，`.pr` 仍然可以执行，只是编辑器对未知参数的推断减少。

## 4. 标准库声明与 Rust 实现

根目录 `builtin/` 是类型提示的维护源；`src/prim-language-server/builtin/` 是发布副本，不应直接编辑。

- `types.pri`：基础值类型、数字联合 `Number`、终端尺寸 `TerminalSize` 和 GC 统计 `GCStats`。
- `std.pri`：`std` 的顶层函数及 `text`、`math`、`bits`、`seq`、`random`、`time`、`env`、`process`、`fs`、`term`、`sys`、`ast`、`inspect`、`gc` 模块。
- `jit.pri`：`jit` 的调用提示；语言服务器保留其输入用户闭包的具体签名。

目前声明覆盖 `prim-rs/src/builtins.rs` 注册的全部 168 项函数。名称和参数数量来自 Rust 注册表，参数类型、返回类型和说明由 `.pri` 描述。构建时会校验二者一致并复制接口文件：

```bash
node scripts/prim-language-server/sync-builtins.js
node scripts/prim-language-server/sync-builtins.js --check
```

新增运行时函数时，先修改 Rust 实现，再更新相应 `.pri`，然后同步并测试。类型提示不会改变标准库只读规则：预置名字不可写，标准库字段槽的只读检查在运行时发生，编辑器可以提前以 warning 提醒。

## 5. 编辑器功能与边界

支持 `.pri` 的语法高亮、语义类型色、类型名补全、悬停、定义／类型定义跳转、大纲、折叠和语法诊断。`.pr` 可消费同名接口和标准库接口，显示带类型的函数签名、参数名提示、变量类型提示，并跳转到 `.pri` 字段或类型声明。

接口提示修改后会重新分析相关打开文档；支持 LSP 刷新协议的客户端会刷新语义高亮与行内提示。

当前边界：

- 不实现完整类型检查、泛型、重载、类型收窄或跨函数副作用分析。提示与运行时值不一致时，执行结果仍由 `prim-rs` 决定。
- 不对联合类型猜测某个分支的专属字段或签名。
- `.pri` 的值声明、类型名均不会成为 `.pr` 的隐式全局变量。
- 引用搜索主要限于当前文档；不承诺工作区级跨接口引用索引。
- 暂不自动重命名接口声明或有同名接口约束的实现绑定，避免只改一侧造成接口失配。其他普通局部变量仍支持重命名。
- 运行时 `.pr` 不接受类型注解、`type` 声明或 `->`。修改 `.pri` 语法不扩大解释器的语言范围。
