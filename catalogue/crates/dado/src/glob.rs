//! Glob compilation and matching.
//!
//! Supports `*` (any run except `/`), `**` (any run including `/`),
//! `?` (one character except `/`), and literal text. Kept dependency-free so
//! it can be used in build scripts and WASM targets.

/// A compiled glob pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    raw: String,
}

impl Pattern {
    /// Compile a pattern. Compilation cannot fail today; the signature leaves
    /// room for a future error type without breaking callers.
    pub fn new(pattern: &str) -> Self {
        Self { raw: pattern.to_owned() }
    }

    /// The original pattern text.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Match a path against the pattern.
    pub fn is_match(&self, path: &str) -> bool {
        matches_from(self.raw.as_bytes(), path.as_bytes())
    }
}

fn matches_from(pattern: &[u8], text: &[u8]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    match pattern[0] {
        b'*' if pattern.len() > 1 && pattern[1] == b'*' => {
            let rest = &pattern[2..];
            let rest = rest.strip_prefix(b"/").unwrap_or(rest);
            for i in 0..=text.len() {
                if matches_from(rest, &text[i..]) {
                    return true;
                }
            }
            false
        }
        b'*' => {
            let rest = &pattern[1..];
            for i in 0..=text.len() {
                if matches_from(rest, &text[i..]) {
                    return true;
                }
                if text.get(i) == Some(&b'/') {
                    break;
                }
            }
            false
        }
        b'?' => match text.first() {
            Some(&c) if c != b'/' => matches_from(&pattern[1..], &text[1..]),
            _ => false,
        },
        c => match text.first() {
            Some(&t) if t == c => matches_from(&pattern[1..], &text[1..]),
            _ => false,
        },
    }
}
