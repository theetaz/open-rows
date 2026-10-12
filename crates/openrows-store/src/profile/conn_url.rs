// SPDX-License-Identifier: Apache-2.0
//! Connection URL parsing (FR-CONN-02) for every engine.
//!
//! - `postgres://` and `postgresql://` follow libpq's URI form:
//!   `postgresql://[user[:password]@][host][:port][/dbname][?param=value&...]`.
//! - `mysql://` and `mariadb://` take the same shape (default port 3306);
//!   `ssl-mode` accepts the `MySQL` names (`DISABLED`, `PREFERRED`,
//!   `REQUIRED`, `VERIFY_CA`, `VERIFY_IDENTITY`) as well as libpq `sslmode`.
//! - `sqlserver://` and `mssql://` take the same shape (default port 1433);
//!   `encrypt` (`true`/`false`, `mandatory`/`optional`/`strict`) and
//!   `trustServerCertificate` map onto the TLS modes when `sslmode` is
//!   absent, and `databaseName` names the database. `jdbc:sqlserver://`
//!   URLs are parsed too (see `sqlserver_url.rs`).
//! - `sqlite:` names an absolute file path (`sqlite:/data/app.db` or
//!   `sqlite:///data/app.db`); `?mode=ro` opens it read-only.
//!   `sqlite::memory:` (also `sqlite://:memory:`) is an in-memory database.
//!
//! Components are percent-decoded (`+` stays a plus, as in libpq).
//! Passwords, from userinfo or `?password=`, come back as a [`SecretString`]
//! and never reach [`ServerParams`]. Malformed input yields a [`UrlError`];
//! the parser never panics.

use std::path::PathBuf;

use openrows_secrets::SecretString;
use percent_encoding::percent_decode_str;
use url::{Host, Url};

use super::sqlserver_url::SqlServerTls;
use super::{ConnectionProfile, Driver, EnvTag, FileParams, ServerParams, TlsMode, UrlError};

/// A parsed URL: engine, profile parameters and the password, kept apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    /// The engine the scheme names.
    pub driver: Driver,
    /// Server parameters with defaults applied (`localhost`, default port);
    /// defaults only for file engines.
    pub params: ServerParams,
    /// The database file of a `sqlite:` URL.
    pub file: Option<FileParams>,
    /// `sslmode`, or the host-based default when absent (`disable` for files).
    pub tls_mode: TlsMode,
    /// Password from the URL, if any.
    pub password: Option<SecretString>,
}

impl ParsedUrl {
    /// Build a new profile from the parsed parameters; the caller hands the
    /// returned password to [`super::ProfileService::create`].
    #[must_use]
    pub fn into_profile(
        self,
        name: &str,
        env_tag: EnvTag,
    ) -> (ConnectionProfile, Option<SecretString>) {
        let mut profile = match self.file {
            Some(file) => ConnectionProfile::new_file(name, file, env_tag),
            None => ConnectionProfile::new(name, self.driver, self.params, env_tag),
        };
        profile.driver = self.driver;
        profile.tls.mode = self.tls_mode;
        (profile, self.password)
    }
}

