# Prim 内置接口完整参考

本文对应 `src/prim-rs/src/builtins.rs` 的实际注册表，共 **168 个可调用内置接口**。查看当前二进制的注册结果：

```bash
build/prim-rs/prim --builtins
```

输出形式为 `接口名/参数数量`。源文件扩展名为 `.pr`，可执行文件名为 `prim`。

## 1. 统一约定

- `std` 及其 `time/random/text/math/bits/seq/env/process/fs/term/sys/ast/inspect/gc` 字段都是只读数据闭包，方法是宿主可调用闭包。`jit` 是独立的只读预置名字。
- 所有内置参数按值传递，数量严格匹配，不接受 `&` 实参。复制对象值仍共享闭包身份与字段槽。
- `()` 表示没有结果、EOF 或超时，具体含义由接口定义；它与 `""`、`false`、`0` 不同。
- 类型不匹配报 `TypeError`，参数数量错误报 `ArityError`，传引用实参报 `ArgumentModeError`。数量和模式检查发生在实参求值之前。
- 内置异常终止当前脚本；解释器和 JIT 使用相同实现。无静默转换、无异常捕获语法。
- 字符串为有效 UTF-8，长度与索引采用 Unicode 标量值，不是字节数或显示宽度。NUL 可以存在于字符串值中。
- 表中的 `any`、`number`、`closure` 是文档记法，不是语言类型注解。`number` 表示 int 或 float。

## 2. 输出与输入（9 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.print(value)` | 按 `std.str` 规则输出到 stdout，追加换行，返回 `()` |
| `std.write(value)` | 输出到 stdout 但不追加换行，返回 `()` |
| `std.flush()` | 刷新 stdout，返回 `()` |
| `std.read_line()` | 阻塞读取一行，去掉末尾 LF/CRLF；EOF 且没有内容时返回 `()` |
| `std.input(prompt)` | prompt 必须是 str；输出并刷新提示，再按 read_line 读取 |
| `std.eprint(value)` | 输出到 stderr，追加换行，返回 `()` |
| `std.ewrite(value)` | 输出到 stderr 但不追加换行，返回 `()` |
| `std.eflush()` | 刷新 stderr，返回 `()` |
| `std.read_all()` | 读取 stdin 到 EOF，返回 str；空输入返回 `""` |

空行返回 `""`；EOF 前最后一行没有换行也返回其内容。输入不是有效 UTF-8 或 I/O 失败报 `IoError`；单行限制 16 MiB。`read_all` 同样限制 16 MiB，空输入返回空字符串而不是 `()`。行输入和 `read_all` 必须在 `std.term.enter()` 之外使用，先 `std.term.leave()` 才能恢复读取。

stderr 接口直接写宿主进程的 stderr，不累计到 `Runtime.output`，也不受 `capture_output` 影响；嵌入场景需要自行重定向进程 stderr。

```prim
let name = std.input("Your name: ");
if name != () {
    std.write("Hello, " + std.text.trim(name));
    std.print("!");
};
```

CLI 实时输出但不在内存中累计历史画面。嵌入 Rust 的默认 Config 则收集 `Runtime.output` 而不写终端；调用方可以设 `capture_output=false`，或通过 `take_output()` 取走并清空缓冲区。

## 3. 类型、转换与断言（9 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.assert(condition)` | condition 必须是 bool；真返回 `()`，假报 `AssertionFailed` |
| `std.assert_eq(actual, expected)` | 值相等返回 `()`；不等报 `AssertionFailed`，消息包含两者显示值 |
| `std.fail(message)` | 总是报 `UserError`，message 必须是 str |
| `std.type(value)` | 返回 `int/float/bool/str/unit/closure` 之一 |
| `std.str(value)` | 转为显示字符串；闭包统一显示 `<closure>`，unit 显示 `()` |
| `std.int(number)` | int 原样；float 向零截断，超 i64 范围报 `IntegerOverflow` |
| `std.float(number)` | float 原样；int 舍入到 binary64 |
| `std.parse_int(text)` | 去掉首尾 Unicode 空白，解析带可选正负号的十进制 i64；失败/超范围返回 `()` |
| `std.parse_float(text)` | 去掉首尾空白，解析有限浮点值；失败、NaN、无穷或超范围返回 `()` |

