// SPDX-License-Identifier: Apache-2.0
//! Server readiness and the test database, run first by
//! `scripts/test-mssql.sh` instead of `sqlcmd` inside the container.
use crate::support::{TestResult, ensure_database};

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn server_accepts_logins_and_test_database_exists() -> TestResult {
    ensure_database().await
}
