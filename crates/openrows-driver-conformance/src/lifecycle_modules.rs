// SPDX-License-Identifier: Apache-2.0
//! Capability-gated trigger and routine lifecycle checks.
use crate::lifecycle::{apply, listed, name};
use crate::{Failure, Group, Target, ensure, fixture_name};
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::objects::{ObjectChange, RoutineKind};
use openrows_driver_api::{Connection, Feature, catalog};
use openrows_sql::Dialect;

pub(crate) async fn run(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    let caps = target.driver.capabilities();
    if caps.supports(Feature::TriggerEditing) {
        trigger(target, connection, table).await?;
    }
    if caps.supports(Feature::RoutineEditing) {
        for (kind, object) in [
            (RoutineKind::Function, ObjectKind::Function),
            (RoutineKind::Procedure, ObjectKind::Procedure),
        ] {
            routine(target, connection, (kind, object)).await?;
        }
    }
    Ok(())
}

fn qualified(target: &Target, object: &str) -> Result<String, Failure> {
    target
        .driver
        .dialect()
        .qualified(&target.schema, object)
        .map_err(|error| Failure::Setup(error.to_string()))
}

async fn trigger(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
) -> Result<(), Failure> {
    let trigger = fixture_name(Group::Lifecycle, "trigger");
    let function = fixture_name(Group::Lifecycle, "trigger_fn");
    let relation = qualified(target, table)?;
    let routine = qualified(target, &function)?;
    let definition = match target.driver.dialect() {
        Dialect::Postgres => format!(
            "CREATE FUNCTION {routine}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$; CREATE TRIGGER {trigger} BEFORE INSERT ON {relation} FOR EACH ROW EXECUTE FUNCTION {routine}()"
        ),
        Dialect::MySql => format!(
            "CREATE TRIGGER {} BEFORE INSERT ON {relation} FOR EACH ROW SET NEW.title = NEW.title",
            qualified(target, &trigger)?
        ),
        Dialect::Sqlite => format!(
            "CREATE TRIGGER {} AFTER INSERT ON {relation} BEGIN SELECT 1; END",
            qualified(target, &trigger)?
        ),
        Dialect::SqlServer => format!(
            "CREATE TRIGGER {} ON {relation} AFTER INSERT AS BEGIN SET NOCOUNT ON; END",
            qualified(target, &trigger)?
        ),
        other => {
            return Err(Failure::Contract(format!(
                "missing trigger fixture for {other:?}"
            )));
        }
    };
    let save = ObjectChange::SaveTrigger {
        table: name(target, table),
        replaces: None,
        definition,
    };
    let drop = ObjectChange::DropTrigger {
        table: name(target, table),
        name: trigger.clone(),
    };
    let outcome = trigger_check(target, connection, table, (&trigger, &save, &drop)).await;
    if outcome.is_err() {
        let _ = apply(target, connection, &drop).await;
    }
    if target.driver.dialect() == Dialect::Postgres {
        let removed = apply(
            target,
            connection,
            &ObjectChange::DropRoutine {
                routine: name(target, &function),
                kind: RoutineKind::Function,
                arguments: Some(String::new()),
            },
        )
        .await;
        if outcome.is_ok() {
            removed?;
        }
    }

    outcome
}

async fn trigger_check(
    target: &Target,
    connection: &mut dyn Connection,
    table: &str,
    (trigger, save, drop): (&str, &ObjectChange, &ObjectChange),
) -> Result<(), Failure> {
    apply(target, connection, save).await?;
    let detail = catalog(connection)?
        .table_detail(&target.schema, table)
        .await?;
    ensure(detail.triggers.iter().any(|t| t.name == trigger), || {
        format!("created trigger {trigger} not listed")
    })?;
    apply(target, connection, drop).await?;
    let detail = catalog(connection)?
        .table_detail(&target.schema, table)
        .await?;
    ensure(!detail.triggers.iter().any(|t| t.name == trigger), || {
        format!("dropped trigger {trigger} still listed")
    })
}

async fn routine(
    target: &Target,
    connection: &mut dyn Connection,
    (kind, object): (RoutineKind, ObjectKind),
) -> Result<(), Failure> {
    let routine = fixture_name(Group::Lifecycle, object.as_str());
    let qualified = qualified(target, &routine)?;
    let definition = match (target.driver.dialect(), kind) {
        (Dialect::Postgres, RoutineKind::Function) => {
            format!("CREATE FUNCTION {qualified}() RETURNS integer LANGUAGE sql AS 'SELECT 1'")
        }
        (Dialect::Postgres, RoutineKind::Procedure) => {
            format!("CREATE PROCEDURE {qualified}() LANGUAGE plpgsql AS $$ BEGIN NULL; END; $$")
        }
        (Dialect::MySql, RoutineKind::Function) => {
            format!("CREATE FUNCTION {qualified}() RETURNS INT DETERMINISTIC RETURN 1")
        }
        (Dialect::MySql, RoutineKind::Procedure) => {
            format!("CREATE PROCEDURE {qualified}() SELECT 1")
        }
        (Dialect::SqlServer, RoutineKind::Function) => {
            format!("CREATE FUNCTION {qualified}() RETURNS int AS BEGIN RETURN 1; END")
        }
        (Dialect::SqlServer, RoutineKind::Procedure) => {
            format!("CREATE PROCEDURE {qualified} AS SELECT 1")
        }
        other => {
            return Err(Failure::Contract(format!(
                "missing routine fixture for {other:?}"
            )));
        }
    };
    let save = ObjectChange::SaveRoutine {
        routine: name(target, &routine),
        kind,
        replaces: None,
        definition,
    };
    let drop = ObjectChange::DropRoutine {
        routine: name(target, &routine),
        kind,
        arguments: Some(String::new()),
    };
    let outcome = routine_check(target, connection, (&routine, object, kind), (&save, &drop)).await;
    if outcome.is_err() {
        let _ = apply(target, connection, &drop).await;
    }
    outcome
}

async fn routine_check(
    target: &Target,
    connection: &mut dyn Connection,
    (routine, object, kind): (&str, ObjectKind, RoutineKind),
    (save, drop): (&ObjectChange, &ObjectChange),
) -> Result<(), Failure> {
    apply(target, connection, save).await?;
    let objects = listed(target, connection, object).await?;
    ensure(objects.iter().any(|n| n == routine), || {
        format!("created routine {routine} not listed")
    })?;
    let definition = catalog(connection)?
        .routine_definition((&target.schema, routine), kind, Some(""))
        .await?;
    ensure(definition.is_some_and(|s| !s.trim().is_empty()), || {
        format!("routine {routine} has no definition")
    })?;
    apply(target, connection, drop).await?;
    let objects = listed(target, connection, object).await?;
    ensure(!objects.iter().any(|n| n == routine), || {
        format!("dropped routine {routine} still listed")
    })
}
