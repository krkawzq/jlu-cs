use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
impl Span {
    pub fn join(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Error {
    pub kind: String,
    pub message: String,
    pub span: Span,
    pub trace: Vec<Span>,
}
impl Error {
    pub fn new(kind: &str, message: impl Into<String>, span: Span) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            span,
            trace: vec![],
        }
    }
    pub fn render(&self, name: &str, source: &str) -> String {
        let position = source.floor_char_boundary(self.span.start.min(source.len()));
        let (line, begin) = line_position(source, position);
        let end = source[position..]
            .find(['\n', '\r'])
            .map_or(source.len(), |n| position + n);
        let column = expand_tabs(&source[begin..position]).chars().count() + 1;
        let mut result = format!(
            "{}: {}\n --> {}:{}:{}\n  |\n{line:>2}| {}\n  | {}^\n",
            self.kind,
            self.message,
            name,
            line,
            column,
            expand_tabs(&source[begin..end]),
            " ".repeat(column - 1)
        );
        for span in self.trace.iter().rev().take(12) {
            let p = span.start.min(source.len());
            let (line, _) = line_position(source, p);
            result.push_str(&format!("  called at {name}:{line}\n"));
        }
        result
    }
}
fn line_position(source: &str, position: usize) -> (usize, usize) {
    let bytes = source.as_bytes();
    let (mut line, mut begin, mut i) = (1, 0, 0);
    while i < position {
        match bytes[i] {
            b'\r' => {
                i += 1;
                if i < position && bytes[i] == b'\n' {
                    i += 1;
                }
                line += 1;
                begin = i;
            }
            b'\n' => {
                i += 1;
                line += 1;
                begin = i;
            }
            _ => i += 1,
        }
    }
    (line, begin)
}
fn expand_tabs(source: &str) -> String {
    let mut result = String::new();
    let mut width = 0;
    for ch in source.chars() {
        if ch == '\t' {
            let n = 4 - width % 4;
            result.push_str(&" ".repeat(n));
            width += n;
        } else {
            result.push(ch);
            width += 1;
        }
    }
    result
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Int(String),
    Float(String),
    Str(String),
    Word(&'static str),
    Sym(&'static str),
    Eof,
}
#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn lex(source: &str) -> Result<Vec<Token>> {
    let b = source.as_bytes();
    let mut i = 0;
    let mut out = vec![];
    if source.starts_with('\u{feff}') || b.contains(&0) {
        return Err(Error::new(
            "ForbiddenSourceCharacter",
            "BOM and raw NUL are not allowed",
            Span::default(),
        ));
    }
    while i < b.len() {
        let start = i;
        if b[i].is_ascii_whitespace() && matches!(b[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"//") {
            while i < b.len() && !matches!(b[i], b'\n' | b'\r') {
                i += 1;
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i += 2;
            while i < b.len() && !b[i..].starts_with(b"*/") {
                i += 1;
            }
            if i == b.len() {
                return Err(Error::new(
                    "UnterminatedComment",
                    "expected */",
                    Span { start, end: i },
                ));
            }
            i += 2;
            continue;
        }
        let kind = if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let name = &source[start..i];
            match name {
                "let" => TokenKind::Word("let"),
                "if" => TokenKind::Word("if"),
                "else" => TokenKind::Word("else"),
                "loop" => TokenKind::Word("loop"),
                "break" => TokenKind::Word("break"),
                "continue" => TokenKind::Word("continue"),
                "return" => TokenKind::Word("return"),
                "true" => TokenKind::Word("true"),
                "false" => TokenKind::Word("false"),
                "self" => TokenKind::Word("self"),
                _ => TokenKind::Ident(name.into()),
            }
        } else if b[i].is_ascii_digit() {
            let prefixed = b[i] == b'0'
                && i + 1 < b.len()
                && matches!(b[i + 1], b'x' | b'X' | b'o' | b'O' | b'b' | b'B');
            if prefixed {
                i += 2;
            }
            let mut dot = false;
            let mut exponent = false;
            while i < b.len() {
                if !prefixed && matches!(b[i], b'e' | b'E') && !exponent {
                    exponent = true;
                    i += 1;
                    if i < b.len() && matches!(b[i], b'+' | b'-') {
                        i += 1;
                    }
                } else if b[i].is_ascii_alphanumeric() || b[i] == b'_' {
                    i += 1;
                } else if !prefixed
                    && !dot
                    && !exponent
                    && b[i] == b'.'
                    && i + 1 < b.len()
                    && b[i + 1].is_ascii_digit()
                {
                    dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            let raw = &source[start..i];
            if !valid_number(raw, prefixed) {
                return Err(Error::new(
                    "InvalidNumber",
                    format!("invalid number {raw}"),
                    Span { start, end: i },
                ));
            }
            if dot || exponent {
                TokenKind::Float(raw.into())
            } else {
                TokenKind::Int(raw.into())
            }
        } else if b[i] == b'"' {
            i += 1;
            let mut value = String::new();
            let mut closed = false;
            while i < b.len() {
                let ch = source[i..].chars().next().unwrap();
                i += ch.len_utf8();
                match ch {
                    '"' => {
                        closed = true;
                        break;
                    }
                    '\n' | '\r' => {
                        return Err(Error::new(
                            "UnterminatedString",
                            "newline in string",
                            Span { start, end: i },
                        ));
                    }
                    '\\' => {
                        if i == b.len() {
                            break;
                        }
                        let esc = b[i];
                        i += 1;
                        match esc {
                            b'"' => value.push('"'),
                            b'\\' => value.push('\\'),
                            b'n' => value.push('\n'),
                            b'r' => value.push('\r'),
                            b't' => value.push('\t'),
                            b'0' => value.push('\0'),
                            b'u' => {
                                let begin = i;
                                if i == b.len() || b[i] != b'{' {
                                    return Err(Error::new(
                                        "InvalidEscape",
                                        "expected \\u{hex}",
                                        Span {
                                            start: begin - 2,
                                            end: i,
                                        },
                                    ));
                                }
                                i += 1;
                                let hex = i;
                                while i < b.len() && b[i].is_ascii_hexdigit() {
                                    i += 1;
                                }
                                if i == b.len() || b[i] != b'}' || i == hex || i - hex > 6 {
                                    return Err(Error::new(
                                        "InvalidEscape",
                                        "invalid Unicode escape",
                                        Span {
                                            start: begin - 2,
                                            end: i,
                                        },
                                    ));
                                }
                                let code = u32::from_str_radix(&source[hex..i], 16).unwrap();
                                i += 1;
                                value.push(char::from_u32(code).ok_or_else(|| {
                                    Error::new(
                                        "InvalidEscape",
                                        "not a Unicode scalar",
                                        Span {
                                            start: begin - 2,
                                            end: i,
                                        },
                                    )
                                })?);
                            }
                            _ => {
                                return Err(Error::new(
                                    "InvalidEscape",
                                    "unknown string escape",
                                    Span {
                                        start: i - 2,
                                        end: i,
                                    },
                                ));
                            }
                        }
                    }
                    c if c < ' ' || c == '\u{7f}' => {
                        return Err(Error::new(
                            "InvalidEscape",
                            "raw control character in string",
                            Span {
                                start: i - 1,
                                end: i,
                            },
                        ));
                    }
                    c => value.push(c),
                }
            }
            if !closed {
                return Err(Error::new(
                    "UnterminatedString",
                    "expected closing quote",
                    Span { start, end: i },
                ));
            }
            TokenKind::Str(value)
        } else {
            let mut found = None;
            for symbol in [
                "==", "!=", "<=", ">=", "&&", "||", "=", "+", "-", "*", "/", "%", "<", ">", "!",
                "&", "(", ")", "[", "]", "{", "}", ".", ",", ";", "@", "$",
            ] {
                if source[i..].starts_with(symbol) {
                    found = Some(symbol);
                    break;
                }
            }
            match found {
                Some(sym) => {
                    i += sym.len();
                    TokenKind::Sym(sym)
                }
                None => {
                    let ch = source[i..].chars().next().unwrap();
                    return Err(Error::new(
                        "IllegalCharacter",
                        format!("unexpected character {ch:?}"),
                        Span {
                            start,
                            end: start + ch.len_utf8(),
                        },
                    ));
                }
            }
        };
        out.push(Token {
            kind,
            span: Span { start, end: i },
        });
    }
    out.push(Token {
        kind: TokenKind::Eof,
        span: Span { start: i, end: i },
    });
    Ok(out)
}
fn digit_group(s: &str, radix: u32) -> bool {
    let mut digit = false;
    for ch in s.chars() {
        if ch == '_' {
            if !digit {
                return false;
            }
            digit = false;
        } else if ch.is_digit(radix) {
            digit = true;
        } else {
            return false;
        }
    }
    digit
}
fn valid_number(s: &str, prefixed: bool) -> bool {
    if prefixed {
        return digit_group(
            &s[2..],
            match s.as_bytes()[1] {
                b'x' | b'X' => 16,
                b'o' | b'O' => 8,
                _ => 2,
            },
        );
    }
    let (mantissa, exponent) = s
        .find(['e', 'E'])
        .map_or((s, None), |p| (&s[..p], Some(&s[p + 1..])));
    if let Some(exp) = exponent {
        if !digit_group(exp.strip_prefix(['+', '-']).unwrap_or(exp), 10) {
            return false;
        }
    }
    if let Some(p) = mantissa.find('.') {
        digit_group(&mantissa[..p], 10) && digit_group(&mantissa[p + 1..], 10)
    } else {
        digit_group(mantissa, 10)
    }
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub enum ExprKind {
    Int(String),
    Float(String),
    Str(String),
    Bool(bool),
    Unit,
    Name(String),
    SelfValue,
    Group(Box<Expr>),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Arg>),
    Field(Box<Expr>, String),
    Block(Body),
    If(Box<Expr>, Body, Option<Box<Expr>>),
    Loop(Body),
    Data(Vec<Stmt>),
    Closure(Vec<Binding>, Vec<Binding>, Body),
    Decorate(String, Box<Expr>),
}
#[derive(Clone, Debug)]
pub struct Binding {
    pub name: String,
    pub by_ref: bool,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Arg {
    pub expr: Expr,
    pub by_ref: bool,
}
#[derive(Clone, Debug)]
pub struct Body {
    pub statements: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub enum StmtKind {
    Let(String, Arg),
    Assign(Expr, Expr),
    Expr(Expr),
    Return(Option<Expr>),
    Break(Option<Expr>),
    Continue,
}

pub fn parse(tokens: Vec<Token>) -> Result<Body> {
    let mut p = Parser {
        tokens,
        pos: 0,
        depth: 0,
    };
    let body = p.body(false, false)?;
    p.expect_eof()?;
    Ok(body)
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
}
impl Parser {
    fn token(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn is(&self, s: &str) -> bool {
        matches!(&self.token().kind, TokenKind::Word(t)|TokenKind::Sym(t) if *t == s)
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.is(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn take(&mut self) -> Token {
        let t = self.token().clone();
        if !matches!(t.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        t
    }
    fn fail<T>(&self, msg: impl Into<String>) -> Result<T> {
        Err(Error::new("SyntaxError", msg, self.token().span))
    }
    fn expect(&mut self, s: &str) -> Result<()> {
        if self.eat(s) {
            Ok(())
        } else {
            self.fail(format!("expected '{s}', found {:?}", self.token().kind))
        }
    }
    fn expect_eof(&self) -> Result<()> {
        if matches!(self.token().kind, TokenKind::Eof) {
            Ok(())
        } else {
            self.fail("expected end of file")
        }
    }
    fn name(&mut self) -> Result<(String, Span)> {
        let t = self.take();
        if let TokenKind::Ident(n) = t.kind {
            Ok((n, t.span))
        } else {
            Err(Error::new("SyntaxError", "expected identifier", t.span))
        }
    }
    fn body(&mut self, braced: bool, data: bool) -> Result<Body> {
        let start = self.token().span;
        if braced {
            self.expect("{")?;
        }
        let mut statements = vec![];
        let mut tail = None;
        while !(matches!(self.token().kind, TokenKind::Eof) || braced && self.is("}")) {
            let begin = self.token().span;
            let kind = if self.eat("let") {
                let (name, _) = self.name()?;
                self.expect("=")?;
                let arg = self.argument()?;
                self.expect(";")?;
                StmtKind::Let(name, arg)
            } else if self.eat("return") {
                let value = if self.is(";") {
                    None
                } else {
                    Some(self.expr(0)?)
                };
                self.expect(";")?;
                StmtKind::Return(value)
            } else if self.eat("break") {
                let value = if self.is(";") {
                    None
                } else {
                    Some(self.expr(0)?)
                };
                self.expect(";")?;
                StmtKind::Break(value)
            } else if self.eat("continue") {
                self.expect(";")?;
                StmtKind::Continue
            } else {
                let lhs = self.expr(0)?;
                if self.eat("=") {
                    let rhs = self.expr(0)?;
                    self.expect(";")?;
                    StmtKind::Assign(lhs, rhs)
                } else if self.eat(";") {
                    StmtKind::Expr(lhs)
                } else if !data
                    && (braced && self.is("}")
                        || !braced && matches!(self.token().kind, TokenKind::Eof))
                {
                    tail = Some(Box::new(lhs));
                    break;
                } else {
                    return self.fail(if data {
                        "data closure statements require ';'; no tail expression"
                    } else {
                        "expected ';' after expression"
                    });
                }
            };
            statements.push(Stmt {
                kind,
                span: begin.join(self.tokens[self.pos - 1].span),
            });
        }
        if braced {
            self.expect("}")?;
        }
        Ok(Body {
            statements,
            tail,
            span: start.join(self.tokens[self.pos.saturating_sub(1)].span),
        })
    }
    fn argument(&mut self) -> Result<Arg> {
        let by_ref = self.eat("&");
        let expr = self.expr(if by_ref { 70 } else { 0 })?;
        Ok(Arg { expr, by_ref })
    }
    fn bindings(&mut self, end: &str) -> Result<Vec<Binding>> {
        let mut list = vec![];
        if self.eat(end) {
            return Ok(list);
        }
        loop {
            let by_ref = self.eat("&");
            let (name, span) = self.name()?;
            list.push(Binding { name, by_ref, span });
            if self.eat(end) {
                break;
            }
            self.expect(",")?;
            if self.eat(end) {
                break;
            }
        }
        Ok(list)
    }
    fn if_expression(&mut self) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 256 {
            self.depth -= 1;
            return self.fail("syntax nesting limit (256) exceeded");
        }
        let result = self.if_expression_inner();
        self.depth -= 1;
        result
    }
    fn if_expression_inner(&mut self) -> Result<Expr> {
        let start = self.token().span;
        self.expect("if")?;
        let cond = self.expr(0)?;
        let body = self.body(true, false)?;
        let other = if self.eat("else") {
            if self.is("if") {
                Some(Box::new(self.if_expression()?))
            } else {
                let b = self.body(true, false)?;
                Some(Box::new(Expr {
                    span: b.span,
                    kind: ExprKind::Block(b),
                }))
            }
        } else {
            None
        };
        let end = other.as_ref().map_or(body.span, |e| e.span);
        Ok(Expr {
            span: start.join(end),
            kind: ExprKind::If(Box::new(cond), body, other),
        })
    }
    fn callable(&mut self) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 256 {
            self.depth -= 1;
            return self.fail("syntax nesting limit (256) exceeded");
        }
        let result = self.callable_inner();
        self.depth -= 1;
        result
    }
    fn callable_inner(&mut self) -> Result<Expr> {
        let start = self.token().span;
        if self.eat("@") {
            let (name, _) = self.name()?;
            let inner = self.callable()?;
            let span = start.join(inner.span);
            return Ok(Expr {
                kind: ExprKind::Decorate(name, Box::new(inner)),
                span,
            });
        }
        self.expect("$")?;
        self.expect("[")?;
        let captures = self.bindings("]")?;
        self.expect("(")?;
        let params = self.bindings(")")?;
        let body = self.body(true, false)?;
        Ok(Expr {
            span: start.join(body.span),
            kind: ExprKind::Closure(captures, params, body),
        })
    }
    fn expr(&mut self, min: u8) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 256 {
            return self.fail("expression nesting limit (256) exceeded");
        }
        let result = self.expr_inner(min);
        self.depth -= 1;
        result
    }
    fn expr_inner(&mut self, min: u8) -> Result<Expr> {
        let start = self.token().span;
        let mut lhs = if self.is("!") || self.is("-") || self.is("+") {
            let TokenKind::Sym(op) = self.take().kind else {
                unreachable!()
            };
            let operand = self.expr(60)?;
            Expr {
                span: start.join(operand.span),
                kind: ExprKind::Unary(op.into(), Box::new(operand)),
            }
        } else if self.is("$")
            || self.is("@")
                && !matches!(
                    self.tokens.get(self.pos + 1).map(|t| &t.kind),
                    Some(TokenKind::Sym("{"))
                )
        {
            self.callable()?
        } else if self.eat("@") {
            let body = self.body(true, true)?;
            Expr {
                span: start.join(body.span),
                kind: ExprKind::Data(body.statements),
            }
        } else if self.is("{") {
            let body = self.body(true, false)?;
            Expr {
                span: body.span,
                kind: ExprKind::Block(body),
            }
        } else if self.is("if") {
            self.if_expression()?
        } else if self.eat("loop") {
            let body = self.body(true, false)?;
            Expr {
                span: start.join(body.span),
                kind: ExprKind::Loop(body),
            }
        } else if self.eat("(") {
            if self.eat(")") {
                Expr {
                    kind: ExprKind::Unit,
                    span: start.join(self.tokens[self.pos - 1].span),
                }
            } else {
                let e = self.expr(0)?;
                self.expect(")")?;
                Expr {
                    kind: ExprKind::Group(Box::new(e)),
                    span: start.join(self.tokens[self.pos - 1].span),
                }
            }
        } else {
            let t = self.take();
            let kind = match t.kind {
                TokenKind::Ident(n) => ExprKind::Name(n),
                TokenKind::Int(n) => ExprKind::Int(n),
                TokenKind::Float(n) => ExprKind::Float(n),
                TokenKind::Str(s) => ExprKind::Str(s),
                TokenKind::Word("true") => ExprKind::Bool(true),
                TokenKind::Word("false") => ExprKind::Bool(false),
                TokenKind::Word("self") => ExprKind::SelfValue,
                _ => return Err(Error::new("SyntaxError", "expected expression", t.span)),
            };
            Expr { kind, span: t.span }
        };
        let mut compared = false;
        loop {
            if min <= 70 && self.eat("(") {
                let mut args = vec![];
                if !self.eat(")") {
                    loop {
                        args.push(self.argument()?);
                        if self.eat(")") {
                            break;
                        }
                        self.expect(",")?;
                        if self.eat(")") {
                            break;
                        }
                    }
                }
                lhs = Expr {
                    span: lhs.span.join(self.tokens[self.pos - 1].span),
                    kind: ExprKind::Call(Box::new(lhs), args),
                };
                continue;
            }
            if min <= 70 && self.eat(".") {
                let (name, end) = self.name()?;
                lhs = Expr {
                    span: lhs.span.join(end),
                    kind: ExprKind::Field(Box::new(lhs), name),
                };
                continue;
            }
            let (op, bp) = match &self.token().kind {
                TokenKind::Sym(op) => match *op {
                    "||" => (*op, 10),
                    "&&" => (*op, 20),
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => (*op, 30),
                    "+" | "-" => (*op, 40),
                    "*" | "/" | "%" => (*op, 50),
                    _ => break,
                },
                _ => break,
            };
            if bp < min {
                break;
            }
            if bp == 30 && compared {
                return self.fail("comparisons cannot be chained; use &&");
            }
            if bp == 30 {
                compared = true;
            }
            self.pos += 1;
            let rhs = self.expr(bp + 1)?;
            let span = lhs.span.join(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary(op.into(), Box::new(lhs), Box::new(rhs)),
                span,
            };
        }
        Ok(lhs)
    }
}

pub fn location(source: &str, position: usize) -> (usize, usize) {
    let position = source.floor_char_boundary(position.min(source.len()));
    let (line, start) = line_position(source, position);
    (
        line,
        expand_tabs(&source[start..position]).chars().count() + 1,
    )
}
