# Prim Rust Lexer 实现

本文记录 [`syntax.rs`](../src/prim-rs/src/syntax.rs) 的当前实现。源码后缀为 `.pr`。词素规则见 [lexical-spec.md](lexical-spec.md)，语法见 [parser-design.md](parser-design.md)。

## 1. 输入、输出与错误策略

```rust
pub fn lex(source: &str) -> Result<Vec<Token>>;

pub struct Span {
    pub start: usize,
    pub end: usize,
}

pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
```

CLI 使用 Rust 的 UTF-8 文本读取接口，非法 UTF-8 文件在进入 lex 前成为文件读取错误。lex 接收已经有效的 `&str`，拒绝文件开头的 BOM 与任意位置的原始 NUL。

Span 为源字符串零基字节偏移、半开区间 `[start,end)`，没有 SourceId。CLI 在格式化错误时另外传入文件名与源码。成功结果最后恰好一个 Eof Token；任何错误立即返回 Err，不产生 Error Token，不继续恢复扫描。

当前 Lexer 是手写状态机，不依赖 re2c、Bison 或正则表达式库。标识符和数字词素使用拥有所有权的 String；字符串在词法阶段解码为 String。没有 SourceManager、字符串驻留表或指向 TokenBuffer 内部的裸指针。

## 2. 实际 TokenKind

```rust
pub enum TokenKind {
    Ident(String),
    Int(String),
    Float(String),
    Str(String),
    Word(&'static str),
    Sym(&'static str),
    Eof,
}
```

| 输入 | TokenKind 内容 |
| --- | --- |
| `counter` | `Ident("counter")` |
| `let` | `Word("let")` |
| `0xFF`、`123` | `Int`，保留进制前缀与分组符的原词素 |
| `1.5e2` | `Float("1.5e2")` |
| `"a\n"` | `Str`，内容是 a 和真正的换行字符 |
| `+`、`&&`、`@`、`$` | 对应的 `Sym` |
| 文件结束 | `Eof`，Span 位于文件长度处 |

文法中用 IDENT/INT_DEC 等大写记号表示词法类别，不能把它们理解为 Rust 中实际存在的枚举变体。

## 3. 主扫描循环

按下列顺序处理：

1. 跳过空格、Tab、LF、CRLF、CR。
2. 检查 `//` 和 `/*`，先于普通 `/` 运算符。
3. 扫描 ASCII 标识符，完成后对整个词素查关键字表。
4. 扫描数字候选并检查格式。
5. 扫描双引号字符串并解码转义。
6. 按长符号优先的表匹配运算符与标点。
7. 无匹配即报 IllegalCharacter。

Lexer 不维护括号匹配栈，不区分负号和减号，不识别装饰器的语义。`@jit` 是 `Sym("@")` 与 `Ident("jit")`。

源码以 UTF-8 字节切分，但遇到需要报告的非 ASCII 字符时读取完整 Unicode 标量，Span 不会把该字符拆成多条错误。

## 4. 数字状态

- 起始 `0x/0o/0b` 选择相应基数，其余按十进制。
- 同一个数字组允许两位数字之间的单个 `_`。
- 十进制中，小数点后紧跟数字才纳入字面量。
- 十进制 `e/E` 开始指数段，可接一个正负号；十六进制的 e 是数字。
- 连续字母、数字、下划线纳入候选检查，不能把 `123abc` 当成整数和名字两个合法 Token。
- 完整候选由 `valid_number`、`digit_group` 检查，错误返回 InvalidNumber。

数值范围由 `ir::compile` 检查：Lexer 只保留原始词素。这样 `-9223372036854775808` 可以先输出负号与整数幅值，再由编译阶段识别最小负数。

## 5. 字符串状态

普通 Unicode 字符按字符边界推进；识别 `\"`、`\\`、`\n`、`\r`、`\t`、`\0`、`\u{...}`。Unicode 转义限制 1–6 位十六进制、合法标量值，排除代理项。

双引号关闭字符串，实际 CR/LF 在闭引号前出现时立即报 UnterminatedString。非法转义或原始控制字符立即报 InvalidEscape，不继续扫描剩余字符串。解码的 NUL 保存在有长度的 Rust String 中。

EOF 前没有闭引号报 UnterminatedString。所有诊断使用原源码字节范围，不使用解码后的字符串偏移。

## 6. 注释

行注释消费到 CR/LF/EOF；块注释消费到第一个 `*/`，不嵌套。块注释到 EOF 未结束报 UnterminatedComment。注释不连接两侧标识符，也不会处理其中的引号或括号。

## 7. 诊断数据与显示

```rust
pub struct Error {
    pub kind: String,
    pub message: String,
    pub span: Span,
    pub trace: Vec<Span>,
}
```

词法错误类别为 ForbiddenSourceCharacter、IllegalCharacter、InvalidNumber、InvalidEscape、UnterminatedString、UnterminatedComment。

渲染器按源码计算行号与列号，CRLF 视为一次换行，单独 CR 同样换行；Tab 使用 4 列停止位。显示列按 Unicode 标量计数，不保证宽字符/组合字素在所有终端上的视觉宽度。调用栈 trace 由运行时添加，不由 Lexer 生成。

当前没有多错误列表、自动修复或错误 Token；这是实际的遇错即停接口。

## 8. 可执行检查

```bash
build/prim-rs/prim --tokens src/prim-rs/examples/hello.pr
build/prim-rs/prim --check src/prim-rs/examples/snake.pr
```

测试覆盖进制与分组边界、坏数字、转义、EOF、Unicode 及 CR/LF/CRLF 位置。词法成功不代表语法成功，例如 `([)]` 会形成正常符号 Token，但 Parser 会拒绝。
