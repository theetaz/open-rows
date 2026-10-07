// SPDX-License-Identifier: Apache-2.0
//! Zeroizing secret text that never appears in `Debug` output or logs.

use std::fmt;

use zeroize::Zeroizing;

/// A password, passphrase or one-time code, wiped from memory on drop.
///
/// There is deliberately no `Display` implementation, and `Debug` prints a
/// fixed placeholder, so a secret cannot leak through formatting or tracing.
#[derive(Clone, Default)]
pub struct SecretString(Zeroizing<String>);

impl SecretString {
    /// Take ownership of `value`; the buffer is zeroized when dropped.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    /// Borrow the secret for the moment it is handed to the SSH library.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString([redacted])")
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::SecretString;

    #[test]
    fn debug_output_is_redacted() {
        let secret = SecretString::from("hunter2-synthetic");
        let rendered = format!("{secret:?} {:?}", Some(secret.clone()));
        assert!(!rendered.contains("hunter2"));
        assert!(rendered.contains("[redacted]"));
        assert_eq!(secret.expose(), "hunter2-synthetic");
    }
}
