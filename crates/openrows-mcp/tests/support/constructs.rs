// SPDX-License-Identifier: Apache-2.0
//! Positive constructs and implicit generated-expression refusal fixtures.
use super::TestResult;
use openrows_driver_api::{ConnectRequest, ConnectTrace, DiscardSink, Driver};
use openrows_mcp::{ReadEngine, protected_read};
pub(super) async fn verify_constructs(
    driver: &dyn Driver,
    request: &ConnectRequest,
    engine: ReadEngine,
    schema: &str,
) -> TestResult {
    for sql in [
        "SELECT a.id FROM main.ai_fixture AS a JOIN main.ai_fixture AS b ON a.id=b.id",
        "SELECT id FROM main.ai_fixture WHERE EXISTS (SELECT 1 FROM main.ai_fixture)",
        "SELECT id FROM (SELECT id FROM main.ai_fixture) AS selected",
        "WITH x AS (SELECT id FROM main.ai_fixture) SELECT id FROM x",
        "WITH x AS (SELECT id FROM main.ai_fixture), y AS (SELECT id FROM x) SELECT id FROM y",
        "SELECT id, count(*) AS total FROM main.ai_fixture GROUP BY id HAVING count(*)>0",
        "SELECT id, row_number() OVER (ORDER BY id) AS position FROM main.ai_fixture",
        "SELECT id FROM main.ai_fixture UNION SELECT id FROM main.ai_fixture ORDER BY id LIMIT 10",
    ] {
        let sql = sql.replace("main.", &format!("{schema}."));
        let connection = driver
            .connect(request, &mut ConnectTrace::default())
            .await?;
        let result = protected_read(connection, engine, &sql, false).await?;
        assert_eq!(result.samples[0].rows.len(), 1, "{engine:?}: {sql}");
        if sql.contains(" AS ") || sql.contains("WITH ") || sql.contains("UNION") {
            assert!(
                result.samples[0]
                    .columns
                    .iter()
                    .all(|column| column == "secret")
            );
        }
    }
    Ok(())
}
pub(super) async fn verify_generated(
    driver: &dyn Driver,
    request: &ConnectRequest,
    (engine, schema): (ReadEngine, &str),
    admin: &mut dyn openrows_driver_api::Connection,
) -> TestResult {
    if !matches!(engine, ReadEngine::Mysql | ReadEngine::Mariadb) {
        return Ok(());
    }
    admin
        .execute(
            "CREATE TABLE ai_generated(id INT, computed INT GENERATED ALWAYS AS (id+1) VIRTUAL)",
            &mut DiscardSink,
        )
        .await?;
    for sql in [
        "SELECT id FROM main.ai_generated",
        "SELECT f.id FROM main.ai_fixture f JOIN main.ai_generated g ON f.id=g.id",
        "WITH x AS (SELECT id FROM main.ai_generated) SELECT * FROM x",
    ] {
        let connection = driver
            .connect(request, &mut ConnectTrace::default())
            .await?;
        assert!(
            protected_read(
                connection,
                engine,
                &sql.replace("main.", &format!("{schema}.")),
                false
            )
            .await
            .is_err()
        );
    }
    admin
        .execute("DROP TABLE ai_generated", &mut DiscardSink)
        .await?;
    Ok(())
}

pub(super) async fn verify_function_resolution(
    driver: &dyn Driver,
    request: &ConnectRequest,
    (engine, schema): (ReadEngine, &str),
    admin: &mut dyn openrows_driver_api::Connection,
) -> TestResult {
    if !matches!(engine, ReadEngine::Mysql | ReadEngine::Mariadb) {
        return Ok(());
    }
    admin
        .execute(
            "CREATE FUNCTION count (input_value INT) RETURNS INT DETERMINISTIC RETURN 71",
            &mut DiscardSink,
        )
        .await?;
    // With IGNORE_SPACE off this calls the stored routine, not native COUNT.
    let mut rows = super::Collect::default();
    admin.execute("SELECT count (1)", &mut rows).await?;
    assert_eq!(rows.values, vec!["71"]);
    for sql in [
        "SELECT count (id) FROM main.ai_fixture",
        "SELECT sum\n(id) FROM main.ai_fixture",
        "SELECT max/**/(id) FROM main.ai_fixture",
        "SELECT min -- comment\n(id) FROM main.ai_fixture",
        "SELECT 'a\\b' FROM main.ai_fixture",
    ] {
        let connection = driver
            .connect(request, &mut ConnectTrace::default())
            .await?;
        assert!(
            protected_read(
                connection,
                engine,
                &sql.replace("main.", &format!("{schema}.")),
                false
            )
            .await
            .is_err()
        );
        let mut unchanged = super::Collect::default();
        admin
            .execute("SELECT id FROM ai_fixture", &mut unchanged)
            .await?;
        assert_eq!(unchanged.values, vec!["1"]);
    }
    admin
        .execute("DROP FUNCTION count", &mut DiscardSink)
        .await?;
    Ok(())
}

pub(super) async fn verify_postgres_case(
    driver: &dyn Driver,
    request: &ConnectRequest,
    (engine, schema): (ReadEngine, &str),
    admin: &mut dyn openrows_driver_api::Connection,
) -> TestResult {
    if engine != ReadEngine::Postgres {
        return Ok(());
    }
    admin
        .execute("CREATE TABLE \"AI_VIEW\"(id INT)", &mut DiscardSink)
        .await?;
    for sql in [
        "SELECT id FROM main.AI_VIEW",
        "SELECT f.id FROM main.ai_fixture f JOIN main.AI_VIEW v ON f.id=v.id",
        "WITH x AS (SELECT id FROM main.AI_VIEW) SELECT * FROM x",
    ] {
        let connection = driver
            .connect(request, &mut ConnectTrace::default())
            .await?;
        assert!(
            protected_read(
                connection,
                engine,
                &sql.replace("main.", &format!("{schema}.")),
                false
            )
            .await
            .is_err()
        );
        let mut unchanged = super::Collect::default();
        admin
            .execute("SELECT id FROM ai_fixture", &mut unchanged)
            .await?;
        assert_eq!(unchanged.values, vec!["1"]);
    }
    let connection = driver
        .connect(request, &mut ConnectTrace::default())
        .await?;
    let sql =
        format!("WITH Selected AS (SELECT id FROM {schema}.ai_fixture) SELECT id FROM selected");
    assert_eq!(
        protected_read(connection, engine, &sql, false)
            .await?
            .samples[0]
            .rows
            .len(),
        1
    );
    admin
        .execute("DROP TABLE \"AI_VIEW\"", &mut DiscardSink)
        .await?;
    Ok(())
}
