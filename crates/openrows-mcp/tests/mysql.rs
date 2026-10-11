// SPDX-License-Identifier: Apache-2.0
//! `MySQL` and `MariaDB` read-only enforcement with persisted state verification.
#[path = "support/read.rs"]
mod fixture;
#[tokio::test]
#[ignore = "requires just test-mysql's single native MySQL/MariaDB fixture"]
async fn mysql_refuses_attacks_with_zero_changes() -> Result<(), Box<dyn std::error::Error>> {
    let maria = std::env::var("OPENROWS_TEST_MYSQL_ENGINE").is_ok_and(|engine| engine == "mariadb");
    let flavour = if maria {
        openrows_driver_mysql::Flavour::MariaDb
    } else {
        openrows_driver_mysql::Flavour::MySql
    };
    let engine = if maria {
        openrows_mcp::ReadEngine::Mariadb
    } else {
        openrows_mcp::ReadEngine::Mysql
    };
    let request = fixture::server_request(
        "OPENROWS_TEST_MYSQL_PORT",
        "OPENROWS_TEST_MYSQL_PASSWORD",
        "root",
        "openrows_test",
    )?;
    fixture::exercise(
        &openrows_driver_mysql::MySqlDriver(flavour),
        &request,
        engine,
        "openrows_test",
    )
    .await
}
