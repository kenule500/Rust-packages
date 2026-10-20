//! Indentation-aware emitter for generated code.
//!
//! Deliberately not a template engine: generators call typed methods, so
//! escaping, whitespace, and newline behaviour stay under compiler control.

/// A line-oriented writer that tracks indentation.
#[derive(Debug)]
pub struct Writer {
    out: String,
    depth: usize,
    unit: &'static str,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    /// A writer using four spaces per level.
    pub fn new() -> Self {
        Self { out: String::new(), depth: 0, unit: "    " }
    }

    /// A writer using `unit` per level (use `"\t"` for tab-indented targets).
    pub fn with_unit(unit: &'static str) -> Self {
        Self { out: String::new(), depth: 0, unit }
    }

    /// Write one indented line. Empty input emits a blank line.
    pub fn line(&mut self, text: &str) -> &mut Self {
        if text.is_empty() {
            self.out.push('\n');
        } else {
            for _ in 0..self.depth {
                self.out.push_str(self.unit);
            }
            self.out.push_str(text);
            self.out.push('\n');
        }
        self
    }

    /// Write a line and increase indentation.
    pub fn open(&mut self, text: &str) -> &mut Self {
        self.line(text);
        self.depth += 1;
        self
    }

    /// Decrease indentation and write a closing line.
    pub fn close(&mut self, text: &str) -> &mut Self {
        self.depth = self.depth.saturating_sub(1);
        self.line(text);
        self
    }

    /// Blank line.
    pub fn blank(&mut self) -> &mut Self {
        self.out.push('\n');
        self
    }

    /// Current indent depth.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Consume the writer, returning the generated text.
    pub fn finish(self) -> String {
        self.out
    }
}
