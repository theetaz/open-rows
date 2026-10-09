// SPDX-License-Identifier: Apache-2.0
//! Databases, schemas, objects, columns, indexes, constraints, triggers,
//! the DDL, the ER diagram, the fingerprint and the process list.
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::schema::{ConstraintKind, RelationKind};
use openrows_driver_api::{Admin, Catalog, Driver, ProcessSignal};
use openrows_driver_mssql::{SqlServerDriver, SqlServerSession};

use super::support::{Collect, DATABASE, TestResult, code, fixture, run, session};

async fn fixture_schema(session: &mut SqlServerSession, schema: &str) -> TestResult {
    run(
        session,
        &[
            &format!("CREATE SCHEMA {schema}"),
            &format!(
                "CREATE TABLE {schema}.parent (id int IDENTITY PRIMARY KEY, \
                 code varchar(10) COLLATE Latin1_General_BIN NOT NULL CONSTRAINT uq_code UNIQUE, \
                 qty int NOT NULL CONSTRAINT df_qty DEFAULT 1 CONSTRAINT ck_qty CHECK (qty > 0), \
                 total AS qty * 2)"
            ),
            &format!(
                "CREATE TABLE {schema}.child (id int PRIMARY KEY, parent_id int \
                 CONSTRAINT fk_parent REFERENCES {schema}.parent (id), note nvarchar(max))"
            ),
            &format!("CREATE INDEX ix_child ON {schema}.child (parent_id) INCLUDE (note)"),
            &format!("CREATE VIEW {schema}.v AS SELECT id FROM {schema}.child"),
            &format!("CREATE TRIGGER {schema}.tr ON {schema}.child AFTER INSERT, DELETE AS SET NOCOUNT ON"),
            &format!(
                "EXEC sys.sp_addextendedproperty N'MS_Description', N'parents', N'SCHEMA', \
                 N'{schema}', N'TABLE', N'parent'"
            ),
        ],
    )
    .await?;
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn the_catalog_lists_and_details_a_schema() -> TestResult {
    let mut session = session().await?;
    let schema = fixture("cat");
    fixture_schema(&mut session, &schema).await?;
    let outcome = check(&mut session, &schema).await;
    run(
        &mut session,
        &[
            &format!("DROP VIEW {schema}.v"),
            &format!("DROP TABLE {schema}.child"),
            &format!("DROP TABLE {schema}.parent"),
            &format!("DROP SCHEMA {schema}"),
        ],
    )
    .await?;
    outcome
}

async fn check(session: &mut SqlServerSession, schema: &str) -> TestResult {
    let databases = session.list_databases().await?;
    assert!(databases.iter().any(|db| db.name == DATABASE && !db.system));
    assert!(databases.iter().any(|db| db.name == "master" && db.system));
    let schemas = session.list_schemas().await?;
    assert!(schemas.iter().any(|s| s.name == schema && !s.system));
    assert!(schemas.iter().any(|s| s.name == "sys" && s.system));
    let objects = session.list_objects(schema, &ObjectKind::ALL).await?;
    let listed: Vec<_> = objects.iter().map(|o| (o.name.as_str(), o.kind)).collect();
    assert_eq!(
        listed,
        [
            ("child", ObjectKind::Table),
            ("parent", ObjectKind::Table),
            ("v", ObjectKind::View)
        ]
    );
    assert_eq!(objects[1].comment.as_deref(), Some("parents"));
    assert!(
        session
            .search_index()
            .await?
            .iter()
            .any(|o| o.schema == schema && o.name == "v")
    );
    let columns = session.table_columns(schema, "parent").await?;
    let shape: Vec<_> = columns
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.data_type.as_str(),
                c.nullable,
                c.primary_key,
            )
        })
        .collect();
    assert_eq!(
        shape,
        [
            ("id", "int", false, true),
            ("code", "varchar(10)", false, false),
            ("qty", "int", false, false),
            ("total", "int", true, false)
        ]
    );
    detail(session, schema).await?;
    diagram(session, schema).await
}