/// Parse a `postgres://`, `postgresql://`, `mysql://`, `mariadb://`,
/// `sqlserver://`, `mssql://`, `jdbc:sqlserver://` or `sqlite:` URL.
///
/// # Errors
/// A [`UrlError`] describing the first problem; it never contains the
/// password or the raw URL.
pub fn parse_url(input: &str) -> Result<ParsedUrl, UrlError> {
    let input = input.trim();
    if input.eq_ignore_ascii_case("duckdb::memory:")
        || input.eq_ignore_ascii_case("duckdb://:memory:")
    {
        let mut parsed = sqlite_url(FileParams::memory());
        parsed.driver = Driver::Duckdb;
        return Ok(parsed);
    }
    if input.eq_ignore_ascii_case("libsql-file::memory:") {
        let mut parsed = sqlite_url(FileParams::memory());
        parsed.driver = Driver::LibsqlFile;
        return Ok(parsed);
    }
    if is_sqlite_memory(input) {
        return Ok(sqlite_url(FileParams::memory()));
    }
    if let Some(rest) = super::sqlserver_url::strip_scheme(input) {
        return super::sqlserver_url::parse(rest);
    }
    if input.split_once(':').is_some_and(|(scheme, _)| {
        ["libsql", "https", "http"]
            .iter()
            .any(|value| scheme.eq_ignore_ascii_case(value))
    }) {
        return super::libsql_url::parse(input);
    }
    reject_multiple_hosts(input)?;
    let url = Url::parse(input)?;
    let driver = match url.scheme() {
        "postgres" | "postgresql" => Driver::Postgres,
        "mysql" => Driver::Mysql,
        "mongodb" | "mongodb+srv" => Driver::Mongodb,
        "mariadb" => Driver::Mariadb,
        "sqlserver" | "mssql" => Driver::Sqlserver,
        "clickhouse" | "clickhouse+https" => Driver::Clickhouse,
        "sqlite" => return parse_sqlite(&url),
        "duckdb" => {
            let mut parsed = parse_sqlite(&url)?;
            parsed.driver = Driver::Duckdb;
            return Ok(parsed);
        }
        "libsql-file" => {
            let mut parsed = parse_sqlite(&url)?;
            parsed.driver = Driver::LibsqlFile;
            return Ok(parsed);
        }
        scheme => {
            return Err(UrlError::UnsupportedScheme {
                scheme: scheme.to_owned(),
            });
        }
    };
    let mut draft = Draft {
        driver,
        params: base_params(&url, driver)?,
        tls_mode: match url.scheme() {
            "clickhouse" => Some(TlsMode::Disable),
            "clickhouse+https" => Some(TlsMode::VerifyFull),
            _ => None,
        },
        sql_server: SqlServerTls::default(),
        password: decode_optional(url.password(), "password")?.map(SecretString::new),
    };
    if url.scheme() == "clickhouse+https" && url.port().is_none() {
        draft.params.port = 8443;
    }
    if url.scheme() == "mongodb+srv" {
        if url.port().is_some() {
            return Err(UrlError::MultipleHosts);
        }
        draft
            .params
            .options
            .insert("mongodb_srv".into(), "true".into());
        draft.tls_mode = Some(TlsMode::VerifyFull);
    }
    for (key, value) in query_pairs(url.query().unwrap_or_default())? {
        draft.apply(&key, value)?;
    }
    draft.finish()
}

/// `sqlite:/abs/path` or `sqlite:///abs/path`, optionally `?mode=ro`.
fn parse_sqlite(url: &Url) -> Result<ParsedUrl, UrlError> {
    let path = PathBuf::from(decode(url.path(), "path")?);
    if !path.is_absolute()
        || url
            .host()
            .is_some_and(|host| host != Host::Domain("localhost"))
    {
        return Err(UrlError::InvalidFilePath);
    }
    let pairs = query_pairs(url.query().unwrap_or_default())?;
    let read_only = pairs
        .iter()
        .any(|(key, value)| key == "mode" && value == "ro");
    Ok(sqlite_url(FileParams {
        path,
        read_only,
        ..FileParams::default()
    }))
}

/// `sqlite::memory:` (or `sqlite://:memory:`), scheme case-insensitive.
fn is_sqlite_memory(input: &str) -> bool {
    input
        .split_once(':')
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("sqlite"))
        .is_some_and(|(_, rest)| rest.strip_prefix("//").unwrap_or(rest) == ":memory:")
}

fn sqlite_url(file: FileParams) -> ParsedUrl {
    ParsedUrl {
        driver: Driver::Sqlite,
        params: ServerParams::default(),
        file: Some(file),
        tls_mode: TlsMode::Disable,
        password: None,
    }
}

