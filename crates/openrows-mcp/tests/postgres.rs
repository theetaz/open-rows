// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` read-only enforcement with persisted state verification.
#[path = "support/read.rs"]
mod fixture;
#[tokio::test]
#[ignore = "requires just test-pg's single native PostgreSQL fixture"]
async fn postgres_refuses_attacks_with_zero_changes() -> Result<(), Box<dyn std::error::Error>> {
    let request = fixture::server_request(
        "OPENROWS_TEST_PG_PORT",
        "OPENROWS_TEST_PG_PASSWORD",
        "postgres",
        "postgres",
    )?;
    fixture::exercise(
        &openrows_driver_postgres::PgDriver,
        &request,
        openrows_mcp::ReadEngine::Postgres,
        "public",
    )
    .await
}

#[tokio::test]
#[ignore = "requires just test-pg's native fixture"]
async fn postgres_implicit_user_code_tables_fail_closed() -> Result<(), Box<dyn std::error::Error>>
{
    use openrows_driver_api::{ConnectTrace, DiscardSink, Driver};
    use openrows_mcp::{McpError, ReadEngine, protected_read};
    let request = fixture::server_request(
        "OPENROWS_TEST_PG_PORT",
        "OPENROWS_TEST_PG_PASSWORD",
        "postgres",
        "postgres",
    )?;
    let mut admin = openrows_driver_postgres::PgDriver
        .connect(&request, &mut ConnectTrace::default())
        .await?;
    for (name, definition) in [
        (
            "ai_rls",
            "CREATE TABLE ai_rls(id INT); ALTER TABLE ai_rls ENABLE ROW LEVEL SECURITY",
        ),
        (
            "ai_generated",
            "CREATE TABLE ai_generated(id INT, computed INT GENERATED ALWAYS AS (id + 1) STORED)",
        ),
        ("ai_array", "CREATE TABLE ai_array(items INT[])"),
    ] {
        for statement in openrows_sql::split(definition, openrows_sql::Dialect::Postgres) {
            admin
                .execute(statement.text(definition), &mut DiscardSink)
                .await?;
        }
        let connection = openrows_driver_postgres::PgDriver
            .connect(&request, &mut ConnectTrace::default())
            .await?;
        assert!(matches!(
            protected_read(
                connection,
                ReadEngine::Postgres,
                &format!("SELECT * FROM public.{name}"),
                false
            )
            .await,
            Err(McpError::Unsafe)
        ));
        admin
            .execute(&format!("DROP TABLE {name}"), &mut DiscardSink)
            .await?;
    }
    admin.close().await?;
    Ok(())
}
