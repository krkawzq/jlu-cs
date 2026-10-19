use crate::syntax::Span;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct SourceUnit {
    pub name: String,
    pub start: usize,
    pub text: Rc<String>,
}
impl SourceUnit {
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.start + self.text.len()
    }
    pub fn local_span(&self, span: Span) -> Span {
        Span {
            start: span.start.saturating_sub(self.start),
            end: span.end.saturating_sub(self.start),
        }
    }
    pub fn segment(&self, span: Span) -> Option<&str> {
        let local = self.local_span(span);
        self.text.get(local.start..local.end)
    }
}