pub(super) struct Draft {
    pub(super) driver: Driver,
    pub(super) params: ServerParams,
    pub(super) tls_mode: Option<TlsMode>,
    pub(super) sql_server: SqlServerTls,
    pub(super) password: Option<SecretString>,
}

impl Draft {
    /// SQL Server option names (case-insensitive); `true` when handled.
    fn apply_sql_server(&mut self, key: &str, value: &str) -> bool {
        match key.to_ascii_lowercase().as_str() {
            "encrypt" => self.sql_server.encrypt = Some(value.to_owned()),
            "trustservercertificate" => self.sql_server.trust = Some(value.to_owned()),
            "databasename" | "database" => self.params.database = Some(value.to_owned()),
            "applicationname" => self.params.application_name = Some(value.to_owned()),
            "user" | "username" => self.params.user = Some(value.to_owned()),
            _ => return false,
        }
        true
    }

    pub(super) fn apply(&mut self, key: &str, value: String) -> Result<(), UrlError> {
        if self.driver == Driver::Sqlserver && self.apply_sql_server(key, &value) {
            return Ok(());
        }
        if self.driver == Driver::Mongodb && key == "tls" {
            self.tls_mode = Some(match value.as_str() {
                "true" => TlsMode::VerifyFull,
                "false" => TlsMode::Disable,
                _ => return Err(UrlError::InvalidSslMode { value }),
            });
            return Ok(());
        }
        match key {
            "password" => self.password = Some(SecretString::new(value)),
            "sslmode" | "ssl-mode" | "ssl_mode" => {
                self.tls_mode = Some(parse_ssl_mode(value)?);
            }
            "host" => self.params.host = value,
            "port" => self.params.port = parse_port(value)?,
            "user" => self.params.user = Some(value),
            "dbname" => self.params.database = Some(value),
            "application_name" => self.params.application_name = Some(value),
            _ => {
                self.params.options.insert(key.to_owned(), value);
            }
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<ParsedUrl, UrlError> {
        let tls_mode = match self.tls_mode {
            Some(mode) => mode,
            None => self
                .sql_server
                .mode()?
                .unwrap_or_else(|| TlsMode::default_for_host(&self.params.host)),
        };
        Ok(ParsedUrl {
            driver: self.driver,
            file: None,
            params: self.params,
            tls_mode,
            password: self.password,
        })
    }
}

/// libpq multi-host lists (`h1,h2`) would otherwise surface as a confusing
/// port error. Only the host part after the last `@` is inspected, because a
/// password may legitimately contain commas.
fn reject_multiple_hosts(input: &str) -> Result<(), UrlError> {
    let Some((_, rest)) = input.split_once("://") else {
        return Ok(());
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_part = authority.rsplit('@').next().unwrap_or_default();
    if host_part.contains(',') {
        return Err(UrlError::MultipleHosts);
    }
    Ok(())
}

fn base_params(url: &Url, driver: Driver) -> Result<ServerParams, UrlError> {
    let mut params = ServerParams::local(driver.default_port().unwrap_or(5432));
    if let Some(host) = host_text(url)? {
        params.host = host;
    }
    if let Some(port) = url.port() {
        params.port = check_port(port, &port.to_string())?;
    }
    params.user = decode_optional(Some(url.username()), "user")?;
    let path = url.path().strip_prefix('/').unwrap_or(url.path());
    params.database = decode_optional(Some(path), "database")?;
    Ok(params)
}

fn host_text(url: &Url) -> Result<Option<String>, UrlError> {
    match url.host() {
        None => Ok(None),
        Some(Host::Domain(domain)) => decode_optional(Some(domain), "host"),
        Some(Host::Ipv4(address)) => Ok(Some(address.to_string())),
        Some(Host::Ipv6(address)) => Ok(Some(address.to_string())),
    }
}

/// Percent-decode `raw`; empty or absent components become `None`.
fn decode_optional(raw: Option<&str>, field: &'static str) -> Result<Option<String>, UrlError> {
    match raw {
        None | Some("") => Ok(None),
        Some(text) => decode(text, field).map(Some),
    }
}

fn decode(text: &str, field: &'static str) -> Result<String, UrlError> {
    percent_decode_str(text)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(|_| UrlError::InvalidEncoding { field })
}

fn query_pairs(query: &str) -> Result<Vec<(String, String)>, UrlError> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Ok((decode(key, "query key")?, decode(value, "query value")?))
        })
        .collect()
}