`int/float` 不接受字符串；使用 `parse_int/parse_float` 解析交互输入。文本解析不接受源码中的进制前缀或数字分组下划线。浮点显示使用 Rust `f64::to_string` 的往返格式，缺少小数点/指数时补 `.0`；负零显示 `-0.0`。当前不保证字符数最短的科学记数法选择。

`assert_eq` 按运行时值比较：基础值比内容，闭包比身份。消息顺序固定为 `expected <expected>, got <actual>`，便于测试输出直接阅读。

## 4. 固定长度数组与动态字段访问（5 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.len(value)` | str 返回 Unicode 标量数；closure 返回字段数 |
| `std.array(length, initial)` | 返回有 `length` 个可写数字字段的数据闭包，每个槽初始为 initial 的值副本 |
| `std.get(object, key)` | 读取现存字段值 |
| `std.set(object, key, value)` | 改写现存可写字段，返回 `()` |
| `std.has(object, key)` | 字段存在返回 true，否则 false |

`length` 是 `0..1000000` 的 int。负长度报 `IndexError`，超过上限报 `ResourceLimit`。数组仍是 `closure`，没有新增值类型或 `[]` 字面量/索引语法。

key 是字符串字段名，或转换为十进制字段名的非负 int。数组构造的字段名为 `"0" ... "length-1"`，支持整数下标直接定位；普通闭包按名字查找。负下标报 `IndexError`；不存在的整数键报 `IndexError`，不存在的字符串键报 `MissingField`。`has` 对缺失键返回 false，但非法键类型/负数仍报错。

字段集合固定，`set` 不添加字段、不改变数组长度。只读标准环境字段报 `ReadOnlySlot`。

```prim
let board = std.array(10, false);
std.set(board, 3, true);
std.assert(std.get(board, 3));
std.assert(std.len(board) == 10);
let point = @{ let x = 1; };
std.set(point, "x", 7);
std.assert(point.x == 7);
```

initial 为闭包时，各元素共享该闭包身份。需要独立对象时，在循环内分别构造并 `set`。数组也可以保存自己或其他数组，循环图由 GC 回收。

## 5. 手动编译（1 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `jit(callable)` | 返回共享原捕获槽、使用同一代码但标记为 JIT 的新闭包；已标记的输入返回自身 |

只接受 Prim 用户可调用闭包，否则 `JitTargetError`。`@jit $[](...) { ... }` 等价于 `jit(...)`，首次调用使用动态链接的 LLVM ORC 编译；失败报 `JitCompileError`，不回退执行。详见 [JIT 与运行时](jit-runtime.md)。

## 6. 时间（5 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.time.now_ms()` | 返回 Runtime 创建以来的单调时间，int 毫秒 |
| `std.time.unix_ms()` | 当前 Unix 时间戳，int 毫秒 |
| `std.time.sleep(milliseconds)` | 暂停当前单线程运行时，返回 `()` |
| `std.time.format_utc(unix_ms)` | 格式化为 `YYYY-MM-DD HH:MM:SS`，范围为 0000–9999 年 |
| `std.time.parts(unix_ms)` | 返回 UTC 日历字段闭包 `TimeParts` |

sleep 参数要求 int，范围 `0..86400000`，越界报 `ValueError`。等待期间会检查终端中断，SIGINT/SIGTERM 可使其提前结束。系统时间早于 Unix epoch 报 `ClockError`。游戏帧间隔使用单调时间，不使用可能跳变的墙上时间。

`format_utc` 对负时间戳按 UTC 向前推算；结果年份超出 `0..9999` 报 `ValueError`。`parts` 返回字段 `year/month/day/hour/minute/second/millisecond/weekday`，其中 `weekday` 为 `0=Sunday` 到 `6=Saturday`。两者都只做 UTC，不读取本地时区。

```prim
std.assert_eq(std.time.format_utc(0), "1970-01-01 00:00:00");
let t = std.time.parts(1700000000000);
std.assert_eq(t.year, 2023);
std.assert_eq(t.weekday, 2);
```

