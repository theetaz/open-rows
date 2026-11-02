// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Transactions`]: statements a user types (`BEGIN`, `COMMIT`,
//! `ROLLBACK`), seen from a second connection. Data-safety code (docs/25 §0):
//! every step reads the table back.

use openrows_driver_api::{Connection, Feature};
use openrows_sql::Dialect;

use crate::dialect::{begin, codes, create_table};
use crate::sink::{query, sqlstate};
use crate::{Failure, Group, Target, ensure, fixture_name};

/// The fixture relation and its dialect.
struct Fixture {
    relation: String,
    extra: String,
    dialect: Dialect,
    transactional_ddl: bool,
}

impl Fixture {
    fn new(target: &Target) -> Result<Self, Failure> {
        let dialect = target.driver.dialect();
        let qualify = |suffix: &str| {
            dialect
                .qualified(&target.schema, &fixture_name(Group::Transactions, suffix))
                .map_err(|error| Failure::Setup(error.to_string()))
        };
        Ok(Self {
            relation: qualify("t")?,
            extra: qualify("ddl")?,
            dialect,
            transactional_ddl: target
                .driver
                .capabilities()
                .supports(Feature::TransactionalDdl),
        })
    }

    fn insert(&self, id: u32) -> String {
        format!(
            "INSERT INTO {} (id, name) VALUES ({id}, 'row {id}')",
            self.relation
        )
    }

    /// Ids visible to `connection`, ascending.
    async fn ids(&self, connection: &mut dyn Connection) -> Result<Vec<String>, Failure> {
        let sql = format!("SELECT id FROM {} ORDER BY id", self.relation);
        let (sink, _) = query(connection, &sql).await?;
        Ok(sink.column(0)?.into_iter().flatten().collect())
    }

    async fn sees(
        &self,
        connection: &mut dyn Connection,
        expected: &[&str],
        step: &str,
    ) -> Result<(), Failure> {
        let found = self.ids(connection).await?;
        ensure(found == expected, || {
            format!("{step}: ids {found:?}, expected {expected:?}")
        })
    }

    /// Start a transaction, which the session must report.
    async fn begin(&self, connection: &mut dyn Connection) -> Result<(), Failure> {
        query(connection, begin(self.dialect)).await?;
        ensure(connection.in_transaction(), || {
            "BEGIN not reported as an open transaction".to_owned()
        })
    }

    /// End a transaction with `sql`, which the session must report.
    async fn end(&self, connection: &mut dyn Connection, sql: &str) -> Result<(), Failure> {
        query(connection, sql).await?;
        ensure(!connection.in_transaction(), || {
            format!("{sql} left the transaction reported as open")
        })
    }
}

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let fixture = Fixture::new(target)?;
    let mut writer = target.connect().await?;
    let mut reader = target.connect().await?;
    read_without_waiting(reader.as_mut()).await?;
    query(writer.as_mut(), &create_table(&fixture.relation)).await?;
    let outcome = contract(target, &fixture, writer.as_mut(), reader.as_mut()).await;
    if writer.in_transaction() {
        query(writer.as_mut(), "ROLLBACK").await?;
    }
    query(writer.as_mut(), &format!("DROP TABLE {}", fixture.relation)).await?;
    writer.close().await?;
    reader.close().await?;
    outcome
}

/// `CockroachDB` serializes by default, where a reader waits for an open
/// writer instead of ignoring its changes.
async fn read_without_waiting(reader: &mut dyn Connection) -> Result<(), Failure> {
    if reader.server().product == "CockroachDB" {
        query(
            reader,
            "SET default_transaction_isolation = 'read committed'",
        )
        .await?;
    }
    Ok(())
}

async fn contract(
    target: &Target,
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    commit_publishes(fixture, writer, reader).await?;
    rollback_discards(fixture, writer, reader).await?;
    if fixture.dialect == Dialect::Postgres {
        failure_aborts(fixture, writer, reader).await?;
    } else {
        failure_keeps_going(fixture, writer, reader).await?;
    }
    savepoint_recovers(fixture, writer, reader).await?;
    closed_connection_rolls_back(target, fixture, writer, reader).await?;
    ddl_in_transaction(fixture, writer, reader).await
}

async fn commit_publishes(
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    fixture.begin(writer).await?;
    query(writer, &fixture.insert(1)).await?;
    fixture.sees(writer, &["1"], "own uncommitted row").await?;
    fixture
        .sees(reader, &[], "another connection before commit")
        .await?;
    fixture.end(writer, "COMMIT").await?;
    fixture
        .sees(reader, &["1"], "another connection after commit")
        .await
}

