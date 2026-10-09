// SPDX-License-Identifier: Apache-2.0
//! Real `MySQL`/`MariaDB` through a fake kubectl process, retaining the logical host.
use super::support::{Collect, TestResult, request};
use openrows_driver_api::{ConnectTrace, Endpoint, Session as _, SslMode};
use openrows_driver_mysql::connect;
use openrows_net::helper::{HelperDialer, HelperHop, HelperSpec, Helpers};
use std::{path::PathBuf, sync::Arc};

#[tokio::test]
#[ignore = "needs MySQL/MariaDB; run just test-mysql"]
async fn kubectl_helper_select_one() -> TestResult {
    let mut request = request(SslMode::Disable)?;
    let Endpoint::Server { host, port } = &request.endpoint else {
        return Err("not a server".into());
    };
    let hop = HelperHop::Kubectl {
        context: "test".into(),
        namespace: "default".into(),
        resource: "svc/db".into(),
        remote_port: 3306,
        kubeconfig: None,
    };
    let mut spec = HelperSpec::new(
        hop,
        PathBuf::from(std::env::var("OPENROWS_FAKE_HELPER")?),
        "fake 1.0.0".into(),
    );
    spec.environment
        .push(("OPENROWS_FAKE_TARGET".into(), format!("{host}:{port}")));
    let pool = Helpers::default();
    let helper = pool.acquire("test", spec).await?;
    request.via = Some(Arc::new(HelperDialer::new(Arc::clone(&helper))));
    let mut session = connect(&request, &mut ConnectTrace::default()).await?;
    let mut sink = Collect::default();
    session.execute("SELECT 1 AS n", &mut sink).await?;
    assert_eq!(sink.rows(), 1);
    Box::new(session).close().await?;
    drop(request);
    pool.shutdown().await;
    Ok(())
}
