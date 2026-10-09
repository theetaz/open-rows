// SPDX-License-Identifier: Apache-2.0
//! The log level: `OPENROWS_LOG` wins, then `diagnostics.logLevel` in
//! `settings.json`, then `info` (docs/14 §6).
use tracing::level_filters::LevelFilter;

/// Environment variable that overrides the settings file, for development.
pub(crate) const ENV_VAR: &str = "OPENROWS_LOG";

/// The level used when nothing else is configured.
pub(crate) const DEFAULT: LevelFilter = LevelFilter::INFO;

/// A level name as written in the environment or the settings file.
pub(crate) fn parse(text: &str) -> Option<LevelFilter> {
    match text.trim().to_ascii_lowercase().as_str() {
        "off" => Some(LevelFilter::OFF),
        "error" => Some(LevelFilter::ERROR),
        "warn" => Some(LevelFilter::WARN),
        "info" => Some(LevelFilter::INFO),
        "debug" => Some(LevelFilter::DEBUG),
        "trace" => Some(LevelFilter::TRACE),
        _ => None,
    }
}

/// The `diagnostics.logLevel` of a `settings.json` text, when it is valid.
pub(crate) fn from_settings(text: &str) -> Option<LevelFilter> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    parse(value.get("diagnostics")?.get("logLevel")?.as_str()?)
}

/// The effective level: a valid environment value, else a valid settings
/// value, else [`DEFAULT`]. Invalid values are ignored, never fatal.
pub(crate) fn resolve(env: Option<&str>, settings: Option<&str>) -> LevelFilter {
    env.and_then(parse)
        .or_else(|| settings.and_then(from_settings))
        .unwrap_or(DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse_case_insensitively_and_reject_others() {
        assert_eq!(parse(" Debug "), Some(LevelFilter::DEBUG));
        assert_eq!(parse("OFF"), Some(LevelFilter::OFF));
        assert_eq!(parse("verbose"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn environment_wins_over_settings_which_win_over_the_default() {
        let settings = r#"{"openrows":1,"diagnostics":{"logLevel":"warn"}}"#;
        assert_eq!(resolve(Some("trace"), Some(settings)), LevelFilter::TRACE);
        assert_eq!(resolve(None, Some(settings)), LevelFilter::WARN);
        assert_eq!(resolve(Some("nope"), Some(settings)), LevelFilter::WARN);
        assert_eq!(resolve(None, None), DEFAULT);
        assert_eq!(resolve(None, Some("not json")), DEFAULT);
        assert_eq!(
            resolve(None, Some(r#"{"diagnostics":{"logLevel":3}}"#)),
            DEFAULT
        );
    }
}