## 7. 随机数（9 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.random.seed(seed)` | int 的 64 位位模式作为种子，重置当前 Runtime 的序列，返回 `()` |
| `std.random.int(lo, hi)` | 返回 `[lo, hi)` 内的均匀 int；要求两个 int 且 lo < hi |
| `std.random.float()` | 返回 `[0.0, 1.0)` 内的 float |
| `std.random.bool()` | 以相等概率返回 true 或 false |
| `std.random.float_range(lo, hi)` | 返回 `[lo, hi)` 内的 float；要求有限且 lo < hi |
| `std.random.choice(items)` | 从非空闭包中均匀选一个字段值 |
| `std.random.shuffle(items)` | 原地 Fisher–Yates 洗牌所有可写字段，返回 `()` |
| `std.random.bytes(count)` | 返回 `count` 个 `0..255` int 的数组 |
| `std.random.normal()` | 返回标准正态分布 float（Box–Muller） |

实现采用每个 Runtime 独立的 SplitMix64 状态；整数范围使用拒绝采样避免取模偏差，支持跨负数的完整 i64 区间。`choice/shuffle` 也使用拒绝采样。默认种子来自系统时间与进程号；设定相同种子及调用顺序得到相同序列。适用于游戏与可复现示例，不是密码学随机源。非法区间、空闭包、负 count 或超过 1000000 的 bytes 数量分别报 `ValueError`/`IndexError`/`ResourceLimit`；`shuffle` 遇到只读字段报 `ReadOnlySlot`。

```prim
std.random.seed(7);
let dice = std.random.int(1, 7);
let card = std.random.choice(deck);
std.random.shuffle(deck);
```

## 8. 字符串（21 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.text.at(text, index)` | 返回指定 Unicode 标量组成的一字符字符串 |
| `std.text.slice(text, start, end)` | 返回半开区间 `[start,end)` |
| `std.text.repeat(text, count)` | 重复 count 次 |
| `std.text.trim(text)` | 去掉首尾 Unicode 空白 |
| `std.text.contains(text, needle)` | 是否包含子串 |
| `std.text.starts_with(text, prefix)` | 是否以前缀开头 |
| `std.text.ends_with(text, suffix)` | 是否以后缀结尾 |
| `std.text.replace(text, from, to)` | 替换全部不重叠匹配；from 为空时在字符边界插入 |
| `std.text.index_of(text, needle)` | 第一次出现的 Unicode 标量下标；不存在返回 `()` |
| `std.text.last_index_of(text, needle)` | 最后一次出现的 Unicode 标量下标；不存在返回 `()` |
| `std.text.split(text, separator)` | 按分隔符切分为字符串数组 |
| `std.text.lines(text)` | 按 LF/CRLF 切分为行数组 |
| `std.text.join(parts, separator)` | 用分隔符连接闭包中的全部字符串字段 |
| `std.text.upper(text)` | 按 Unicode 映射转大写 |
| `std.text.lower(text)` | 按 Unicode 映射转小写 |
| `std.text.pad_start(text, width, fill)` | 在左侧填充一个 Unicode 标量直到宽度 |
| `std.text.pad_end(text, width, fill)` | 在右侧填充一个 Unicode 标量直到宽度 |
| `std.text.code_at(text, index)` | 返回该下标的 Unicode 标量值 int |
| `std.text.from_code(code)` | 从 Unicode 标量值构造一字符字符串 |
| `std.text.fixed(value, digits)` | 将 float 格式化为固定小数位字符串 |
| `std.text.parse_int_radix(text, base)` | 按 2–36 进制解析带可选符号的 i64；失败返回 `()` |

索引和次数为非负 int。`at` 要求 index < len，slice 要求 `0 <= start <= end <= len`；越界报 `IndexError`。repeat/replace 的输出分配限制为 16 MiB，超过报 `ResourceLimit`。其他文本接口不改变原字符串。

`index_of`/`last_index_of` 返回标量下标，不是字节偏移；空 needle 分别返回 `0` 和字符串长度。`split` 使用非空分隔符时保留空片段，例如 `"a,,b"` 得到 3 项；空分隔符按 Unicode 标量拆分，空字符串得到空数组。`lines` 接受 LF 和 CRLF，末尾换行不额外产生空行。`join` 按字段声明顺序读取，要求每个字段都是 str，输出超过 16 MiB 报 `ResourceLimit`。

