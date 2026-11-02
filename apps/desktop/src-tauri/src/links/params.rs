// SPDX-License-Identifier: Apache-2.0
//! Strict query-parameter reading: every key is known and given once (only
//! `filter` repeats), no key or value can smuggle a secret, and every value
//! is valid, control-free text within its length limit.
use std::collections::HashMap;

use url::Url;
use url::form_urlencoded;

use super::LinkError;

/// Longest identifier (connection, database, schema, object or column name).
const MAX_NAME_CHARS: usize = 256;
/// Longest free value (a path or a filter value).
const MAX_VALUE_CHARS: usize = 4096;
/// Key fragments that name credentials; such keys are refused outright.
const SECRET_KEYS: &[&str] = &[
    "pass",
    "pwd",
    "secret",
    "token",
    "key",
    "credential",
    "auth",
];

/// A link's parameters, consumed one by one; leftovers are unknown keys.
pub(super) struct Params {
    single: HashMap<String, String>,
    filters: Vec<String>,
}

impl Params {
    /// The query parameters of `url`.
    pub(super) fn read(url: &Url) -> Result<Self, LinkError> {
        Self::collect(url.query_pairs(), true)
    }

    /// The form-encoded parameters inside one `filter` value.
    pub(super) fn nested(text: &str) -> Result<Self, LinkError> {
        Self::collect(form_urlencoded::parse(text.as_bytes()), false)
    }

    fn collect<'a>(
        pairs: impl Iterator<Item = (std::borrow::Cow<'a, str>, std::borrow::Cow<'a, str>)>,
        filters: bool,
    ) -> Result<Self, LinkError> {
        let mut params = Self {
            single: HashMap::new(),
            filters: Vec::new(),
        };
        for (key, value) in pairs {
            check(&key, &value)?;
            if filters && key == "filter" {
                params.filters.push(value.into_owned());
            } else if params
                .single
                .insert(key.into_owned(), value.into_owned())
                .is_some()
            {
                return Err(LinkError::Duplicate);
            }
        }
        Ok(params)
    }

    /// A free value up to 4096 characters; `None` when absent.
    pub(super) fn value(&mut self, key: &'static str) -> Result<Option<String>, LinkError> {
        self.limited(key, MAX_VALUE_CHARS, true)
    }

    /// A non-empty single-line identifier; `None` when absent.
    pub(super) fn name(&mut self, key: &'static str) -> Result<Option<String>, LinkError> {
        let name = self.limited(key, MAX_NAME_CHARS, false)?;
        if name.as_deref().is_some_and(str::is_empty) {
            return Err(LinkError::Invalid(key));
        }
        Ok(name)
    }

    /// A required identifier.
    pub(super) fn required(&mut self, key: &'static str) -> Result<String, LinkError> {
        self.name(key)?.ok_or(LinkError::Missing(key))
    }

    /// Every `filter` value, in link order.
    pub(super) fn filters(&mut self) -> Vec<String> {
        std::mem::take(&mut self.filters)
    }

    /// Refuse parameters no reader asked for.
    pub(super) fn finish(self) -> Result<(), LinkError> {
        if self.single.is_empty() && self.filters.is_empty() {
            Ok(())
        } else {
            Err(LinkError::Unknown)
        }
    }

    fn limited(
        &mut self,
        key: &'static str,
        max: usize,
        multiline: bool,
    ) -> Result<Option<String>, LinkError> {
        let Some(value) = self.single.remove(key) else {
            return Ok(None);
        };
        let breaks = value.contains(['\n', '\r', '\t']);
        if value.chars().count() > max || (breaks && !multiline) {
            return Err(LinkError::Invalid(key));
        }
        Ok(Some(value))
    }
}

/// Reject credentials, undecodable bytes and control characters (tabs and
/// line breaks are left to each reader: only free values allow them).
fn check(key: &str, value: &str) -> Result<(), LinkError> {
    let lower = key.to_ascii_lowercase();
    if SECRET_KEYS.iter().any(|secret| lower.contains(secret)) || has_credentials(value) {
        return Err(LinkError::Secret);
    }
    let invalid = |text: &str| {
        text.chars()
            .any(|c| c == char::REPLACEMENT_CHARACTER || (c.is_control() && !"\n\r\t".contains(c)))
    };
    if invalid(key) || invalid(value) || key.contains(['\n', '\r', '\t']) {
        return Err(LinkError::Encoding);
    }
    Ok(())
}

/// Whether `value` holds a URL with a `user:password@` part.
fn has_credentials(value: &str) -> bool {
    value.match_indices("://").any(|(at, _)| {
        let rest = &value[at + 3..];
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        authority
            .rsplit_once('@')
            .is_some_and(|(user, _)| user.contains(':'))
    })
}