async fn rollback_discards(
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    fixture.begin(writer).await?;
    query(writer, &fixture.insert(2)).await?;
    fixture.end(writer, "ROLLBACK").await?;
    fixture.sees(writer, &["1"], "after rollback").await?;
    fixture
        .sees(reader, &["1"], "another connection after rollback")
        .await
}

/// The duplicate of row 1 fails with the unique SQLSTATE inside a transaction
/// holding row 3.
async fn fail_a_statement(fixture: &Fixture, writer: &mut dyn Connection) -> Result<(), Failure> {
    fixture.begin(writer).await?;
    query(writer, &fixture.insert(3)).await?;
    let code = sqlstate(query(writer, &fixture.insert(1)).await, "duplicate key")?;
    let expected = codes(fixture.dialect).unique;
    ensure(code == expected, || {
        format!("duplicate key in a transaction: SQLSTATE {code}, expected {expected}")
    })?;
    ensure(writer.in_transaction(), || {
        "a failed statement ended the transaction".to_owned()
    })
}

/// `PostgreSQL`: statements fail until the transaction ends; `COMMIT` rolls
/// back and nothing of it survives.
async fn failure_aborts(
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    fail_a_statement(fixture, writer).await?;
    let code = sqlstate(query(writer, "SELECT 1").await, "statement after failure")?;
    ensure(code == "25P02", || {
        format!("statement in an aborted transaction: SQLSTATE {code}")
    })?;
    fixture.end(writer, "COMMIT").await?;
    fixture
        .sees(writer, &["1"], "commit of an aborted transaction")
        .await?;
    fixture
        .sees(reader, &["1"], "another connection after that")
        .await
}

/// `MySQL` and `SQLite`: only the statement fails; the transaction carries on
/// and commits its earlier row.
async fn failure_keeps_going(
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    fail_a_statement(fixture, writer).await?;
    fixture
        .sees(writer, &["1", "3"], "after a failed statement")
        .await?;
    fixture.end(writer, "COMMIT").await?;
    fixture
        .sees(reader, &["1", "3"], "another connection after commit")
        .await?;
    let delete = format!("DELETE FROM {} WHERE id = 3", fixture.relation);
    query(writer, &delete).await?;
    Ok(())
}

async fn savepoint_recovers(
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    fixture.begin(writer).await?;
    query(writer, &fixture.insert(4)).await?;
    query(writer, "SAVEPOINT orc_step").await?;
    sqlstate(query(writer, &fixture.insert(1)).await, "duplicate key")?;
    query(writer, "ROLLBACK TO SAVEPOINT orc_step").await?;
    query(writer, &fixture.insert(5)).await?;
    fixture.end(writer, "COMMIT").await?;
    fixture
        .sees(reader, &["1", "4", "5"], "after a savepoint rollback")
        .await
}

/// Row 6 stays uncommitted when its connection closes; inserting it again
/// waits for the server to finish that rollback, then succeeds.
async fn closed_connection_rolls_back(
    target: &Target,
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    let mut doomed = target.connect().await?;
    fixture.begin(doomed.as_mut()).await?;
    query(doomed.as_mut(), &fixture.insert(6)).await?;
    doomed.close().await?;
    fixture
        .sees(reader, &["1", "4", "5"], "after a closed connection")
        .await?;
    query(writer, &fixture.insert(6)).await?;
    fixture
        .sees(reader, &["1", "4", "5", "6"], "row inserted again")
        .await
}

/// DDL joins the transaction with `TransactionalDdl`, and otherwise commits
/// what the transaction holds: the behaviour a user must be warned about.
async fn ddl_in_transaction(
    fixture: &Fixture,
    writer: &mut dyn Connection,
    reader: &mut dyn Connection,
) -> Result<(), Failure> {
    fixture.begin(writer).await?;
    query(writer, &fixture.insert(7)).await?;
    query(writer, &create_table(&fixture.extra)).await?;
    if fixture.transactional_ddl {
        fixture.end(writer, "ROLLBACK").await?;
        fixture
            .sees(reader, &["1", "4", "5", "6"], "rolled-back DDL transaction")
            .await?;
        let probe = query(writer, &format!("SELECT * FROM {}", fixture.extra)).await;
        sqlstate(probe, "a table created in a rolled-back transaction")?;
        return Ok(());
    }
    ensure(!writer.in_transaction(), || {
        "DDL without TransactionalDdl left the transaction open".to_owned()
    })?;
    fixture
        .sees(reader, &["1", "4", "5", "6", "7"], "implicit DDL commit")
        .await?;
    query(writer, &format!("DROP TABLE {}", fixture.extra)).await?;
    Ok(())
}