`upper/lower` 使用 Rust Unicode 大小写映射，结果可能改变标量数量。`pad_start/pad_end` 的 `fill` 必须恰好是一个 Unicode 标量，宽度按标量数计算；已足够宽时原样返回。`from_code` 拒绝代理项和大于 `0x10FFFF` 的值，报 `ValueError`。`fixed` 的 digits 为 `0..17`，四舍五入遵循 Rust 浮点格式；更大的 digits 报 `ValueError`。`parse_int_radix` 先去首尾空白，接受可选 `+/-`，不接受进制前缀或下划线分组。

```prim
let parts = std.text.split("alpha,beta,gamma", ",");
std.assert_eq(std.text.join(parts, " | "), "alpha | beta | gamma");
std.assert_eq(std.text.index_of("你好😀", "😀"), 2);
std.assert_eq(std.text.pad_start("7", 3, "0"), "007");
```

## 9. 数学（25 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.math.abs(number)` | 绝对值，保持种类；i64 最小值报溢出 |
| `std.math.min(a, b)` | 同种数字中的较小值 |
| `std.math.max(a, b)` | 同种数字中的较大值 |
| `std.math.clamp(value, lo, hi)` | 将同种数字限制到闭区间，要求 lo <= hi |
| `std.math.floor(x)` | float 向负无穷取整，返回 float |
| `std.math.ceil(x)` | float 向正无穷取整，返回 float |
| `std.math.round(x)` | float 取最近整数，中点远离零，返回 float |
| `std.math.sqrt(x)` | float 平方根 |
| `std.math.pow(x, exponent)` | 两个 float 的幂运算 |
| `std.math.sin(x)` | float 弧度正弦 |
| `std.math.cos(x)` | float 弧度余弦 |
| `std.math.tan(x)` | float 弧度正切 |
| `std.math.asin(x)` | float 反正弦 |
| `std.math.acos(x)` | float 反余弦 |
| `std.math.atan(x)` | float 反正切 |
| `std.math.atan2(y, x)` | 结合象限的 y/x 反正切 |
| `std.math.exp(x)` | e 的 x 次幂 |
| `std.math.ln(x)` | 自然对数 |
| `std.math.log(x, base)` | 指定底数的对数 |
| `std.math.log2(x)` | 以 2 为底的对数 |
| `std.math.log10(x)` | 以 10 为底的对数 |
| `std.math.trunc(x)` | float 向零取整，返回 float |
| `std.math.hypot(x, y)` | `sqrt(x*x + y*y)`，避免中间溢出 |
| `std.math.pi()` | 圆周率 float |
| `std.math.sign(number)` | 返回 int `-1/0/1`；正负浮点零均返回 0 |

min/max/clamp 不隐式混合 int 与 float。floor/ceil/round/sqrt/pow/sin/cos/tan/asin/acos/atan/atan2/exp/ln/log/log2/log10/trunc/hypot 要求 float，整数需显式 `std.float`。非法 clamp 区间报 `ValueError`；产生 NaN/无穷的数学运算报 `FloatRangeError`，包括定义域外的 `sqrt/asin/acos/ln`、底数非法的 `log` 和溢出的 `exp/pow`。

## 10. 位运算（9 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.bits.and(a, b)` | 按位与 |
| `std.bits.or(a, b)` | 按位或 |
| `std.bits.xor(a, b)` | 按位异或 |
| `std.bits.not(value)` | 按位取反 |
| `std.bits.shl(value, bits)` | 左移 0–63 位，丢弃高位 |
| `std.bits.shr(value, bits)` | 算术右移 0–63 位，保留符号 |
| `std.bits.count_ones(value)` | 统计置位数量 |
| `std.bits.leading_zeros(value)` | 64 位表示中的前导零数量 |
| `std.bits.trailing_zeros(value)` | 64 位表示中的尾随零数量 |

全部按 i64 的二进制补码表示操作。移位数量必须是 `0..63` 的 int，越界报 `ValueError`；`shl` 使用环绕语义，不报整数溢出。`count_ones/leading_zeros/trailing_zeros` 返回 `0..64`。

```prim
std.assert_eq(std.bits.and(0xF0, 0x3C), 0x30);
std.assert_eq(std.bits.count_ones(0xFF), 8);
std.assert_eq(std.bits.shl(1, 10), 1024);
```

