# Prim 解释器 CLI

可执行文件为 `prim`，源码扩展名为 `.pr`。以下对应当前 Rust CLI，使用 `prim --help` 查看程序内帮助。

## 1. 执行入口

```bash
prim program.pr
prim run program.pr
prim program.pr -- argument1 argument2
prim -e 'std.print(42);'
prim -c 'std.print(42);'
prim --result -e '21 * 2'
cat program.pr | prim -
cat program.pr | prim
```

无输入参数时：stdin 是终端就进入 REPL，否则将 stdin 全部作为源码执行。`-` 显式表示从 stdin 读源码。需要在程序中读取用户输入时使用文件或 `-e`，避免源码已经消费掉 stdin。

`-e`、`-c`、`--eval` 等价，可以重复；多个输入单元共享会话状态：

```bash
prim -e 'let x = 1; std.print("once");' -e 'x = x + 1; std.print(x);'
```

结果只输出一次 `once` 和一次 `2`，不会重跑前面的输入。一个命令不能同时提供文件和 `-e`；多输入检查模式也不接受重复 `-e`。

`--` 后的内容原样成为脚本参数，经 `std.env.argc/arg` 访问。不包含可执行文件名或脚本路径。若尚未指定输入，`--` 后第一个参数作为文件路径，因此可以执行名字以 `-` 开头的文件。

## 2. 模式与子命令

| 子命令/flag | 行为 |
| --- | --- |
| `run` | 执行，默认模式 |
| `check` / `--check` | 词法、语法、名字与静态检查、IR 生成；不执行 |
| `lex` / `tokens` / `--tokens` / `--lexer-only` | 只输出 Token |
| `ast` / `parse` / `--ast` | 输出 AST 并完成静态检查；不执行 |
| `ir` / `--ir` | 输出 Core IR 与根活跃性元数据；不执行 |
| `repl` / `--repl` | 交互会话；也接受通过管道传入的命令行 |
| `builtins` / `--builtins` | 列出全部内置名与参数数量 |
| `version` / `-V` / `--version` | 版本与实际 LLVM 版本 |
| `help` / `-h` / `--help` | 帮助 |

```bash
prim check src/prim-rs/examples/snake.pr
prim ast -e 'let x = 1; x + 2'
prim ir src/prim-rs/examples/closures.pr
```

结构查看模式都不会运行 std.print、输入、文件写入或 JIT。它们只接受一个源码输入，不能同时要求执行后进入交互。

子命令名只在第一个参数位置识别；执行名为 `run` 等的文件时使用 `./run` 或完整路径。

## 3. REPL 与执行后交互

```bash
prim
prim repl
prim -i program.pr
prim -i -e 'let counter = 0;'
prim repl --quiet --no-prompt
```

示例会话：

```text
>>> let x = 2;
>>> let square = @jit $[&x]() { x * x };
>>> x = 5;
>>> square()
25
>>> :vars
square = <closure>
x = 5
```

- 输入保持语言分号规则：let/赋值等语句需要 `;`；表达式可以不带分号并自动显示结果。
- 未闭合块/参数列表和块注释使用 `... ` 提示继续输入。
- `()` 结果不自动显示；脚本/eval 可通过 `--result` 显示所有尾值。
- 每条输入单独编译并只执行一次，已有变量槽、闭包和 JIT 入口保留。
- 同一条输入内仍禁止重复 let；后续输入可以 `let x = ...` 遮蔽会话里的旧绑定，旧闭包仍保有原槽。
- 编译失败不执行该输入。运行时失败保留已经发生的写入/输出，以及已经执行的顶层声明；失败点后的声明不会加入会话。
- 错误后会话继续接受输入。文件/eval 使用 `-i` 时，初始输入失败后也可以继续检查已完成的状态。
- Ctrl+D 或 `:quit` 退出；`:cancel` 放弃尚未完成的多行输入。当前没有终端行编辑库、持久命令历史文件或 Tab 补全。

### 会话命令

| 命令 | 行为 |
| --- | --- |
| `:help` | 显示会话命令 |
| `:quit` / `:q` | 退出 |
| `:cancel` | 清空未完成的多行输入 |
| `:vars` | 按名字列出全局绑定及显示值 |
| `:del NAME` | 删除会话绑定，并释放最后结果根后收集；其他闭包仍可持有旧槽 |
| `:load FILE` | 在当前会话执行文件，导入已执行的顶层绑定 |
| `:reset` | 销毁整个旧 Runtime、源码记录及 LLJIT，建立空会话 |
| `:builtins` | 显示内置接口 |
| `:stats` | 输出累计执行/JIT/GC 统计 |
| `:gc` | 主动收集并输出统计 |

`:load` 后剩余文本是路径，不经过 shell 解释，可包含空格。所有命令除 quit/cancel 外仅在没有未完成输入时识别。

会话源码用于定位旧函数定义中的错误，并受 `--max-source-bytes` 限制。旧函数代码、诊断源码和机器码保持到 `:reset`/退出，这是会话保留策略；没有自动卸载仍可能被闭包使用的代码。

