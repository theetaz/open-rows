#![allow(clippy::unwrap_used, reason = "synthetic fixtures and test assertions")]
// SPDX-License-Identifier: Apache-2.0
//! Every control family round-trips through the actual postcard/framing contract.
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, Endpoint, FileSource, SslMode, TlsFiles,
};
use openrows_host_protocol::*;
use std::collections::BTreeMap;

fn config() -> OwnedConnect {
    OwnedConnect::try_from(&ConnectRequest {
        endpoint: Endpoint::File {
            source: FileSource::Memory {
                name: "test-only".into(),
            },
            attached: vec![],
        },
        database: "main".into(),
        user: String::new(),
        password: Some(openrows_net::SecretString::from("pw")),
        application_name: "test-only".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: vec!["select 1".into()],
        driver_options: BTreeMap::new(),
    })
    .unwrap()
}
fn cases() -> Vec<Control> {
    let key = FlowKey {
        request: 2,
        result: 1,
        batch: 1,
    };
    let grant = Grant {
        key,
        encoded: 64,
        decoded: 128,
        exclusive: false,
    };
    let auth = LaneHello {
        token: WireSecret::new("test-only".into()),
        lane: Lane::Control,
        generation: 1,
        parent_generation: 1,
    };
    vec![
        Control::Authenticate(auth),
        Control::Hello(Hello::sqlite(1)),
        Control::Ready(Hello::sqlite(1)),
        Control::Connect(config()),
        Control::Execute {
            sql: "select 1".into(),
        },
        Control::ExecuteBound {
            sql: "select ?".into(),
            params: vec![Some("test-only".into()), None],
        },
        Control::Ping,
        Control::Cancel { target: 2 },
        Control::CancelAck {
            target: 2,
            requested: true,
        },
        Control::Pause(key),
        Control::Paused { key, sequence: 3 },
        Control::Resume(grant),
        Control::Grant(grant),
        Control::Consumed(key),
        Control::Stop(key),
        Control::Close,
        Control::CloseAck,
        Control::Heartbeat,
        Control::Accepted,
        Control::Reply(Reply::Pong),
        Control::Reply(Reply::Executed(WireSummary::default())),
        Control::Reply(Reply::Connected(Connected {
            product: "SQLite".into(),
            version: "test-only".into(),
            flavour: Some("memory".into()),
            database: "main".into(),
            user: String::new(),
            tls: 2,
            trace: WireTrace {
                completed: vec![(2, 10)],
                failed: None,
                retried_tls: false,
                local_tls: true,
            },
            facets: Facets::default(),
            in_transaction: false,
        })),
        Control::ResultBegin,
        Control::ResultEnd { sequence: 2 },
        Control::BatchReady(grant),
        Control::Editor,
        Control::Admin,
        Control::Bulk,
    ]
}
#[test]
fn every_control_dto_round_trips() -> Result<(), ProtocolError> {
    let mut cases = cases();
    for kind in [
        CatalogKind::Databases,
        CatalogKind::Schemas,
        CatalogKind::Objects,
        CatalogKind::SearchIndex,
        CatalogKind::Columns,
        CatalogKind::Structure,
        CatalogKind::Detail,
        CatalogKind::Fingerprint,
        CatalogKind::Diagram,
        CatalogKind::Extensions,
        CatalogKind::Roles,
        CatalogKind::Grants,
        CatalogKind::ObjectDetail,
        CatalogKind::RoutineDefinition,
    ] {
        cases.push(Control::Catalog { kind, limit: 32 });
    }
    for error in [
        WireError::Server {
            code: "53200".into(),
            message: "test-only".into(),
        },
        WireError::Unsupported("test-only".into()),
        WireError::Decode {
            column: 2,
            reason: "test-only".into(),
        },
        WireError::Auth,
        WireError::Tls,
        WireError::Network,
        WireError::Protocol,
    ] {
        cases.push(Control::Error(error));
    }
    for control in cases {
        let frame = Frame::control(1, 2, &control)?;
        let encoded = frame.encode(Lane::Control)?;
        let decoded = FrameDecoder::new(Lane::Control).push(&encoded)?;
        assert_eq!(decoded[0].decode_control()?, control);
    }
    Ok(())
}
#[test]
fn owned_connect_preserves_config_and_redacts_secrets() {
    let owned = config();
    let native = owned.to_request().unwrap();
    let round_trip = OwnedConnect::try_from(&native).unwrap();
    assert_eq!(owned, round_trip);
    assert!(!format!("{owned:?}").contains("pw"));
    assert_eq!(native.password.unwrap().expose(), "pw");
}
#[test]
fn routed_connect_is_unsupported() {
    let native =
        config()
            .to_request()
            .unwrap()
            .through(std::sync::Arc::new(openrows_net::TcpDialer::new(
                "test-only".into(),
                1,
            )));
    assert!(matches!(
        OwnedConnect::try_from(&native),
        Err(openrows_driver_api::DriverError::Unsupported(_))
    ));
}
#[test]
fn incompatible_versions_builds_and_capabilities_fail_before_connect() {
    let expected = Hello::sqlite(1);
    for field in 0..8 {
        let mut other = expected.clone();
        match field {
            0 => other.version.0 += 1,
            1 => other.version.1 += 1,
            2 => other.build.push('x'),
            3 => other.schema.push('x'),
            4 => other.generation += 1,
            5 => other.capabilities.features.clear(),
            6 => other.limits[0] += 1,
            _ => other.required = 0,
        }
        assert_eq!(other.validate(&expected), Err(ProtocolError::Incompatible));
    }
}
#[test]
fn wrong_missing_and_replayed_secret_refused_on_both_lanes() {
    let bootstrap = Bootstrap {
        control: "c".into(),
        data: "d".into(),
        directory: "test-only".into(),
        token: WireSecret::new("test-only".into()),
        parent_generation: 1,
        generation: 2,
    };
    for lane in [Lane::Control, Lane::Data] {
        let mut authentication = Authentication::new(bootstrap.clone());
        let mut hello = LaneHello {
            token: WireSecret::new("wrong".into()),
            lane,
            parent_generation: 1,
            generation: 2,
        };
        assert_eq!(
            authentication.accept(&hello, lane),
            Err(ProtocolError::Authentication)
        );
        hello.token = WireSecret::default();
        assert_eq!(
            authentication.accept(&hello, lane),
            Err(ProtocolError::Authentication)
        );
        hello.token = bootstrap.token.clone();
        assert_eq!(authentication.accept(&hello, lane), Ok(()));
        assert_eq!(
            authentication.accept(&hello, lane),
            Err(ProtocolError::Authentication)
        );
    }
}