## 11. 序列操作（7 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.seq.slice(items, start, end)` | 复制 `[start,end)` 字段到新数组 |
| `std.seq.reverse(items)` | 原地反转字段，返回 `()` |
| `std.seq.sort(items)` | 原地稳定排序同种 int/float/str 字段 |
| `std.seq.fill(items, value)` | 将所有字段设为同一个值 |
| `std.seq.index_of(items, value)` | 返回第一个值相等的字段下标；不存在返回 `()` |
| `std.seq.copy(items)` | 复制字段到新数组，槽相互独立 |
| `std.seq.concat(left, right)` | 连接两个闭包的字段到新数组 |

`items` 可以是 `std.array` 生成的数组，也可以是普通数据闭包；操作按字段声明顺序进行。`slice` 要求 `0 <= start <= end <= len`，越界报 `IndexError`。`reverse/sort/fill` 原地写回，遇到只读字段报 `ReadOnlySlot`。`sort` 要求所有字段同属 int、float 或 str，混合种类或 bool/unit/closure 报 `TypeError`；float 必须是有限值。`copy/slice/concat` 是浅复制，闭包元素仍共享身份。`concat` 的结果超过 1000000 个字段报 `ResourceLimit`。

```prim
let scores = std.array(4, 0);
std.set(scores, 0, 30); std.set(scores, 1, 10);
std.set(scores, 2, 20); std.set(scores, 3, 10);
std.seq.sort(scores);
std.assert_eq(std.get(scores, 0), 10);
std.seq.reverse(scores);
std.assert_eq(std.get(scores, 0), 30);
```

## 12. 脚本环境与进程（9 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.env.argc()` | `--` 后脚本参数的数量 |
| `std.env.arg(index)` | 指定参数字符串；越过末尾返回 `()`，负下标报 `IndexError` |
| `std.env.get(name)` | 环境变量字符串；不存在返回 `()` |
| `std.env.cwd()` | 当前工作目录字符串 |
| `std.env.args()` | 全部脚本参数组成的字符串数组 |
| `std.env.home()` | 家目录字符串；无法确定时返回 `()` |
| `std.env.temp_dir()` | 平台临时目录字符串 |
| `std.process.pid()` | 当前进程 id |
| `std.process.exit(code)` | 立即退出，code 为 `0..255` |

```bash
prim program.pr -- first second
```

上述参数为 `arg(0)=="first"`、`arg(1)=="second"`，不包含程序名、脚本路径或 Prim CLI 选项；`args()` 返回同样顺序的数组。环境变量名为空或包含 `=`/NUL 报 `ValueError`；环境值无法表示为 UTF-8 报 `IoError`。cwd 使用系统路径的 UTF-8 有损显示形式。

`env.home()` 读取宿主 `HOME`，未设置或不是 UTF-8 时返回 `()`。`temp_dir()` 使用平台临时目录，通常遵循 `TMPDIR`。`process.exit` 先刷新 stdout/stderr 并恢复终端，再以给定状态码终止进程；它不会执行后续语句或 GC。状态码越界报 `ValueError`。

## 13. 文本文件与目录（13 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.fs.read_text(path)` | 读取 UTF-8 文件，返回 str |
| `std.fs.write_text(path, text)` | 创建或截断覆盖文件，返回 `()` |
| `std.fs.append_text(path, text)` | 创建或追加文件，返回 `()` |
| `std.fs.exists(path)` | 返回路径是否存在；权限等查询错误仍报错 |
| `std.fs.list_dir(path)` | 返回按名称排序的目录项数组，不含 `.`/`..` |
| `std.fs.mkdir_all(path)` | 创建目录及缺失父目录，已存在时成功 |
| `std.fs.remove_file(path)` | 删除文件 |
| `std.fs.remove_dir(path)` | 删除空目录 |
| `std.fs.rename(from, to)` | 重命名或移动文件/目录 |
| `std.fs.copy_file(from, to)` | 复制文件，覆盖目标 |
| `std.fs.file_size(path)` | 返回文件字节数 int |
| `std.fs.is_file(path)` | 是否普通文件 |
| `std.fs.is_dir(path)` | 是否目录 |

路径是 str，相对当前工作目录解析。父目录不会自动创建（`mkdir_all` 除外）。文件系统权限沿用宿主进程；这些接口没有额外的沙箱目录。读/单次写入限制 16 MiB，超过报 `ResourceLimit`；打开、读取、写入或 UTF-8 错误报 `IoError`。文本写入不是事务写入，写入失败可能留下部分内容。

