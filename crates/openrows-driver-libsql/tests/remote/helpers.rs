// SPDX-License-Identifier: Apache-2.0
//! Authenticated Hrana over cloud and explicitly trusted custom TCP helpers.
use super::support::{Collect, request};
use openrows_driver_api::{ConnectTrace, Session};
use openrows_driver_libsql::connect;
use openrows_net::helper::{HelperDialer, HelperHop, HelperSpec, Helpers};
use openrows_store::profile::{
    CommandPurpose, CommandTrust, EnvKind, EnvTag, hop_command, parse_url, trusted_hop_spec,
};
use std::{path::PathBuf, sync::Arc};
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn cloud_and_trusted_custom_hops_execute_authenticated_hrana() -> TestResult {
    let mut request = request()?;
    let fake = PathBuf::from(std::env::var("OPENROWS_FAKE_HELPER")?);
    let port = std::env::var("OPENROWS_TEST_LIBSQL_PORT")?.parse()?;
    let directory = tempfile::tempdir_in(".")?;
    let trust = CommandTrust::new(&directory.path().join("trust.json"));
    let pool = Helpers::default();
    for hop in hops(&fake, port) {
        let mut spec = spec(hop, &fake, port, &trust)?;
        spec.environment
            .push(("OPENROWS_FAKE_TARGET".into(), format!("127.0.0.1:{port}")));
        let helper = pool.acquire("libsql-test", spec).await?;
        request.via = Some(Arc::new(HelperDialer::new(Arc::clone(&helper))));
        let mut session = connect(&request, &mut ConnectTrace::default()).await?;
        let mut sink = Collect::default();
        session.execute("SELECT 1", &mut sink).await?;
        assert_eq!(sink.value(0, 0), Some("1".into()));
        Box::new(session).close().await?;
        request.via = None;
        helper.close().await;
    }
    pool.shutdown().await;
    Ok(())
}

fn hops(fake: &std::path::Path, port: u16) -> [HelperHop; 2] {
    [
        HelperHop::Kubectl {
            context: "test".into(),
            namespace: "default".into(),
            resource: "svc/db".into(),
            remote_port: port,
            kubeconfig: None,
        },
        HelperHop::Custom {
            program: fake.to_string_lossy().into_owned(),
            args: vec![
                "--port".into(),
                "{local_port}".into(),
                "--tcp-target".into(),
                "{remote_host}:{remote_port}".into(),
            ],
            ready_timeout_seconds: None,
        },
    ]
}

fn spec(
    hop: HelperHop,
    fake: &std::path::Path,
    port: u16,
    trust: &CommandTrust,
) -> Result<HelperSpec, Box<dyn std::error::Error>> {
    if !matches!(hop, HelperHop::Custom { .. }) {
        return Ok(HelperSpec::new(hop, fake.into(), "fake".into()));
    }
    let (mut profile, _) = parse_url(&format!("http://127.0.0.1:{port}"))?
        .into_profile("test", EnvTag::new(EnvKind::Local));
    profile.helper = Some(hop);
    assert!(trusted_hop_spec(&profile, trust).is_err());
    trust.trust_for(
        profile.id,
        CommandPurpose::Hop,
        &hop_command(&profile).ok_or("missing hop")?,
    )?;
    trusted_hop_spec(&profile, trust)?.ok_or_else(|| "missing trusted hop".into())
}
