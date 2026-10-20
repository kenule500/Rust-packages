//! Diagnostics: stable codes, source spans, and rendered context.

/// A byte range in the source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
}

impl Span {
    /// Build a span.
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Compilation cannot continue.
    Error,
    /// Compilation continues, but the result is suspect.
    Warning,
    /// Purely informational.
    Note,
}

/// One diagnostic, with a stable code and an optional span.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Stable code such as `E1001`; part of the public contract.
    pub code: &'static str,
    /// Short message, one line.
    pub message: String,
    /// Severity.
    pub severity: Severity,
    /// Source location, if known.
    pub span: Option<Span>,
    /// Optional long-form explanation.
    pub help: Option<String>,
}

/// Render diagnostics against the source text, with line numbers and a caret
/// underline. This is the only formatting function in the crate; consumers
/// that want a different presentation read the fields directly.
pub fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    for d in diagnostics {
        out.push_str(&format!("{} {}: {}\n", severity_label(d.severity), d.code, d.message));
        if let Some(span) = d.span {
            if let Some((line_no, line)) = line_at(source, span.start) {
                out.push_str(&format!("  {line_no:>4} | {line}\n"));
                let col = span.start.saturating_sub(byte_offset_of_line(source, span.start));
                out.push_str(&format!("       | {}^\n", " ".repeat(col)));
            }
        }
        if let Some(help) = &d.help {
            out.push_str(&format!("       = help: {help}\n"));
        }
    }
    out
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

fn byte_offset_of_line(source: &str, at: usize) -> usize {
    source[..at.min(source.len())].rfind('\n').map_or(0, |i| i + 1)
}

fn line_at(source: &str, at: usize) -> Option<(usize, &str)> {
    let at = at.min(source.len());
    let start = byte_offset_of_line(source, at);
    let end = source[start..].find('\n').map_or(source.len(), |i| start + i);
    let number = source[..start].matches('\n').count() + 1;
    Some((number, &source[start..end]))
}
