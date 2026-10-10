// SPDX-License-Identifier: Apache-2.0
//! Stored `SQLite` definitions shared by file and remote catalogs.
use openrows_driver_api::schema::{
    ConstraintDetail, ConstraintKind, IndexDetail, TriggerDetail, TriggerEvent, TriggerTiming,
};
use openrows_sql::{Dialect, ddl::TableDefinition};

/// Table-level CHECK constraints from stored SQL, where `SQLite` has no pragma.
#[must_use]
pub fn checks(table: &str, create: Option<&str>) -> Vec<ConstraintDetail> {
    let Some(definition) = create.and_then(|sql| TableDefinition::parse(sql, Dialect::Sqlite))
    else {
        return Vec::new();
    };
    crate::ddl::table_checks(table, &definition)
        .into_iter()
        .map(|check| ConstraintDetail {
            name: check.name,
            kind: ConstraintKind::Check,
            definition: check.text,
            columns: Vec::new(),
            references: None,
        })
        .collect()
}

/// Timing and events read from the trigger's header (`… BEFORE UPDATE OF a ON t …`).
#[must_use]
pub fn trigger(name: String, definition: String) -> TriggerDetail {
    let upper = definition.to_ascii_uppercase();
    let header = upper.split(" ON ").next().unwrap_or_default();
    let timing = if header.contains(" INSTEAD OF ") {
        TriggerTiming::InsteadOf
    } else if header.contains(" BEFORE ") {
        TriggerTiming::Before
    } else {
        TriggerTiming::After
    };
    let events = [
        (" INSERT", TriggerEvent::Insert),
        (" UPDATE", TriggerEvent::Update),
        (" DELETE", TriggerEvent::Delete),
    ]
    .into_iter()
    .filter(|(word, _)| header.contains(word))
    .map(|(_, event)| event)
    .collect();
    TriggerDetail {
        name,
        timing,
        events,
        row_level: true,
        function: String::new(),
        enabled: true,
        definition,
    }
}

/// The DDL view: the stored `CREATE` statements, table first.
pub fn definition(
    create: Option<&str>,
    indexes: &[IndexDetail],
    triggers: &[TriggerDetail],
) -> String {
    let mut statements: Vec<String> = create.map(str::to_owned).into_iter().collect();
    statements.extend(
        indexes
            .iter()
            .filter(|index| !index.definition.starts_with("--"))
            .map(|index| index.definition.clone()),
    );
    statements.extend(triggers.iter().map(|trigger| trigger.definition.clone()));
    statements
        .iter()
        .fold(String::new(), |mut text, statement| {
            text.push_str(statement);
            text.push_str(";\n");
            text
        })
}