`list_dir` 只返回条目名，不返回完整路径；目录项名不是有效 UTF-8 时返回 `IoError`。`is_file/is_dir` 在权限不足或路径不存在时返回 false。`copy_file` 使用宿主文件复制，不受 16 MiB 文本限制；目标被覆盖。删除和重命名是真实文件系统操作，调用前应确认路径。

```prim
std.fs.mkdir_all("output/logs");
std.fs.write_text("output/logs/run.txt", "ok\n");
let names = std.fs.list_dir("output/logs");
std.assert_eq(std.get(names, 0), "run.txt");
```

## 14. 交互终端（14 个）

当前终端实现面向 Linux x86_64，使用 termios、poll、ioctl 和 ANSI 控制序列。

| 签名 | 返回值与行为 |
| --- | --- |
| `std.term.enter()` | 要求 stdin/stdout 均为 TTY；进入备用屏幕、隐藏光标、关闭规范输入与回显，返回 `()` |
| `std.term.leave()` | 恢复终端属性、默认颜色、光标和主屏幕；可重复调用，返回 `()` |
| `std.term.is_tty()` | stdin 与 stdout 是否均为终端 |
| `std.term.size()` | 返回可写数据闭包 `{ width, height }`，单位为字符单元；无法查询时使用 80×24 |
| `std.term.clear()` | 清屏并移动到左上角 |
| `std.term.home()` | 只移动到左上角 |
| `std.term.move(x, y)` | 移动到零基列/行；int 范围各为 `0..10000` |
| `std.term.cursor(visible)` | bool 控制光标显示 |
| `std.term.color(foreground, background)` | 设置前景/背景颜色，参数为 int `-1` 或 `0..15` |
| `std.term.color_rgb(foreground, background)` | 设置 24 位颜色，参数为 `-1` 或 `0xRRGGBB` |
| `std.term.reset()` | 重置颜色/样式 |
| `std.term.bell()` | 响铃 |
| `std.term.title(text)` | 设置终端窗口标题 |
| `std.term.poll_key(timeout_ms)` | 读取一个逻辑按键；超时返回 `()`，EOF 返回 `"eof"` |

enter/leave 立即刷新屏幕控制序列。clear/home/move/cursor/color/color_rgb/reset/bell/title 只写入，使用 `std.flush()` 结束一帧。绘制和 poll_key 要求先 enter；size/is_tty/leave 可在未进入时调用。进入终端模式要求 Runtime 的 `echo_output=true`，CLI 已设置。

颜色 `-1` 表示终端默认，`0..7` 为黑、红、绿、黄、蓝、品红、青、白，`8..15` 为相应亮色。`color_rgb` 把三个通道打包为 `0xRRGGBB`，`-1` 保持默认；终端不支持真彩时由终端自行近似。`title` 拒绝控制字符，长度限制 1024 字节。

poll_key 的 timeout_ms 是 int：`0` 立即检查，`-1` 一直等待，`1..60000` 最多等待对应毫秒。解析一个已经开始的转义/UTF-8 按键序列可能额外等待几十毫秒。

返回字符串包括：

- `up/down/left/right/home/end/delete`。
- `enter/tab/backspace/escape/ctrl_d`。
- 普通 ASCII 或 Unicode 字符本身，例如 `"w"`、`" "`、`"é"`。
- `unknown`：当前不支持的控制键或转义序列。
- `eof`：输入关闭。

Ctrl+C 通常由保留的 ISIG 处理为 `Interrupted` 错误，而非普通按键；收到原始字节 3 的情况下也能解码为 `ctrl_c`。终端模式期间 SIGINT/SIGTERM 设置中断标志，运行时操作、按键等待和 sleep 检查它，并在错误返回后恢复终端。正常退出、语言错误与 Runtime 析构同样执行恢复。强制 SIGKILL、进程崩溃或终端设备已经关闭不属于可保证恢复的路径。

```prim
if std.term.is_tty() {
    std.term.enter();
    std.term.color_rgb(0x00FF88, -1);
    std.term.title("Prim");
    std.write("Press a key..."); std.flush();
    let key = std.term.poll_key(-1);
    std.term.leave();
    std.print(key);
};
```

