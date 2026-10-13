// SPDX-License-Identifier: Apache-2.0
use super::*;
use openrows_driver_api::{ConnectTimeouts, Endpoint, SslMode, TlsFiles};
use std::collections::BTreeMap;

fn request() -> ConnectRequest {
    ConnectRequest {
        endpoint: Endpoint::Server {
            host: "localhost".into(),
            port: 27017,
        },
        database: "profile_database".into(),
        user: "fixture".into(),
        password: None,
        application_name: "test".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
        driver_options: BTreeMap::new(),
    }
}

#[test]
fn authentication_defaults_preserve_profile_database() -> Result<(), DriverError> {
    let mut request = request();
    assert_eq!(
        credential(&request, None)?.and_then(|c| c.source),
        Some("profile_database".into())
    );
    request.driver_options.insert(
        "mongodb_default_auth_source".into(),
        "profile_database".into(),
    );
    request.database = "browsed_database".into();
    assert_eq!(
        credential(&request, None)?.and_then(|c| c.source),
        Some("profile_database".into())
    );
    request.driver_options.clear();
    request.database.clear();
    assert_eq!(
        credential(&request, None)?.and_then(|c| c.source),
        Some("admin".into())
    );
    Ok(())
}

#[test]
fn dns_source_yields_to_explicit_authentication_database() -> Result<(), DriverError> {
    let mut request = request();
    request.driver_options.insert(
        "mongodb_default_auth_source".into(),
        "profile_database".into(),
    );
    let discovered = Credential::builder()
        .source("dns_database".to_owned())
        .build();
    assert_eq!(
        credential(&request, Some(&discovered))?.and_then(|c| c.source),
        Some("dns_database".into())
    );
    request
        .driver_options
        .insert("authSource".into(), "explicit_database".into());
    assert_eq!(
        credential(&request, Some(&discovered))?.and_then(|c| c.source),
        Some("explicit_database".into())
    );
    Ok(())
}

#[test]
fn password_without_a_user_is_rejected() {
    let mut request = request();
    request.user.clear();
    assert!(credential(&request, None).is_ok_and(|c| c.is_none()));
    request.password = Some(openrows_net::SecretString::from("synthetic"));
    assert!(matches!(
        credential(&request, None),
        Err(DriverError::Auth(_))
    ));
}