## 4. 执行与资源 flags

| flag | 默认与作用 |
| --- | --- |
| `--jit=on` | 默认；仅尊重源码中显式 @jit，不自动编译所有函数 |
| `--jit=off` / `--no-jit` | 让标记函数也解释执行，用于差分验证 |
| `--jit-log` | 显示真实发生的本机编译 |
| `-O0` / `-O1` / `-O2` | 默认 O2。O0 使用通用降低；O1/O2 启用局部标量降低，并运行对应 LLVM 优化管线 |
| `--opt-level N` / `-O N` | 与短选项等价，只接受 0、1、2；不改变手动 @jit 策略 |
| `--emit-llvm DIR` | 保存 `function_ID.ll`（优化前）和 `function_ID.opt.ll`（实际提交给 ORC） |
| `--gc-threshold N` | 正整数，默认 1024 次槽/闭包分配后检查自动 GC |
| `--fuel N` | 非负运行时操作预算，默认不限制；0 不允许执行操作 |
| `--stack-limit N` / `--max-depth N` | 正整数，默认最多 256 个活动 Frame，包含顶层帧 |
| `--max-source-bytes N` | 正整数，默认 16 MiB；文件/单输入限制，会话中同时限制累计源码 |
| `--result` | 打印每个文件/eval 输入单元的尾值 |
| `--stats` | 结束时输出累计统计；REPL 也可使用 :stats |
| `--time` | 输出每个输入单元解析/编译/执行的墙上时间 |

fuel 按每次输入单元重新计算，累计 steps 统计不会清零。资源限制报错后 REPL 仍可继续输入。

设置 `--fuel` 时，已编译函数使用通用路径，逐操作检查预算；不会进入局部标量快速路径。正常完成及同步错误时，各优化等级的 steps 与解释器一致。

`--stats` 还输出 `helper_calls`（本机到运行时的回调次数）、`scalar_calls`（进入局部标量路径次数）、`scalar_fallbacks`（执行中恢复到解释器次数）、`slot_allocations/closure_allocations`（累计分配事件，包括复用 arena）、`gc_ms`（累计 GC 时间）及 `native_code_bytes`（成功读取符号大小的编译入口字节总数）。这些不是 RSS 或分配器实际字节数。

`--emit-llvm` 不生成独立可执行文件；第一次调用或 `std.inspect.compile(f)` 会触发产物生成。无磁盘机器码缓存，无外部编译子进程。

## 5. 输出与诊断 flags

| flag | 作用 |
| --- | --- |
| `-q` / `--quiet` | 隐藏 REPL 欢迎文字及 JIT 禁用提示；不隐藏程序输出或错误 |
| `--no-prompt` | 隐藏 REPL 提示符 |
| `--diagnostic-format text` | 默认，源码片段、位置与源级调用栈 |
| `--diagnostic-format json` | 语言错误以一行 JSON 输出 |
| `--color auto` | 默认，仅 stderr 是终端且未设置 NO_COLOR 时着色 |
| `--color always` / `--color never` | 强制文本错误标题颜色或禁用 |

程序结果、结构查看、内置列表写 stdout；诊断、统计和计时写 stderr。JSON 诊断包含 kind、message、file、line、column、start、end，位置相对所属输入单元。usage/文件读取错误仍使用简短文本；`--stats/--time/--jit-log` 是独立的观测输出，不转为 JSON。

```bash
prim --diagnostic-format=json -e '1 / 0'
prim --color=never --fuel=10000 -e 'loop {};'
prim --stats --time --jit=off src/prim-rs/examples/jit.pr
```

带值的长 flag 支持空格或 `=` 写法，例如 `--fuel 1000` 和 `--fuel=1000`。

## 6. 退出码

| 退出码 | 含义 |
| --- | --- |
| 0 | 成功、帮助、版本或正常交互退出 |
| 1 | 词法/语法/静态语义/运行时/JIT 错误；管道 REPL 中出现过错误也返回 1 |
| 2 | 参数使用错误、输入文件读取错误、输入大小或编码问题 |

终端 REPL 中显示语言错误后允许继续，正常 :quit/Ctrl+D 返回 0。管道 REPL 保留“曾出现错误”的退出状态，便于自动化检查。源码尾值不会被当作进程退出码。

## 7. 常用工作流

```bash
# 编辑后只检查
prim check game.pr

# 查看语法树和核心指令
prim ast game.pr
prim ir game.pr

# 对照解释与本机执行
prim --stats --time game.pr
prim --stats --time --no-jit game.pr

# 先加载定义再交互
prim -i library.pr

# 实际游戏与无界面验证
prim src/prim-rs/examples/snake.pr
prim --stats src/prim-rs/examples/snake.pr -- --self-test

# 内存诊断
prim src/prim-rs/examples/memory.pr
```

源码实现为 `src/main.rs`、`src/session.rs` 及 runtime/ir 的增量入口。测试覆盖 exit code、stdin/eval/argv、无副作用检查、持久状态、加载、错误后继续、JSON、颜色、JIT 选择和预算。
