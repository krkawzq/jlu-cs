use super::{Runtime, Value};
use crate::syntax::{
    self, Arg, Binding, Body, Error, Expr, ExprKind, Result, Span, Stmt, StmtKind, TokenKind,
};
use std::{collections::HashSet, rc::Rc};
const MAX_NODES: usize = 8192;
const MAX_SOURCE: usize = 1024 * 1024;
fn text(s: impl Into<String>) -> Value {
    Value::Str(Rc::new(s.into()))
}
fn number(n: usize) -> Value {
    Value::Int(n as i64)
}
fn parse_error(e: Error, call: Span) -> Error {
    Error::new(
        "AstError",
        format!(
            "{} at source bytes {}..{}: {}",
            e.kind, e.span.start, e.span.end, e.message
        ),
        call,
    )
}
fn check_size(source: &str, call: Span) -> Result<()> {
    if source.len() > MAX_SOURCE {
        Err(Error::new("ResourceLimit", "AST input exceeds 1 MiB", call))
    } else {
        Ok(())
    }
}
struct Tree<'a> {
    runtime: &'a mut Runtime,
    source: &'a str,
    count: usize,
    call: Span,
}
impl Tree<'_> {
    fn node(
        &mut self,
        kind: &str,
        span: Span,
        attributes: Vec<(&str, Value)>,
        children: Vec<Value>,
    ) -> Result<Value> {
        self.count += 1;
        if self.count > MAX_NODES {
            return Err(Error::new(
                "ResourceLimit",
                "AST exceeds 8192 nodes",
                self.call,
            ));
        }
        let attributes = self.runtime.snapshot(attributes);
        let children = self.runtime.snapshot_array(children);
        let span = self.runtime.ast_span(self.source, span);
        Ok(self.runtime.snapshot(vec![
            ("kind", text(kind)),
            ("span", span),
            ("attributes", attributes),
            ("children", children),
        ]))
    }
    fn body(&mut self, body: &Body, kind: &str) -> Result<Value> {
        let mut children = vec![];
        for s in &body.statements {
            children.push(self.statement(s)?);
        }
        if let Some(e) = &body.tail {
            let child = self.expr(e)?;
            children.push(self.node("Tail", e.span, vec![], vec![child])?);
        }
        self.node(kind, body.span, vec![], children)
    }
    fn binding(&mut self, b: &Binding, kind: &str) -> Result<Value> {
        self.node(
            kind,
            b.span,
            vec![
                ("name", text(b.name.clone())),
                ("by_ref", Value::Bool(b.by_ref)),
            ],
            vec![],
        )
    }
    fn argument(&mut self, a: &Arg) -> Result<Value> {
        let child = self.expr(&a.expr)?;
        self.node(
            "Argument",
            a.expr.span,
            vec![("by_ref", Value::Bool(a.by_ref))],
            vec![child],
        )
    }
    fn statement(&mut self, s: &Stmt) -> Result<Value> {
        let (kind, attrs, children) = match &s.kind {
            StmtKind::Let(name, a) => (
                "Let",
                vec![("name", text(name.clone()))],
                vec![self.argument(a)?],
            ),
            StmtKind::Assign(target, value) => (
                "Assign",
                vec![],
                vec![self.expr(target)?, self.expr(value)?],
            ),
            StmtKind::Expr(e) => ("ExprStmt", vec![], vec![self.expr(e)?]),
            StmtKind::Return(value) | StmtKind::Break(value) => {
                let mut children = vec![];
                if let Some(e) = value {
                    children.push(self.expr(e)?);
                }
                (
                    if matches!(s.kind, StmtKind::Return(_)) {
                        "Return"
                    } else {
                        "Break"
                    },
                    vec![("has_value", Value::Bool(value.is_some()))],
                    children,
                )
            }
            StmtKind::Continue => ("Continue", vec![], vec![]),
        };
        self.node(kind, s.span, attrs, children)
    }
    fn expr(&mut self, e: &Expr) -> Result<Value> {
        let (kind, attrs, children) = match &e.kind {
            ExprKind::Int(raw) => ("Integer", vec![("raw", text(raw.clone()))], vec![]),
            ExprKind::Float(raw) => ("Float", vec![("raw", text(raw.clone()))], vec![]),
            ExprKind::Str(s) => ("String", vec![("value", text(s.clone()))], vec![]),
            ExprKind::Bool(b) => ("Boolean", vec![("value", Value::Bool(*b))], vec![]),
            ExprKind::Unit => ("Unit", vec![], vec![]),
            ExprKind::Name(n) => ("Name", vec![("name", text(n.clone()))], vec![]),
            ExprKind::SelfValue => ("Self", vec![], vec![]),
            ExprKind::Group(e) => ("Group", vec![], vec![self.expr(e)?]),
            ExprKind::Unary(op, a) => (
                "Unary",
                vec![("operator", text(op.clone()))],
                vec![self.expr(a)?],
            ),
            ExprKind::Binary(op, a, b) => (
                "Binary",
                vec![("operator", text(op.clone()))],
                vec![self.expr(a)?, self.expr(b)?],
            ),
            ExprKind::Call(f, args) => {
                let mut children = vec![self.expr(f)?];
                for a in args {
                    children.push(self.argument(a)?);
                }
                ("Call", vec![("arity", number(args.len()))], children)
            }
            ExprKind::Field(receiver, name) => (
                "Field",
                vec![("name", text(name.clone()))],
                vec![self.expr(receiver)?],
            ),
            ExprKind::Block(b) => return self.body(b, "Block"),
            ExprKind::If(condition, body, other) => {
                let mut children = vec![self.expr(condition)?, self.body(body, "Block")?];
                if let Some(other) = other {
                    children.push(self.expr(other)?);
                }
                (
                    "If",
                    vec![("has_else", Value::Bool(other.is_some()))],
                    children,
                )
            }
            ExprKind::Loop(b) => ("Loop", vec![], vec![self.body(b, "Block")?]),
            ExprKind::Data(statements) => {
                let mut children = vec![];
                for s in statements {
                    children.push(self.statement(s)?);
                }
                ("Data", vec![], children)
            }
            ExprKind::Closure(captures, parameters, body) => {
                let mut children = vec![];
                for b in captures {
                    children.push(self.binding(b, "Capture")?);
                }
                for b in parameters {
                    children.push(self.binding(b, "Parameter")?);
                }
                children.push(self.body(body, "Block")?);
                (
                    "Closure",
                    vec![
                        ("capture_count", number(captures.len())),
                        ("arity", number(parameters.len())),
                    ],
                    children,
                )
            }
            ExprKind::Decorate(name, target) => (
                "Decorate",
                vec![("name", text(name.clone()))],
                vec![self.expr(target)?],
            ),
        };
        self.node(kind, e.span, attrs, children)
    }
}
impl Runtime {
    fn ast_span(&mut self, source: &str, span: Span) -> Value {
        let (line, column) = syntax::location(source, span.start);
        let (end_line, end_column) = syntax::location(source, span.end);
        self.snapshot(vec![
            ("start", number(span.start)),
            ("end", number(span.end)),
            ("line", number(line)),
            ("column", number(column)),
            ("end_line", number(end_line)),
            ("end_column", number(end_column)),
        ])
    }
    pub(super) fn ast_parse(&mut self, source: &str, call: Span) -> Result<Value> {
        check_size(source, call)?;
        let tokens = syntax::lex(source).map_err(|e| parse_error(e, call))?;
        let body = syntax::parse(tokens).map_err(|e| parse_error(e, call))?;
        Tree {
            runtime: self,
            source,
            count: 0,
            call,
        }
        .body(&body, "Program")
    }
    pub(super) fn ast_tokens(&mut self, source: &str, call: Span) -> Result<Value> {
        check_size(source, call)?;
        let tokens = syntax::lex(source).map_err(|e| parse_error(e, call))?;
        if tokens.len() > MAX_NODES {
            return Err(Error::new(
                "ResourceLimit",
                "token inspection exceeds 8192 tokens",
                call,
            ));
        }
        let mut result = vec![];
        for token in tokens {
            let (kind, value) = match token.kind {
                TokenKind::Ident(s) => ("Identifier", text(s)),
                TokenKind::Int(s) => ("Integer", text(s)),
                TokenKind::Float(s) => ("Float", text(s)),
                TokenKind::Str(s) => ("String", text(s)),
                TokenKind::Word(s) => ("Keyword", text(s)),
                TokenKind::Sym(s) => ("Symbol", text(s)),
                TokenKind::Eof => ("Eof", Value::Unit),
            };
            let span = self.ast_span(source, token.span);
            let raw = source.get(token.span.start..token.span.end).unwrap_or("");
            result.push(self.snapshot(vec![
                ("kind", text(kind)),
                ("text", text(raw)),
                ("value", value),
                ("span", span),
            ]));
        }
        Ok(self.snapshot_array(result))
    }
    pub(super) fn ast_node_shape(&self, value: &Value, call: Span) -> bool {
        matches!(self.snapshot_field(value, "kind", call), Ok(Value::Str(_)))
            && matches!(
                self.snapshot_field(value, "span", call),
                Ok(Value::Closure(_))
            )
            && matches!(
                self.snapshot_field(value, "attributes", call),
                Ok(Value::Closure(_))
            )
            && self
                .snapshot_field(value, "children", call)
                .and_then(|v| self.snapshot_items(&v, call))
                .is_ok()
    }
    pub(super) fn ast_nodes(&self, root: &Value, call: Span) -> Result<Vec<Value>> {
        let mut stack = vec![root.clone()];
        let mut visited = HashSet::new();
        let mut result = vec![];
        while let Some(node) = stack.pop() {
            let Value::Closure(id) = node else {
                return Err(Error::new(
                    "AstError",
                    "AST nodes must be data records",
                    call,
                ));
            };
            if !self.ast_node_shape(&node, call) {
                return Err(Error::new("AstError", "invalid AST node shape", call));
            }
            if !visited.insert(id) {
                return Err(Error::new(
                    "AstError",
                    "AST contains a cycle or a repeated node",
                    call,
                ));
            }
            if visited.len() > MAX_NODES {
                return Err(Error::new(
                    "ResourceLimit",
                    "AST traversal exceeds 8192 nodes",
                    call,
                ));
            }
            let children = self.snapshot_field(&node, "children", call)?;
            let children = self.snapshot_items(&children, call)?;
            stack.extend(children.into_iter().rev());
            result.push(node);
        }
        Ok(result)
    }
    pub(super) fn ast_dump(&self, root: &Value, call: Span) -> Result<String> {
        self.ast_nodes(root, call)?;
        let mut stack = vec![(root.clone(), 0usize)];
        let mut output = String::new();
        while let Some((node, depth)) = stack.pop() {
            let kind = self.snapshot_field(&node, "kind", call)?;
            let span = self.snapshot_field(&node, "span", call)?;
            let start = self.snapshot_field(&span, "start", call)?;
            let end = self.snapshot_field(&span, "end", call)?;
            output.push_str(&format!(
                "{}{} [{}..{}]",
                "  ".repeat(depth),
                kind.display(),
                start.display(),
                end.display()
            ));
            let attrs = self.snapshot_field(&node, "attributes", call)?;
            if let Value::Closure(id) = attrs {
                for (name, slot) in &self.closure(id).fields {
                    let v = &self.slot(*slot).value;
                    let display = if let Value::Str(s) = v {
                        format!("{s:?}")
                    } else {
                        v.display()
                    };
                    output.push_str(&format!(" {name}={display}"));
                }
            }
            output.push('\n');
            if output.len() > 16 * 1024 * 1024 {
                return Err(Error::new("ResourceLimit", "AST dump exceeds 16 MiB", call));
            }
            let children =
                self.snapshot_items(&self.snapshot_field(&node, "children", call)?, call)?;
            stack.extend(children.into_iter().rev().map(|c| (c, depth + 1)));
        }
        Ok(output)
    }
}
