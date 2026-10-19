#![allow(dead_code)]

// Compile the actual interpreter frontend without linking its runtime or LLVM.
#[path = "../../prim-rs/src/syntax.rs"]
mod syntax;
#[path = "../../prim-rs/src/ir.rs"]
mod ir;
#[path = "../../prim-rs/src/liveness.rs"]
mod liveness;

use std::io::{self, Read};
use syntax::{Arg, Body, Expr, ExprKind, StmtKind};

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn list(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(","))
}
fn node(kind: &str, values: impl IntoIterator<Item = String>) -> String {
    list(std::iter::once(quote(kind)).chain(values))
}
fn arg(a: &Arg) -> String {
    node(if a.by_ref { "ref" } else { "value" }, [expr(&a.expr)])
}
fn body(b: &Body) -> String {
    let statements = b.statements.iter().map(|s| match &s.kind {
        StmtKind::Let(n, v) => node("let", [quote(n), arg(v)]),
        StmtKind::Assign(a, b) => node("assign", [expr(a), expr(b)]),
        StmtKind::Expr(e) => node("expression", [expr(e)]),
        StmtKind::Return(e) => node("return", [e.as_ref().map_or("null".into(), expr)]),
        StmtKind::Break(e) => node("break", [e.as_ref().map_or("null".into(), expr)]),
        StmtKind::Continue => node("continue", []),
    });
    node("body", [list(statements), b.tail.as_ref().map_or("null".into(), |e| expr(e))])
}
fn expr(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Int(n) => node("int", [quote(n)]),
        ExprKind::Float(n) => node("float", [quote(n)]),
        ExprKind::Str(_) => node("str", []),
        ExprKind::Bool(v) => node("bool", [v.to_string()]),
        ExprKind::Unit => node("unit", []),
        ExprKind::Name(n) => node("name", [quote(n)]),
        ExprKind::SelfValue => node("self", []),
        ExprKind::Group(e) => node("group", [expr(e)]),
        ExprKind::Unary(op, e) => node("unary", [quote(op), expr(e)]),
        ExprKind::Binary(op, a, b) => node("binary", [quote(op), expr(a), expr(b)]),
        ExprKind::Call(e, args) => node("call", [expr(e), list(args.iter().map(arg))]),
        ExprKind::Field(e, n) => node("field", [expr(e), quote(n)]),
        ExprKind::Block(b) => node("block", [body(b)]),
        ExprKind::If(c, b, other) => node("if", [expr(c), body(b), other.as_ref().map_or("null".into(), |e| expr(e))]),
        ExprKind::Loop(b) => node("loop", [body(b)]),
        ExprKind::Data(statements) => node("data", [body(&Body { statements: statements.clone(), tail: None, span: e.span })]),
        ExprKind::Closure(captures, params, b) => {
            let bindings = |bs: &[syntax::Binding]| list(bs.iter().map(|b| list([quote(&b.name), b.by_ref.to_string()])));
            node("closure", [bindings(captures), bindings(params), body(b)])
        }
        ExprKind::Decorate(n, e) => node("decorate", [quote(n), expr(e)]),
    }
}
fn failure(phase: &str, e: syntax::Error, tree: &str) {
    println!("{{\"ok\":false,\"phase\":{},\"code\":{},\"message\":{},\"start\":{},\"end\":{},\"tree\":{}}}",
        quote(phase), quote(&e.kind), quote(&e.message), e.span.start, e.span.end, tree);
}
fn main() {
    let mut input = io::stdin().lock();
    loop {
        let mut size = [0; 4];
        match input.read_exact(&mut size) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            result => result.unwrap(),
        }
        let mut bytes = vec![0; u32::from_le_bytes(size) as usize];
        input.read_exact(&mut bytes).unwrap();
        let source = String::from_utf8(bytes).unwrap();
        let tokens = match syntax::lex(&source) { Ok(t) => t, Err(e) => { failure("lex", e, "null"); continue; } };
        let ast = match syntax::parse(tokens) { Ok(b) => b, Err(e) => { failure("parse", e, "null"); continue; } };
        let tree = body(&ast);
        match ir::compile(&ast) {
            Ok(_) => println!("{{\"ok\":true,\"tree\":{tree}}}"),
            Err(e) => failure("resolve", e, &tree),
        }
    }
}
