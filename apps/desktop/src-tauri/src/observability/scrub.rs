// SPDX-License-Identifier: Apache-2.0
//! The redaction layer in front of the log file (docs/12 §2, docs/14 §6).
//!
//! Call sites log typed kinds and identifiers, never secrets or query text
//! (see `observability/mod.rs`). This pass is the second line of defence: it
//! removes userinfo passwords from URLs, sensitive query parameters, JSON or
//! `key=value` fields whose names look secret, and bearer tokens, from every
//! line before it is written.
use std::sync::LazyLock;

use regex::Regex;

/// Field names that hold secrets (matched as substrings, case-insensitively).
const SECRET_KEYS: &str = "password|passwd|passphrase|pwd|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|credentials?";

struct Rules {
    userinfo: Regex,
    query: Regex,
    json: Regex,
    pair: Regex,
    bearer: Regex,
}

static RULES: LazyLock<Rules> = LazyLock::new(|| Rules {
    // scheme://user:password@host
    userinfo: compile(r"(?i)([a-z][a-z0-9+.\-]*://[^\s/?#&;\x22'@:]*):[^\s/?#\x22']*@"),
    // ?password=… or &sslkey=… in a URL or form body
    query: compile(&format!(
        r"(?i)([?&;][a-z0-9_.\-]*(?:{SECRET_KEYS}|key|sig|signature)=)[^&\s\x22';]*"
    )),
    // "password": "…", also inside an escaped JSON string (\"password\":\"…\")
    json: compile(&format!(
        r#"(?i)(\\?"[a-z0-9_.\-]*(?:{SECRET_KEYS})[a-z0-9_.\-]*\\?"\s*:\s*)(\\?")(?:[^"\\]|\\[^"])*\\?""#
    )),
    // password=…, token: … in free text
    pair: compile(&format!(
        r#"(?i)\b([a-z0-9_.\-]*(?:{SECRET_KEYS})[a-z0-9_.\-]*[ \t]*[=:][ \t]*)[^\s,;&"'\\}}\])]+"#
    )),
    bearer: compile(r"(?i)\b(bearer[ \t]+)[a-z0-9._~+/=\-]+"),
});

/// Compile one of the constant patterns above; they are covered by the tests.
#[expect(
    clippy::expect_used,
    reason = "constant patterns, checked by the unit tests"
)]
fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("constant redaction pattern")
}

/// `text` with every secret-looking value replaced by [`REDACTED`].
pub(crate) fn scrub(text: &str) -> String {
    let rules = &*RULES;
    let text = rules.userinfo.replace_all(text, "${1}:[redacted]@");
    let text = rules.query.replace_all(&text, "${1}[redacted]");
    let text = rules.json.replace_all(&text, "${1}${2}[redacted]${2}");
    let text = rules.pair.replace_all(&text, "${1}[redacted]");
    rules
        .bearer
        .replace_all(&text, "${1}[redacted]")
        .into_owned()
}

/// The first `max_chars` characters of `text` (never splits a character).
pub(crate) fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping_counts_characters_not_bytes() {
        assert_eq!(clip("héllo wörld", 5), "héllo");
        assert_eq!(clip("short", 50), "short");
    }

    #[test]
    fn url_passwords_are_removed_and_the_rest_of_the_url_kept() {
        let line =
            scrub("connect postgres://app:hunter2-pass@db.internal:5432/orders?sslmode=require");
        assert!(!line.contains("hunter2-pass"), "{line}");
        assert!(line.contains("postgres://app:[redacted]@db.internal:5432/orders?sslmode=require"));
        assert_eq!(
            scrub("postgres://app@db.internal/orders"),
            "postgres://app@db.internal/orders"
        );
    }

    #[test]
    fn sensitive_query_parameters_are_removed() {
        let line = scrub("url=https://x.test/a?password=hunter2-pass&sslmode=require&sig=abc123");
        assert!(
            !line.contains("hunter2-pass") && !line.contains("abc123"),
            "{line}"
        );
        assert!(line.contains("&sslmode=require"));
    }

    #[test]
    fn json_and_key_value_secrets_are_removed_even_when_escaped() {
        let json = scrub(r#"{"password":"hunter2-pass","db_password":"x y","user":"app"}"#);
        assert!(
            !json.contains("hunter2-pass") && !json.contains("x y"),
            "{json}"
        );
        assert!(json.contains(r#""user":"app""#), "{json}");
        let escaped = scrub(r#"{"message":"bad \"password\":\"hunter2-pass\" given"}"#);
        assert!(!escaped.contains("hunter2-pass"), "{escaped}");
        let pair = scrub("password=hunter2-pass user=app");
        assert_eq!(pair, "password=[redacted] user=app");
        assert!(!scrub("Authorization: Bearer abc.def-ghi").contains("abc.def-ghi"));
    }

    #[test]
    fn ordinary_text_passes_through_unchanged() {
        let text = "session opened in 12 ms, 3 rows";
        assert_eq!(scrub(text), text);
    }
}
