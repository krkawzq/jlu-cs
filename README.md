# Prim

Prim 是一个“一切皆闭包”的课设语言：closure 是静态的数据，closuring 是动态构造数据的过程；对象是立即执行的 closuring，函数是延迟执行的 closuring。

源文件扩展名统一为 **`.pr`**（例如 `hello.pr`），可执行文件名为 **`prim`**。

Rust 实现位于 [`src/prim-rs`](src/prim-rs/README.md)，已支持解释执行与手动 `@jit`。JIT 直接动态链接系统 LLVM，通过 ORC LLJIT 在当前进程生成并调用本机代码。

## 快速运行

在具备 Rust/Cargo 与 LLVM 14 的 CPU worker 上执行：

```bash
scripts/prim-rs/build.sh
build/prim-rs/prim --jit-log --stats src/prim-rs/examples/jit.pr
scripts/prim-rs/demo.sh
build/prim-rs/prim repl
build/prim-rs/prim src/prim-rs/examples/snake.pr
```

构建不需要下载 Rust 依赖、不编译 LLVM 源码。输出为 `build/prim-rs/prim`；完整 CLI、示例和实现说明见 [Rust 实现 README](src/prim-rs/README.md)。

```prim
let square = @jit $[](x) { x * x };
std.print(square(7));
```

`@jit` 首次调用才编译，后续复用；没有热点检测。`-O0/-O1/-O2` 选择优化等级（默认 O2），`--emit-llvm DIR` 保存优化前后的真实 IR，`--stats` 报告本机调用与整数运算次数，`--jit=off` 显式选择解释执行用于对照。

## 当前功能

- [x] 手写 Lexer/Pratt Parser、AST、名字解析与 Core IR
- [x] 动态类型、词法作用域、闭包/数据对象、显式捕获与引用参数
- [x] 条件、短路、循环、break/continue/return、递归和装饰器
- [x] 解释器、共享槽、循环结构 GC、标准库、源码错误定位
- [x] LLVM ORC 手动 JIT、解释/本机互调、整数溢出检查快路径
- [x] 168 个内置接口：stdin/stdout/stderr、文本、数学、位运算、序列、随机数、时间、文件、进程、终端、系统信息、AST/运行时检查与内存诊断
- [x] 持久化 REPL、重复 -e、检查/结构查看子命令、JSON 诊断与资源 flags
- [x] 可玩的终端贪吃蛇、真实 PTY 验证和内存审计
- [x] CPU pod 上的 release 测试、可执行文件与答辩示例

当前使用遇错即停前端、CFG 活跃寄存器根与进程内机器码缓存。REPL 增量执行并保留显式全局绑定，:reset 释放整个会话；没有命令历史重放。未做前端多错误恢复、磁盘机器码缓存或全量数值优化，边界见实现 README。

## 目录

```text
src/prim-rs/                 Rust 解释器、运行时、LLVM JIT、测试和新示例
src/prim-language-server/    VS Code 扩展与 Node.js language server
src.bak/                    旧 C++ / re2c / Bison 前端材料
scripts/prim-rs/             构建与答辩演示脚本
scripts/prim-language-server/ 编辑器扩展构建脚本
scripts/fig_logo.py          logo 与文件图标绘制脚本
icons/                      logo 与文件图标（fig_logo.py 输出）
docs/                       语言规范、实现设计和计划书
build/prim-rs/               release 可执行文件、演示日志和导出 IR
```

根目录 `examples/` 提供当前语法的 Hello World、确定性猜数字、素数和 `.pr`／`.pri` 类型提示示例；更多闭包、装饰器、JIT 和交互示例位于 `src/prim-rs/examples/`。

## 设计文档

- [语言设计](docs/language-spec.md)：值、槽、闭包、捕获、调用和装饰器。
- [词法规则](docs/lexical-spec.md)与 [Lexer](docs/lexer-design.md)。
- [Parser](docs/parser-design.md)：EBNF、AST、名字解析与 Core IR。
- [语法速查](docs/syntax-guide.md)。
- [`.pri` 类型提示](docs/type-hints.md)：接口语法、标准库声明、同名脚本关联与编辑器支持。
- [JIT 与运行时](docs/jit-runtime.md)：动态 LLVM、ORC、本机 ABI 与 GC。
- [CLI 手册](docs/cli.md)：执行模式、REPL、所有 flags 与退出码。
- [完整内置接口](docs/prim-builtin.md)：168 个内置方法及交互示例。
- [AST、内省与机器码展示](docs/introspection.md)：源码、签名、捕获槽、JIT 状态和真实反汇编。
- [内存管理与审计](docs/memory-management.md)：根、所有权、修复及 Valgrind 证据。
- [项目计划书](docs/项目计划书.md)：课设目标及阶段验收；实现现状以代码、测试及 Rust README 为准。

## LLVM 与构建

用 `LLVM_CONFIG=/path/to/llvm-config` 指定 LLVM。`build.rs` 查询动态库目录并动态链接 `libLLVM`，当前验证版本为 14.0.0。不需要 CMake、re2c、Bison 或外部 Clang 子进程。

仓库根目录的 `Cargo.toml` 是 workspace 清单，也可以直接在根目录执行 `cargo build --release -p prim` 或 `cargo test -p prim`。

```bash
llvm-config --version --link-shared --libnames
ldd build/prim-rs/prim
CARGO_TARGET_DIR=/tmp/prim-rs-target cargo test --release --offline --locked --manifest-path src/prim-rs/Cargo.toml
```

## VS Code 语言支持

`src/prim-language-server/` 提供语义高亮、定义跳转、引用、重命名、悬停、补全、签名提示、大纲、折叠和静态 error/warning。大写且返回数据闭包的构造器（如 `Point`）使用主题的类型色，变量、参数和字段分别分类。服务端不执行用户程序，不依赖 Rust/LLVM；功能边界见[扩展说明](src/prim-language-server/README.md)。构建脚本在 `scripts/prim-language-server/`。

```bash
scripts/prim-language-server/build.sh     # 生成 build/vscode/prim-0.4.0.vsix
scripts/prim-language-server/install.sh   # 构建并安装到本机 VS Code
```

扩展覆盖 Core 1 关键字、`$[captures](parameters)` 闭包、`@{ ... }` 数据闭包、`@jit`、`&` 捕获/参数、字段访问、数字、字符串和注释，并带有扩展 logo 与 `.pr` 文件图标（light / dark 两套 SVG），由 `scripts/fig_logo.py` 生成到根目录 `icons/`，构建脚本负责重新生成并拷贝进扩展。

## 参与贡献

欢迎提 issue、讨论或 PR。如有兴趣参与开发，可以 fork 本仓库后提交修改建议。

## 联系作者

如有问题或建议，欢迎通过 GitHub issue 或邮件联系作者。

---

本项目由 [krkawzq](https://github.com/krkawzq) 维护。

性能优化、可复现基准与展示步骤见 [optimization.md](docs/optimization.md)。
