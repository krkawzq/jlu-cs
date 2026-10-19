use crate::{Config, Error, Runtime, Value, ir::Program, syntax};

/// Incremental top-level evaluation; previous instructions are never replayed.
pub struct Session {
    pub runtime: Runtime,
    source_bytes: usize,
    pub max_source_bytes: usize,
}
impl Session {
    pub fn new(config: Config) -> Self {
        Self {
            runtime: Runtime::new(Program::default(), config),
            source_bytes: 0,
            max_source_bytes: 16 * 1024 * 1024,
        }
    }
    pub fn evaluate(&mut self, name: &str, source: &str) -> syntax::Result<Value> {
        if self.source_bytes.saturating_add(source.len() + 1) > self.max_source_bytes {
            return Err(Error::new(
                "SourceLimit",
                "session source limit reached; use :reset",
                syntax::Span::default(),
            ));
        }
        let start = self.source_bytes;
        self.runtime.program.add_source(name, source, start);
        self.source_bytes += source.len() + 1;
        let mut tokens = syntax::lex(source).map_err(|mut e| {
            e.span.start += start;
            e.span.end += start;
            e
        })?;
        for token in &mut tokens {
            token.span.start += start;
            token.span.end += start;
        }
        let body = syntax::parse(tokens)?;
        self.runtime.evaluate_fragment(&body)
    }
    pub fn error_source<'a>(&'a self, error: &Error) -> (&'a str, &'a str, Error) {
        if error.kind == "SourceLimit" {
            return ("<session>", "", error.clone());
        }
        if let Some(file) = self.runtime.program.source_at(error.span.start) {
            let mut local = error.clone();
            local.span = file.local_span(error.span);
            local.trace.clear();
            (&file.name, &file.text, local)
        } else {
            ("<session>", "", error.clone())
        }
    }
    pub fn render_error(&self, error: &Error) -> String {
        let (name, source, local) = self.error_source(error);
        let mut text = local.render(name, source);
        for &span in error.trace.iter().rev().take(12) {
            let e = Error::new("Call", "", span);
            let (name, source, local) = self.error_source(&e);
            let (line, column) = syntax::location(source, local.span.start);
            text.push_str(&format!("  called at {name}:{line}:{column}\n"));
        }
        text
    }
    pub fn reset(&mut self) {
        let config = self.runtime.config.clone();
        let limit = self.max_source_bytes;
        *self = Self::new(config);
        self.max_source_bytes = limit;
    }
}

pub fn needs_more_input(source: &str) -> bool {
    match syntax::lex(source) {
        Err(e) => e.kind == "UnterminatedComment",
        Ok(tokens) => match syntax::parse(tokens) {
            Err(e) => e.kind == "SyntaxError" && e.span.start >= source.len(),
            Ok(_) => false,
        },
    }
}
pub fn read_console_line() -> std::io::Result<Option<String>> {
    crate::terminal::Terminal::new().read_line()
}