pub(super) fn parse_port(value: String) -> Result<u16, UrlError> {
    match value.parse::<u16>() {
        Ok(port) => check_port(port, &value),
        Err(_) => Err(UrlError::InvalidPort { value }),
    }
}

fn check_port(port: u16, text: &str) -> Result<u16, UrlError> {
    if port == 0 {
        return Err(UrlError::InvalidPort {
            value: text.to_owned(),
        });
    }
    Ok(port)
}

/// libpq `sslmode` or `MySQL` `ssl-mode` (case-insensitive).
fn parse_ssl_mode(value: String) -> Result<TlsMode, UrlError> {
    let mode = match value.to_ascii_uppercase().as_str() {
        "DISABLED" => Some(TlsMode::Disable),
        "PREFERRED" => Some(TlsMode::Prefer),
        "REQUIRED" => Some(TlsMode::Require),
        "VERIFY_CA" => Some(TlsMode::VerifyCa),
        "VERIFY_IDENTITY" => Some(TlsMode::VerifyFull),
        _ => TlsMode::parse(&value.to_ascii_lowercase()),
    };
    mode.ok_or(UrlError::InvalidSslMode { value })
}

#[cfg(test)]
mod clickhouse_tests {
    use super::*;
    #[test]
    fn clickhouse_urls_select_http_or_https_without_exposing_passwords() -> Result<(), UrlError> {
        let plain = parse_url("clickhouse://default:pw@localhost/db")?;
        assert_eq!(plain.driver, Driver::Clickhouse);
        assert_eq!(plain.params.port, 8123);
        assert_eq!(plain.tls_mode, TlsMode::Disable);
        assert_eq!(plain.params.database.as_deref(), Some("db"));
        assert!(plain.password.is_some());
        let secure = parse_url("clickhouse+https://default@host/db")?;
        assert_eq!(secure.params.port, 8443);
        assert_eq!(secure.tls_mode, TlsMode::VerifyFull);
        let custom = parse_url("clickhouse+https://host:9440/db?sslmode=require")?;
        assert_eq!(custom.params.port, 9440);
        assert_eq!(custom.tls_mode, TlsMode::Require);
        Ok(())
    }
}

#[cfg(test)]
mod mongodb_tests {
    use super::*;
    #[test]
    fn mongodb_and_srv_urls_keep_passwords_out_of_options() -> Result<(), UrlError> {
        let plain =
            parse_url("mongodb://fixture:synthetic@localhost/test?tls=false&authSource=admin")?;
        assert_eq!(plain.driver, Driver::Mongodb);
        assert_eq!(plain.params.port, 27017);
        assert_eq!(plain.tls_mode, TlsMode::Disable);
        assert_eq!(
            plain.params.options.get("authSource").map(String::as_str),
            Some("admin")
        );
        assert!(plain.password.is_some());
        assert!(!format!("{:?}", plain.params).contains("synthetic"));
        let srv = parse_url("mongodb+srv://fixture@cluster.example/test")?;
        assert_eq!(
            srv.params.options.get("mongodb_srv").map(String::as_str),
            Some("true")
        );
        assert_eq!(srv.tls_mode, TlsMode::VerifyFull);
        assert!(parse_url("mongodb+srv://cluster.example:27017/test").is_err());
        Ok(())
    }
}
