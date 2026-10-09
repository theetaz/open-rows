// SPDX-License-Identifier: Apache-2.0
//! SQL Server URL specifics: the `encrypt` and `trustServerCertificate`
//! options and Microsoft JDBC URLs,
//! `jdbc:sqlserver://host[:port][;property=value]…` (property names are
//! case-insensitive; `{…}` quotes a value holding `;`).

use openrows_secrets::SecretString;

use super::conn_url::{Draft, ParsedUrl, parse_port};
use super::{Driver, ServerParams, TlsMode, UrlError};

const JDBC_SCHEME: &str = "jdbc:sqlserver://";

/// SQL Server's `encrypt` and `trustServerCertificate` options.
#[derive(Default)]
pub(super) struct SqlServerTls {
    pub(super) encrypt: Option<String>,
    pub(super) trust: Option<String>,
}

impl SqlServerTls {
    /// The TLS mode the options ask for: no encryption request is
    /// login-only encryption (`allow`); a mandatory one verifies the
    /// certificate unless it is trusted (`require`). `None` without options.
    pub(super) fn mode(&self) -> Result<Option<TlsMode>, UrlError> {
        let trusted = match self
            .trust
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            None | Some("false" | "no") => false,
            Some("true" | "yes") => true,
            Some(_) => return Err(self.invalid()),
        };
        let encrypt = self.encrypt.as_deref().map(str::to_ascii_lowercase);
        Ok(match encrypt.as_deref() {
            Some("false" | "no" | "optional") => Some(TlsMode::Allow),
            Some("true" | "yes" | "mandatory" | "strict") | None if trusted => {
                Some(TlsMode::Require)
            }
            Some("true" | "yes" | "mandatory" | "strict") => Some(TlsMode::VerifyFull),
            None => None,
            Some(_) => return Err(self.invalid()),
        })
    }

    fn invalid(&self) -> UrlError {
        UrlError::InvalidSslMode {
            value: format!(
                "encrypt={}; trustServerCertificate={}",
                self.encrypt.as_deref().unwrap_or_default(),
                self.trust.as_deref().unwrap_or_default()
            ),
        }
    }
}

/// The text after `jdbc:sqlserver://` (scheme case-insensitive).
pub(super) fn strip_scheme(input: &str) -> Option<&str> {
    let head = input.get(..JDBC_SCHEME.len())?;
    head.eq_ignore_ascii_case(JDBC_SCHEME)
        .then(|| &input[JDBC_SCHEME.len()..])
}

/// Parse the server part and properties of a JDBC URL.
///
/// # Errors
/// A malformed port, a named instance without a port, or invalid TLS options.
pub(super) fn parse(rest: &str) -> Result<ParsedUrl, UrlError> {
    let mut parts = split_properties(rest).into_iter();
    let server = parts.next().unwrap_or_default();
    let mut draft = Draft {
        driver: Driver::Sqlserver,
        params: ServerParams::local(1433),
        tls_mode: None,
        sql_server: SqlServerTls::default(),
        password: None,
    };
    let mut instance = None;
    let mut port = None;
    apply_server(&server, (&mut draft, &mut instance, &mut port))?;
    for property in parts {
        let Some((key, value)) = property.split_once('=') else {
            continue;
        };
        let value = unbrace(value.trim());
        match key.trim().to_ascii_lowercase().as_str() {
            "servername" => draft.params.host = value,
            "portnumber" | "port" => port = Some(parse_port(value)?),
            "instancename" => instance = Some(value),
            "password" => draft.password = Some(SecretString::new(value)),
            "sslmode" => draft.apply("sslmode", value)?,
            other => {
                draft.apply(other, value)?;
            }
        }
    }
    match (port, instance) {
        (Some(port), _) => draft.params.port = port,
        (None, Some(instance)) => return Err(UrlError::NamedInstance { instance }),
        (None, None) => {}
    }
    draft.finish()
}

type ServerSlots<'a> = (&'a mut Draft, &'a mut Option<String>, &'a mut Option<u16>);

/// `host`, `host:port`, `host\instance` or `host\instance:port`.
fn apply_server(server: &str, (draft, instance, port): ServerSlots<'_>) -> Result<(), UrlError> {
    let (host, port_text) = match server.rsplit_once(':') {
        Some((host, text)) if !host.is_empty() => (host, Some(text)),
        _ => (server, None),
    };
    if let Some(text) = port_text {
        *port = Some(parse_port(text.to_owned())?);
    }
    let (host, named) = match host.split_once('\\') {
        Some((host, name)) => (host, Some(name.to_owned())),
        None => (host, None),
    };
    if !host.is_empty() {
        host.clone_into(&mut draft.params.host);
    }
    *instance = named.filter(|name| !name.is_empty());
    Ok(())
}

/// Split at `;` outside `{…}`.
fn split_properties(text: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut braced = false;
    for character in text.chars() {
        match character {
            '{' => braced = true,
            '}' => braced = false,
            ';' if !braced => {
                parts.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(last) = parts.last_mut() {
            last.push(character);
        }
    }
    parts
}

/// `{value}` without its braces.
fn unbrace(value: &str) -> String {
    value
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap_or(value)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_split_outside_braces() {
        assert_eq!(
            split_properties("h:1;password={a;b};user=u"),
            ["h:1", "password={a;b}", "user=u"]
        );
        assert_eq!(unbrace("{a;b}"), "a;b");
        assert_eq!(strip_scheme("JDBC:SQLServer://h"), Some("h"));
        assert_eq!(strip_scheme("sqlserver://h"), None);
    }

    #[test]
    fn tls_options_map_to_modes() -> Result<(), UrlError> {
        let mode = |encrypt: Option<&str>, trust: Option<&str>| {
            SqlServerTls {
                encrypt: encrypt.map(str::to_owned),
                trust: trust.map(str::to_owned),
            }
            .mode()
        };
        assert_eq!(mode(None, None)?, None);
        assert_eq!(mode(Some("false"), None)?, Some(TlsMode::Allow));
        assert_eq!(mode(Some("true"), None)?, Some(TlsMode::VerifyFull));
        assert_eq!(
            mode(Some("Mandatory"), Some("TRUE"))?,
            Some(TlsMode::Require)
        );
        assert_eq!(mode(None, Some("true"))?, Some(TlsMode::Require));
        assert!(mode(Some("maybe"), None).is_err());
        Ok(())
    }
}
