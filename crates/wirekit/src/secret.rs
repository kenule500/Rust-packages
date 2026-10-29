//! A credential wrapper that cannot leak through formatting.
//!
//! Scope: hold a secret, render `***` from `Debug` and `Display`, and require
//! an explicit accessor to read it.
//!
//! Non-goals: zeroizing memory on drop, encryption at rest, and `Serialize`.
//! A secret field must not be serializable (CON-214); that is enforced by this
//! type simply not implementing `serde::Serialize`.

use std::fmt;

/// A credential that renders as `***` in logs, traces, and error output.
///
/// ```
/// # use wirekit::Secret;
/// let token = Secret::new(String::from("super-secret"));
/// assert_eq!(format!("{token}"), "***");
/// assert_eq!(format!("{token:?}"), "***");
/// assert_eq!(token.expose(), "super-secret");
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wrap a value.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Borrow the wrapped value.
    ///
    /// Call this only at the point where the credential is applied to a
    /// request or handed to a signature routine.
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Consume the wrapper and return the value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("***")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("***")
    }
}

impl<T: AsRef<str>> AsRef<str> for Secret<T> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