## 15. 宿主信息（2 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.sys.os()` | 编译目标的操作系统名，例如 `linux` |
| `std.sys.arch()` | 编译目标的 CPU 架构名，例如 `x86_64` |

这些值来自 Rust 的编译期目标常量，不执行外部命令，也不读取运行时环境。

## 16. AST 解析（7 个）

`std.ast` 使用与运行时相同的 Lexer/Parser，只做语法解析，不执行源码、不做名字解析。输入上限 1 MiB，结果上限 8192 个节点或 Token；解析失败报 `AstError`，错误消息包含原始字节区间。返回的记录与数组槽都是只读快照。

| 签名 | 返回值与行为 |
| --- | --- |
| `std.ast.tokens(source)` | 返回 `ASTToken` 数组，包含 EOF；字段为 `kind/text/value/span` |
| `std.ast.parse(source)` | 返回 `ASTNode` 树 |
| `std.ast.dump(tree)` | 返回缩进的可读树文本 |
| `std.ast.walk(tree)` | 按深度优先前序返回节点数组 |
| `std.ast.find(tree, kind)` | 返回 `kind` 字段完全相等的节点数组 |
| `std.ast.source_segment(source, node)` | 按节点的字节区间截取原始源码 |
| `std.ast.is_node(value)` | 检查值是否具备 `ASTNode` 的浅层记录形状 |

`ASTNode` 字段为 `kind`、`span`、`attributes`、`children`；`ASTSpan` 字段为 `start/end`（UTF-8 字节偏移）与 `line/column/end_line/end_column`（1 基行列）。`ASTToken.kind` 使用 `Identifier/Integer/Float/String/Keyword/Symbol/Eof`，`text` 是原始词素，`value` 是解码值，EOF 的 `value` 为 `()`。

```prim
let source = "let x = 1; x + 2";
let tree = std.ast.parse(source);
let declarations = std.ast.find(tree, "Let");
std.assert_eq(std.len(declarations), 1);
std.print(std.ast.source_segment(source, std.get(declarations, 0)));
std.print(std.ast.dump(tree));
```

## 17. 运行时检查（21 个）

`std.inspect` 读取当前 Runtime 的闭包、函数、调用栈、JIT 缓存和机器码元数据。除非明确调用 `compile`，查询不会触发 JIT 编译；所有返回记录都是只读快照。

| 签名 | 返回值与行为 |
| --- | --- |
| `std.inspect.is_callable(value)` | 是否可调用闭包（含宿主内置） |
| `std.inspect.is_function(value)` | 是否是 Prim 用户函数 |
| `std.inspect.is_builtin(value)` | 是否是宿主内置函数 |
| `std.inspect.is_jit(value)` | 是否带显式 JIT 策略（即使 `--jit=off`） |
| `std.inspect.describe(value)` | 返回 `InspectDescription` |
| `std.inspect.members(object)` | 返回全部字段的 `InspectMember` 数组 |
| `std.inspect.signature(function)` | 返回 `InspectSignature` |
| `std.inspect.captures(function)` | 返回用户函数捕获的 `InspectCapture` 数组 |
| `std.inspect.source(object)` | 返回用户函数原始源码；不可用时返回 `()` |
| `std.inspect.location(object)` | 返回 `SourceLocation`；不可用时返回 `()` |
| `std.inspect.ast(function)` | 解析该函数源码并返回 `ASTNode` |
| `std.inspect.core_ir(function)` | 返回 Core IR 文本 |
| `std.inspect.stack()` | 返回当前调用帧的 `InspectFrame` 数组 |
| `std.inspect.builtins()` | 返回全部内置的 `{name, arity}` 数组 |
| `std.inspect.jit(function)` | 返回 `JitInfo`，只查询缓存状态 |
| `std.inspect.compile(function)` | 编译显式 `@jit` 函数但不执行函数体，返回原闭包 |
| `std.inspect.llvm_ir(function)` | 返回已编译函数的优化前 LLVM IR 文本 |
| `std.inspect.optimized_ir(function)` | 返回优化后实际提交给 ORC 的 LLVM IR 文本 |
| `std.inspect.machine_code(function)` | 返回已加载机器码的十六进制文本 |
| `std.inspect.disassemble(function)` | 返回反汇编文本 |
| `std.inspect.native(function)` | 返回 `NativeCode` 记录 |

