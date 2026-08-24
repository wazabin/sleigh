use sleigh::Span;

/// A single source edit: replace the bytes at `span` with `replacement`.
///
/// A zero-length span (`start == end`) is a pure insertion.
#[derive(Debug, Clone)]
pub struct Edit {
    /// The bytes to replace, in a *physical* source file — not in the
    /// preprocessed stream.
    pub span: Span,
    /// What to put there.
    pub replacement: String,
}

impl Edit {
    /// Creates an edit replacing `span` with `replacement`.
    pub fn new(span: Span, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }
}
