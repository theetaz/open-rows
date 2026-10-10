// SPDX-License-Identifier: Apache-2.0
//! `SQLite` catalog SQL over Hrana: views, indexes, triggers and foreign keys.
use super::support;
use openrows_driver_api::{Catalog, DiscardSink, DriverError, Editor, Session};
use openrows_driver_api::{
    catalog::ObjectKind, inspection::ObjectIdentity, schema::ConstraintKind,
};
#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn catalog_and_view_inspection_keep_complete_definitions() -> Result<(), DriverError> {
    let mut session = support::session().await?;
    session.sequence("CREATE TABLE libsql_parent(id INTEGER PRIMARY KEY); CREATE TABLE libsql_child(id INTEGER PRIMARY KEY, parent INTEGER REFERENCES libsql_parent(id), value TEXT); CREATE INDEX libsql_child_value ON libsql_child(value); CREATE TRIGGER libsql_child_trigger AFTER INSERT ON libsql_child BEGIN UPDATE libsql_child SET value='x' WHERE id=NEW.id; END; CREATE VIEW libsql_view AS SELECT id, value FROM libsql_child").await?;
    let objects = session
        .list_objects("main", &[ObjectKind::Table, ObjectKind::View])
        .await?;
    assert!(
        objects
            .iter()
            .any(|object| object.name == "libsql_view" && object.kind == ObjectKind::View)
    );
    let detail = session.table_detail("main", "libsql_child").await?;
    assert_eq!(detail.columns.len(), 3);
    assert!(
        detail
            .constraints
            .iter()
            .any(|key| key.kind == ConstraintKind::Foreign)
    );
    assert!(
        detail
            .indexes
            .iter()
            .any(|index| index.name == "libsql_child_value")
    );
    assert_eq!(detail.triggers.len(), 1);
    let view = session
        .object_detail(&ObjectIdentity {
            schema: "main".into(),
            name: "libsql_view".into(),
            kind: ObjectKind::View,
            arguments: None,
        })
        .await?;
    assert!(format!("{view:?}").contains("SELECT id, value"));
    let info = session.row_edit_info("main", "libsql_child").await?;
    assert_eq!(info.key, ["id"]);
    session
        .execute("DROP VIEW libsql_view", &mut DiscardSink)
        .await?;
    session
        .sequence("DROP TABLE libsql_child; DROP TABLE libsql_parent")
        .await?;
    Box::new(session).close().await
}