async fn detail(session: &mut SqlServerSession, schema: &str) -> TestResult {
    let parent = session.table_detail(schema, "parent").await?;
    let kinds: Vec<_> = parent.constraints.iter().map(|c| c.kind).collect();
    assert!(kinds.contains(&ConstraintKind::Primary));
    assert!(kinds.contains(&ConstraintKind::Unique));
    assert!(kinds.contains(&ConstraintKind::Check));
    let ddl = SqlServerDriver.table_ddl((schema, "parent"), &parent)?;
    assert!(
        ddl.contains("[code] varchar(10) COLLATE Latin1_General_BIN NOT NULL"),
        "{ddl}"
    );
    assert!(ddl.contains("CONSTRAINT [df_qty] DEFAULT ((1))"), "{ddl}");
    assert!(ddl.contains("[total] AS ([qty]*(2))"), "{ddl}");
    let child = session.table_detail(schema, "child").await?;
    let foreign = child
        .constraints
        .iter()
        .find(|c| c.kind == ConstraintKind::Foreign)
        .ok_or("no foreign key")?;
    assert_eq!(foreign.columns, ["parent_id"]);
    assert_eq!(
        foreign.references.as_ref().map(|t| t.table.as_str()),
        Some("parent")
    );
    let index = child
        .indexes
        .iter()
        .find(|i| i.name == "ix_child")
        .ok_or("no index")?;
    assert_eq!(index.include, ["note"]);
    assert_eq!(child.triggers.len(), 1);
    assert_eq!(child.triggers[0].events.len(), 2);
    let view = session.table_detail(schema, "v").await?;
    assert_eq!(
        view.info.as_ref().map(|info| info.kind),
        Some(RelationKind::View)
    );
    assert!(
        view.info
            .and_then(|info| info.view_definition)
            .is_some_and(|d| d.contains("CREATE VIEW"))
    );
    Ok(())
}

async fn diagram(session: &mut SqlServerSession, schema: &str) -> TestResult {
    let diagram = session.schema_diagram(schema, 10).await?;
    let tables: Vec<_> = diagram.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tables, ["child", "parent"]);
    assert_eq!(diagram.foreign_keys.len(), 1);
    assert_eq!(diagram.foreign_keys[0].target.table, "parent");
    let before = session
        .schema_fingerprint()
        .await?
        .ok_or("no fingerprint")?;
    run(
        session,
        &[&format!("ALTER TABLE {schema}.child ADD extra int")],
    )
    .await?;
    let after = session
        .schema_fingerprint()
        .await?
        .ok_or("no fingerprint")?;
    assert_ne!(before, after);
    Ok(())
}

#[tokio::test]
#[ignore = "needs SQL Server; run with `just test-mssql`"]
async fn processes_are_listed_and_terminated() -> TestResult {
    let mut lister = session().await?;
    let mut other = session().await?;
    let own = lister.own_process().await?;
    let other_id = other.own_process().await?;
    let processes = lister.list_processes().await?;
    let current: Vec<_> = processes
        .iter()
        .filter(|p| p.current)
        .map(|p| p.id)
        .collect();
    assert_eq!(current, [own]);
    assert!(
        processes
            .iter()
            .any(|p| p.id == other_id && p.application.as_deref() == Some("openrows-test"))
    );
    let cancel = lister
        .signal_process(other_id, ProcessSignal::CancelQuery)
        .await;
    assert!(code(cancel).contains("Unsupported"));
    lister
        .signal_process(other_id, ProcessSignal::Terminate)
        .await?;
    // KILL returns before the session is gone.
    let mut ticks = tokio::time::interval(std::time::Duration::from_millis(100));
    for _ in 0..150 {
        ticks.tick().await;
        if lister
            .list_processes()
            .await?
            .iter()
            .all(|p| p.id != other_id)
        {
            break;
        }
    }
    let after =
        openrows_driver_api::Session::execute(&mut other, "SELECT 1", &mut Collect::default())
            .await;
    assert!(after.is_err());
    // Session ids are reused at once, so a gone one may already name a new
    // session: probe the mapping with an id no session can have.
    let unknown = lister
        .signal_process(32_767, ProcessSignal::Terminate)
        .await;
    assert_eq!(code(unknown), "42704");
    Ok(())
}
