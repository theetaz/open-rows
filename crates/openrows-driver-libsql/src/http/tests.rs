// SPDX-License-Identifier: Apache-2.0
//! Token isolation, sensitive headers and request serialization golden tests.
use super::*;
use crate::protocol::{Request, Statement};
use openrows_driver_api::{ConnectRequest, ConnectTimeouts, Endpoint, SslMode, TlsFiles};
fn request(token: &str) -> ConnectRequest {
    ConnectRequest {
        endpoint: Endpoint::Server {
            host: "db.test".into(),
            port: 443,
        },
        database: "main".into(),
        user: String::new(),
        password: Some(SecretString::from(token)),
        application_name: "test".into(),
        ssl_mode: SslMode::VerifyFull,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: Vec::new(),
    }
}
#[tokio::test]
async fn token_is_only_in_sensitive_authorization_header() -> Result<(), DriverError> {
    let request = request("test-only");
    assert!(!format!("{request:?}").contains("test-only"));
    let client = Http {
        transport: Transport::new(&request)?,
        token: request.password,
        initial: None,
    };
    let requests = [Request::Execute {
        stmt: Statement::text("SELECT 1", &[]),
    }];
    let pipeline = Pipeline {
        baton: None,
        requests: &requests,
    };
    let outgoing = client.build(&pipeline)?;
    assert_eq!(outgoing.uri().path(), "/v3/pipeline");
    assert_eq!(outgoing.headers()[AUTHORIZATION], "Bearer test-only");
    assert!(outgoing.headers()[AUTHORIZATION].is_sensitive());
    assert!(!format!("{:?}", outgoing.headers()).contains("test-only"));
    let body = outgoing
        .into_body()
        .collect()
        .await
        .map_err(|_| error::protocol())?
        .to_bytes();
    assert!(!String::from_utf8_lossy(&body).contains("test-only"));
    assert!(!client.transport.url.as_str().contains("test-only"));
    Ok(())
}
#[test]
fn execute_request_matches_golden_without_interpolation() -> Result<(), serde_json::Error> {
    let params = [Some("x'); DROP TABLE t;--".into()), None];
    let requests = [
        Request::Execute {
            stmt: Statement::text("SELECT ?1, ?2", &params),
        },
        Request::GetAutocommit,
    ];
    let actual = serde_json::to_value(Pipeline {
        baton: None,
        requests: &requests,
    })?;
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/golden/execute.json"))?;
    assert_eq!(actual, expected);
    Ok(())
}
#[test]
fn rejects_credential_redirects_and_header_injection_without_echo() -> Result<(), DriverError> {
    let mut transport = Transport::new(&request("x"))?;
    for next in [
        "https://other.test/",
        "https://user:test-only@db.test/",
        "http://db.test/",
        "https://db.test/?token=test-only",
    ] {
        let error = transport.redirect(next).err().ok_or_else(error::protocol)?;
        assert!(!format!("{error:?} {error}").contains("test-only"));
    }
    transport.redirect("https://db.test/sticky/")?;
    let request = request("test-only\r\nX: x");
    let client = Http {
        transport: Transport::new(&request)?,
        token: request.password,
        initial: None,
    };
    let error = client
        .build(&Pipeline {
            baton: None,
            requests: &[],
        })
        .err()
        .ok_or_else(error::protocol)?;
    assert!(!error.to_string().contains("test-only"));
    Ok(())
}

#[test]
fn response_shape_and_stream_continuity_are_validated() -> Result<(), serde_json::Error> {
    let requests = [
        Request::Execute {
            stmt: Statement::text("SELECT 1", &[]),
        },
        Request::GetAutocommit,
    ];
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/golden/response.json"))?;
    let reply: Reply = serde_json::from_value(golden.clone())?;
    assert!(reply.validate(&requests).is_ok());
    let mut missing_baton = golden.clone();
    missing_baton["baton"] = serde_json::Value::Null;
    assert!(
        serde_json::from_value::<Reply>(missing_baton)?
            .validate(&requests)
            .is_err()
    );
    let mut wrong_type = golden;
    wrong_type["results"][1]["response"]["type"] = "close".into();
    assert!(
        serde_json::from_value::<Reply>(wrong_type)?
            .validate(&requests)
            .is_err()
    );
    assert!(reply.validate(&requests[..1]).is_err());
    Ok(())
}
