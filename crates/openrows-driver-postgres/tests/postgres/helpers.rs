// SPDX-License-Identifier: Apache-2.0
//! Real `PostgreSQL` through every cloud hop using the isolated fake executable.
use super::support::{TestResult, config, single_batch};
use arrow_array::Int32Array;
use openrows_driver_postgres::{ConnectOptions, PgConfig, PgSession, SslMode, TlsFiles};
use openrows_net::helper::{CloudFlavor, HelperDialer, HelperHop, HelperSpec, Helpers};
use std::{path::PathBuf, sync::Arc};

fn hops() -> [HelperHop; 5] {
    [
        HelperHop::Ssm {
            target: "i-test".into(),
            remote_host: Some("db.internal".into()),
            remote_port: 5432,
            profile: None,
            region: None,
        },
        HelperHop::Kubectl {
            context: "test".into(),
            namespace: "default".into(),
            resource: "svc/db".into(),
            remote_port: 5432,
            kubeconfig: None,
        },
        HelperHop::Cloud {
            instance: "project:region:instance".into(),
            flavor: CloudFlavor::Cloudsql,
            iam_auth: true,
            private_ip: true,
            credentials_file: None,
        },
        HelperHop::Cloud {
            instance: "projects/test/locations/test/clusters/test/instances/test".into(),
            flavor: CloudFlavor::Alloydb,
            iam_auth: false,
            private_ip: true,
            credentials_file: None,
        },
        HelperHop::Cloudflared {
            hostname: "db.example.invalid".into(),
        },
    ]
}
#[tokio::test]
#[ignore = "needs PostgreSQL; run just test-pg"]
async fn all_cloud_hops_select_one_with_original_tls_hostname() -> TestResult {
    let config = PgConfig {
        host: "localhost".into(),
        ..config()?
    };
    let fake = PathBuf::from(std::env::var("OPENROWS_FAKE_HELPER")?);
    let pool = Helpers::default();
    for hop in hops() {
        let mut spec = HelperSpec::new(hop, fake.clone(), "fake 1.0.0".into());
        spec.environment.push((
            "OPENROWS_FAKE_TARGET".into(),
            format!("127.0.0.1:{}", config.port),
        ));
        let helper = pool.acquire("test", spec).await?;
        let options = ConnectOptions {
            ssl_mode: SslMode::VerifyFull,
            tls: TlsFiles {
                ca_file: Some(
                    PathBuf::from(std::env::var("OPENROWS_TEST_PG_TLS_DIR")?).join("ca.crt"),
                ),
                ..TlsFiles::default()
            },
            via: Some(Arc::new(HelperDialer::new(Arc::clone(&helper)))),
            ..ConnectOptions::default()
        };
        let mut session = PgSession::connect_with(&config, &options).await?;
        let batch = single_batch(&mut session, "SELECT 1 AS n").await?;
        assert_eq!(
            batch
                .column(0)
                .as_any()
                .downcast_ref::<Int32Array>()
                .ok_or("not int32")?
                .value(0),
            1
        );
        drop(session);
        drop(options);
        helper.close().await;
    }
    pool.shutdown().await;
    Ok(())
}
