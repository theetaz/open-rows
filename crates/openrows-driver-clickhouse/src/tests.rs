// SPDX-License-Identifier: Apache-2.0
//! Request credentials, escaped parameters and typed error boundaries.
use crate::{error, request};
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, DriverError, Endpoint, SslMode, TlsFiles,
};
use openrows_net::SecretString;
fn config() -> ConnectRequest {
    ConnectRequest {
        endpoint: Endpoint::Server {
            host: "localhost".to_owned(),
            port: 8123,
        },
        database: "default".to_owned(),
        user: "default".to_owned(),
        password: Some(SecretString::from("test-only")),
        application_name: "OpenRows".to_owned(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
    }
}
#[test]
fn credentials_stay_in_sensitive_headers() -> Result<(), DriverError> {
    let request = request::build(
        &config(),
        "SELECT {p1:String}",
        &[Some("'\\\n".to_owned())],
        ("id", Some("session")),
    )?;
    assert!(!request.uri().to_string().contains("test-only"));
    assert!(!format!("{request:?}").contains("test-only"));
    assert!(request.headers()["x-clickhouse-key"].is_sensitive());
    assert_eq!(request.headers()["x-clickhouse-key"], "test-only");
    let query: Vec<_> =
        url::form_urlencoded::parse(request.uri().query().unwrap_or_default().as_bytes()).collect();
    assert!(
        query
            .iter()
            .any(|(key, value)| key == "param_p1" && value == "'\\\\\\n")
    );
    Ok(())
}
#[test]
fn server_errors_map_and_redact_passwords() {
    for (number, code) in [
        (62, "42601"),
        (60, "42P01"),
        (47, "42703"),
        (164, "25006"),
        (394, "57014"),
        (236, "57014"),
        (349, "23502"),
        (70, "22018"),
        (373, "55P03"),
        (999, "HY000"),
    ] {
        let body = format!("Code: {number}. DB::Exception: test-only");
        let error = error::server(None, &body, &config());
        assert!(matches!(&error, DriverError::Server { code: found, .. } if found == code));
        assert!(!error.to_string().contains("test-only"));
    }
    assert!(matches!(
        error::server(Some("516"), "rejected test-only", &config()),
        DriverError::Auth(_)
    ));
}
#[test]
fn invalid_credential_headers_never_echo_the_value() {
    let mut config = config();
    config.password = Some(SecretString::from("test-only\n"));
    let result = request::build(&config, "SELECT 1", &[], ("id", None));
    let error = result.expect_err("header must be refused");
    assert!(!error.to_string().contains("test-only"));
}

#[test]
fn uuid_projection_keeps_nested_types_and_quoted_names() -> Result<(), DriverError> {
    let types = vec![("a`b".to_owned(), "Array(Nullable(UUID))".to_owned())];
    assert_eq!(
        crate::query::arrow_sql("SELECT a", &types)?,
        "SELECT CAST(`a\\`b`, 'Array(Nullable(String))') AS `a\\`b` FROM (SELECT a) FORMAT ArrowStream"
    );
    Ok(())
}
