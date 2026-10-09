// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Errors`]: SQLSTATE mapping of common server errors; the
//! session stays usable after each.

use openrows_driver_api::Connection;

use crate::dialect::{codes, create_table};
use crate::sink::{ping, query, run_all, sqlstate};
use crate::{Failure, Group, Target, ensure, fixture_name};

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let expected = codes(dialect);
    let table = fixture_name(Group::Errors, "t");
    let relation = dialect
        .qualified(&target.schema, &table)
        .map_err(|error| Failure::Setup(error.to_string()))?;
    let mut connection = target.connect().await?;
    run_all(
        connection.as_mut(),
        &[
            create_table(&relation),
            format!("INSERT INTO {relation} (id, name) VALUES (1, 'one')"),
        ],
    )
    .await?;
    let missing = fixture_name(Group::Errors, "missing");
    let cases = [
        ("SELEC 1".to_owned(), "syntax error", expected.syntax),
        (
            format!("SELECT * FROM {missing}"),
            "undefined table",
            expected.undefined_table,
        ),
        (
            format!("INSERT INTO {relation} (id, name) VALUES (2, 'one')"),
            "unique violation",
            expected.unique,
        ),
        (
            format!("INSERT INTO {relation} (id, name) VALUES (3, NULL)"),
            "not-null violation",
            expected.not_null,
        ),
    ];
    let outcome = check_all(connection.as_mut(), &cases).await;
    query(connection.as_mut(), &format!("DROP TABLE {relation}")).await?;
    connection.close().await?;
    outcome
}

async fn check_all(
    connection: &mut dyn Connection,
    cases: &[(String, &str, &str)],
) -> Result<(), Failure> {
    for (sql, what, code) in cases {
        let found = sqlstate(query(connection, sql).await, what)?;
        ensure(found == *code, || {
            format!("{what}: SQLSTATE {found}, expected {code}")
        })?;
        ping(connection).await?;
    }
    Ok(())
}
