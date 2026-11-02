// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Notices`]: server messages reach the sink with their
//! severity, in order, without disturbing the statement or the session.

use openrows_driver_api::{Connection, NoticeLevel};
use openrows_sql::Dialect;

use crate::sink::{Collect, ping, query, sqlstate};
use crate::{Failure, Group, Target, ensure, fixture_name};

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let mut connection = target.connect().await?;
    missing_object(target, connection.as_mut()).await?;
    if dialect == Dialect::Postgres {
        postgres_levels(connection.as_mut()).await?;
        postgres_rows_and_errors(connection.as_mut()).await?;
    } else {
        mysql_warning_with_rows(connection.as_mut()).await?;
    }
    ping(connection.as_mut()).await?;
    connection.close().await?;
    Ok(())
}

/// Dropping a missing table names it in an informational message.
async fn missing_object(target: &Target, connection: &mut dyn Connection) -> Result<(), Failure> {
    let name = fixture_name(Group::Notices, "absent");
    let relation = target
        .driver
        .dialect()
        .qualified(&target.schema, &name)
        .map_err(|error| Failure::Setup(error.to_string()))?;
    let (sink, _) = query(connection, &format!("DROP TABLE IF EXISTS {relation}")).await?;
    let notice = sink
        .notices
        .iter()
        .any(|(level, message)| *level == NoticeLevel::Info && message.contains(&name));
    ensure(notice, || {
        format!("drop of a missing table: notices {:?}", sink.notices)
    })
}

/// `RAISE` severities arrive as the server sent them, in order.
async fn postgres_levels(connection: &mut dyn Connection) -> Result<(), Failure> {
    let sql = "DO $$ BEGIN RAISE NOTICE 'orc first'; RAISE WARNING 'orc second'; \
               RAISE INFO 'orc third'; END $$";
    let (sink, _) = query(connection, sql).await?;
    let found: Vec<_> = sink
        .notices
        .iter()
        .map(|(level, message)| (*level, message.as_str()))
        .collect();
    let expected = [
        (NoticeLevel::Info, "orc first"),
        (NoticeLevel::Warning, "orc second"),
        (NoticeLevel::Info, "orc third"),
    ];
    ensure(found == expected, || {
        format!("RAISE levels {found:?}, expected {expected:?}")
    })
}

/// A notice raised while a query runs arrives with its row, and one raised
/// before a statement's error is still delivered.
async fn postgres_rows_and_errors(connection: &mut dyn Connection) -> Result<(), Failure> {
    query(
        connection,
        "CREATE FUNCTION pg_temp.orc_note() RETURNS integer LANGUAGE plpgsql AS \
         $$ BEGIN RAISE NOTICE 'orc row'; RETURN 7; END $$",
    )
    .await?;
    let (sink, _) = query(connection, "SELECT pg_temp.orc_note()").await?;
    let value = sink.single()?;
    ensure(value.as_deref() == Some("7"), || {
        format!("query beside a notice answered {value:?}")
    })?;
    ensure(has_message(&sink, "orc row"), || {
        format!("notice from a query: {:?}", sink.notices)
    })?;
    let mut failing = Collect::default();
    let result = connection
        .execute(
            "DO $$ BEGIN RAISE NOTICE 'orc before'; RAISE EXCEPTION 'orc failure'; END $$",
            &mut failing,
        )
        .await;
    let code = sqlstate(result, "RAISE EXCEPTION")?;
    ensure(code == "P0001", || {
        format!("RAISE EXCEPTION: SQLSTATE {code}")
    })?;
    ensure(has_message(&failing, "orc before"), || {
        format!("notice before an error: {:?}", failing.notices)
    })
}

/// A truncating cast warns and still answers.
async fn mysql_warning_with_rows(connection: &mut dyn Connection) -> Result<(), Failure> {
    let (sink, _) = query(connection, "SELECT CAST('7x' AS SIGNED)").await?;
    let value = sink.single()?;
    ensure(value.as_deref() == Some("7"), || {
        format!("query beside a warning answered {value:?}")
    })?;
    let warned = sink
        .notices
        .iter()
        .any(|(level, _)| *level == NoticeLevel::Warning);
    ensure(warned, || {
        format!("truncating cast: notices {:?}", sink.notices)
    })
}

fn has_message(sink: &Collect, text: &str) -> bool {
    sink.notices.iter().any(|(_, message)| message == text)
}
