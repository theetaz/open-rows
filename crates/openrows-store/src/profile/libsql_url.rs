// SPDX-License-Identifier: Apache-2.0
//! Remote endpoints contain no credentials, query strings or database names.
use super::{Driver, ParsedUrl, ProfileError, ServerParams, TlsMode, UrlError};
use url::{Host, Url};

pub(super) fn parse(input: &str) -> Result<ParsedUrl, UrlError> {
    let url = Url::parse(input)?;
    let scheme = url.scheme();
    if !matches!(scheme, "libsql" | "https" | "http")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(UrlError::InvalidLibsqlUrl);
    }
    let host = match url.host().ok_or(UrlError::InvalidLibsqlUrl)? {
        Host::Domain(host) => host.to_owned(),
        Host::Ipv4(host) => host.to_string(),
        Host::Ipv6(host) => host.to_string(),
    };
    validate_host(&host).map_err(|_| UrlError::InvalidLibsqlUrl)?;
    let params = ServerParams {
        host,
        port: url
            .port()
            .unwrap_or(if scheme == "http" { 80 } else { 443 }),
        ..ServerParams::default()
    };
    if params.port == 0 {
        return Err(UrlError::InvalidLibsqlUrl);
    }
    Ok(ParsedUrl {
        driver: Driver::Libsql,
        params,
        file: None,
        password: None,
        tls_mode: if scheme == "http" {
            TlsMode::Disable
        } else {
            TlsMode::VerifyFull
        },
    })
}

pub(super) fn validate_host(host: &str) -> Result<(), ProfileError> {
    if host.is_empty()
        || host.contains(['/', '@', '?', '#', '%', '\\'])
        || host.chars().any(char::is_whitespace)
        || (host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err())
    {
        return Err(ProfileError::Invalid {
            field: "params.host",
            reason: "use a host only; put the auth token in its secret field",
        });
    }
    Ok(())
}