`InspectDescription` 字段：`type`、`kind`（`object/function/builtin` 或基础值种类）、`display`、`name`、`closure_id`、`code_id`、`arity`、`field_count`、`callable`、`policy`（`none/host/interpreted/jit`）。`InspectMember` 字段：`name/slot_id/type/value/writable`。`InspectSignature` 字段：`name/arity/parameters/text`，参数项为 `{name, index, by_ref}`。`InspectCapture` 字段：`name/mode/slot_id/value/writable`，mode 为 `value` 或 `slot`。`InspectFrame` 字段：`depth/code_id/closure_id/pc/mode/location`。`JitInfo` 与 `NativeCode` 的字段见 `builtin/types.pri`；字段语义、节点种类及机器码来源详见 [内省与机器码展示](introspection.md)。

`ast/core_ir/captures/jit` 要求 Prim 用户函数，其他闭包报 `InspectError`；`source/location` 对其他闭包或不可用源码返回 `()`。`llvm_ir/optimized_ir/machine_code/disassemble/native` 要求该函数的 CodeId 已经编译，否则报 `JitNotCompiled`；`compile` 要求显式 JIT 策略且配置允许 JIT，否则报 `JitTargetError` 或 `JitDisabled`。机器码查询失败报 `NativeInspectionError`。

```prim
let double = @jit $[](x) { x * 2 };
std.print(std.inspect.describe(double).kind);
std.inspect.compile(double);
std.print(std.inspect.jit(double).state);
std.print(std.inspect.signature(double).text);
```

计数及编译产物按 CodeId 共享，执行策略属于闭包实例；快照不会自动更新。capture/member 的 value 保持普通值语义，若是对象仍共享原对象身份，不能把元数据只读理解为用户对象的深度冻结。

汇报用完整程序：`prim src/prim-rs/examples/inspect.pr`。它依次展示源码、AST、Core IR、LLVM IR、从已加载入口复制的机器码、反汇编，以及执行前后的计数。

JIT 快照还包含 `opt_level/lowering/optimized_ir_bytes/scalar_calls/scalar_fallbacks`，字段语义与完整示例见 [内省文档](introspection.md) 和 [性能优化](optimization.md)。

## 18. 内存诊断（2 个）

| 签名 | 返回值与行为 |
| --- | --- |
| `std.gc.collect()` | 在合法运行时安全点主动标记清扫，返回 `()` |
| `std.gc.stats()` | 先收集，再返回内存统计数据闭包 |

stats 字段：`slots`、`closures`、`strings`、`string_bytes`、`slot_capacity`、`closure_capacity`、`collections`、`output_bytes`，均为 int。

统计快照在返回的统计闭包分配之前取得；字符串按唯一 Rc 对象计数，string_bytes 是其 UTF-8 内容长度，不包含 allocator 元数据、闭包字段名、程序常量或 LLVM 内存。arena capacity 与存活数量不同，详细解释见 [内存管理](memory-management.md)。

## 19. 贪吃蛇与验证

```bash
build/prim-rs/prim src/prim-rs/examples/snake.pr
build/prim-rs/prim --stats src/prim-rs/examples/snake.pr -- --self-test
build/prim-rs/prim --jit=off src/prim-rs/examples/snake.pr -- --self-test
```

终端至少 52 列、21 行。方向键/WASD 移动，P 或空格暂停/继续，R 重开，Q/Escape 退出。地图缩小时提示调整窗口并暂停移动；吃食物加分、增长并逐步加速；撞墙/撞身体结束，填满地图获胜。

游戏逻辑完全用 Prim 实现，`step` 由 `@jit` 手动编译。自检验证移动、增长、反向保护、暂停、墙/身体碰撞、进入正在腾出的尾格、胜利和重开。

CPU pod 验证入口：

```bash
CARGO_TARGET_DIR=/tmp/prim-rs-target cargo test --release --offline --locked
uv run --offline --no-project python scripts/prim-rs/test_interactive.py
```

伪终端检查真实按键输入、分段方向键序列、Unicode、Ctrl+C/SIGTERM、JIT 错误后的终端恢复，以及贪吃蛇暂停、重开、缩放与退出。
