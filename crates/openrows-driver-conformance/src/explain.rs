// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Explain`]: the statements `openrows_explain::command`
//! builds run on the real engine, and its parsers read what the engine prints.

use openrows_driver_api::Connection;
use openrows_explain::{
    AnalysedPlan, AnalyzeWrites, Command, Engine, Mode, Wrap, analyse, command, parse,
};
use openrows_sql::Dialect;

use crate::dialect::create_table;
use crate::sink::{query, run_all};
use crate::{Failure, Group, Target, ensure, fixture_name};

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let name = fixture_name(Group::Explain, "t");
    let relation = dialect
        .qualified(&target.schema, &name)
        .map_err(|error| Failure::Setup(error.to_string()))?;
    let mut connection = target.connect().await?;
    run_all(
        connection.as_mut(),
        &[
            create_table(&relation),
            format!("INSERT INTO {relation} (id, name) VALUES (1, 'a'), (2, 'b'), (3, 'c')"),
        ],
    )
    .await?;
    let outcome = contract(connection.as_mut(), dialect, (&relation, &name)).await;
    query(connection.as_mut(), &format!("DROP TABLE {relation}")).await?;
    connection.close().await?;
    outcome
}

/// `MariaDB` shares `MySQL`'s dialect but prints plans its own way.
fn engine(connection: &dyn Connection, dialect: Dialect) -> Result<Engine, Failure> {
    match dialect {
        Dialect::Postgres => Ok(Engine::Postgres),
        Dialect::MySql if connection.server().product == "MariaDB" => Ok(Engine::MariaDb),
        Dialect::MySql => Ok(Engine::MySql),
        Dialect::Sqlite => Ok(Engine::Sqlite),
        Dialect::SqlServer => Ok(Engine::SqlServer),
        other => Err(Failure::Contract(format!(
            "Explain claimed without a plan format for {other:?}"
        ))),
    }
}

async fn contract(
    connection: &mut dyn Connection,
    dialect: Dialect,
    (relation, name): (&str, &str),
) -> Result<(), Failure> {
    let engine = engine(connection, dialect)?;
    let read = format!("SELECT id, name FROM {relation} WHERE note IS NULL");
    let plan = explained(connection, engine, Mode::Plan, &read).await?;
    names_the_table(&plan, name, Mode::Plan)?;
    ensure(!plan.analyzed, || "a plain plan claims actuals".to_owned())?;
    let analysed = explained(connection, engine, Mode::Analyze, &read).await?;
    names_the_table(&analysed, name, Mode::Analyze)?;
    let executes = engine.executes(Mode::Analyze);
    ensure(analysed.analyzed == executes, || {
        format!("Analyze on {engine:?}: analyzed {}", analysed.analyzed)
    })?;
    if engine.analyze_writes() == AnalyzeWrites::RollBack {
        analysing_a_write_changes_nothing(connection, engine, relation).await?;
    }
    Ok(())
}

fn names_the_table(plan: &AnalysedPlan, name: &str, mode: Mode) -> Result<(), Failure> {
    let named = plan
        .nodes
        .iter()
        .filter_map(|node| node.data.relation.as_deref())
        .any(|relation| relation == name || relation.ends_with(&format!(".{name}")));
    ensure(named, || {
        let relations: Vec<_> = plan.nodes.iter().map(|node| &node.data.relation).collect();
        format!("{mode:?} plan does not name {name}: {relations:?}")
    })
}

async fn analysing_a_write_changes_nothing(
    connection: &mut dyn Connection,
    engine: Engine,
    relation: &str,
) -> Result<(), Failure> {
    let write = format!("INSERT INTO {relation} (id, name) VALUES (9, 'z')");
    explained(connection, engine, Mode::Analyze, &write).await?;
    ensure(!connection.in_transaction(), || {
        "Explain Analyze left a transaction open".to_owned()
    })?;
    let (sink, _) = query(
        connection,
        &format!("SELECT id FROM {relation} WHERE id = 9"),
    )
    .await?;
    ensure(sink.rows() == 0, || {
        "Explain Analyze kept the row it inserted".to_owned()
    })
}

async fn explained(
    connection: &mut dyn Connection,
    engine: Engine,
    mode: Mode,
    statement: &str,
) -> Result<AnalysedPlan, Failure> {
    let wrap = if engine.executes(mode) && engine.analyze_writes() == AnalyzeWrites::RollBack {
        Wrap::Transaction
    } else {
        Wrap::None
    };
    let Command {
        before,
        explain,
        after,
        format,
        ..
    } = command(engine, mode, statement, wrap);
    run_all(connection, &before).await?;
    let output = query(connection, &explain).await;
    run_all(connection, &after).await?;
    let (sink, _) = output?;
    let plan = parse(format, &sink.text_rows()?)
        .map_err(|error| Failure::Contract(format!("{mode:?} output: {error}")))?;
    Ok(analyse(plan))
}
