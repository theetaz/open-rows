// SPDX-License-Identifier: Apache-2.0
//! Catalog table detail → the webview's `TableDetailDto`.
use super::schema_dto::{
    ForeignKeyDto, GeneratedDto, IndexDetailDto, PartitionDto, RelationKindDto, StructureColumnDto,
    TableDetailDto, TableInfoDto, TriggerDto, TriggerTimingDto,
};
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, Generated, IndexDetail, PartitionDetail,
    RelationKind, TableDetail, TableInfo, TriggerDetail, TriggerEvent, TriggerTiming,
};

/// The DTO of an existing relation (`None` when it was not found); `ddl` is
/// its reconstructed definition.
pub(crate) fn detail_dto(
    detail: TableDetail,
    ddl: String,
    read_only: bool,
) -> Option<TableDetailDto> {
    let info = detail.info?;
    let primary_key = detail
        .constraints
        .iter()
        .find(|constraint| constraint.kind == ConstraintKind::Primary)
        .map(|constraint| constraint.columns.clone())
        .unwrap_or_default();
    let editable = !read_only
        && matches!(
            info.kind,
            RelationKind::Table | RelationKind::PartitionedTable
        );
    Some(TableDetailDto {
        columns: detail
            .columns
            .into_iter()
            .map(|column| structure_column(column, &detail.constraints))
            .collect(),
        primary_key,
        indexes: detail.indexes.into_iter().map(index).collect(),
        triggers: detail.triggers.into_iter().map(trigger).collect(),
        partitions: detail.partitions.into_iter().map(partition).collect(),
        info: table_info(info),
        ddl,
        editable,
    })
}

fn single<'a>(
    constraints: &'a [ConstraintDetail],
    kind: ConstraintKind,
    column: &'a str,
) -> impl Iterator<Item = &'a ConstraintDetail> {
    constraints.iter().filter(move |item| {
        item.kind == kind && item.columns.len() == 1 && item.columns[0] == column
    })
}

fn structure_column(column: ColumnDetail, constraints: &[ConstraintDetail]) -> StructureColumnDto {
    let checks: Vec<&str> = single(constraints, ConstraintKind::Check, &column.name)
        .map(|check| {
            check
                .definition
                .strip_prefix("CHECK ")
                .unwrap_or(&check.definition)
        })
        .collect();
    let foreign_key = single(constraints, ConstraintKind::Foreign, &column.name)
        .find_map(|constraint| constraint.references.as_ref())
        .and_then(|target| {
            Some(ForeignKeyDto {
                schema: target.schema.clone(),
                table: target.table.clone(),
                column: target.columns.first()?.clone(),
            })
        });
    StructureColumnDto {
        check: (!checks.is_empty()).then(|| checks.join(" AND ")),
        foreign_key,
        name: column.name,
        data_type: column.data_type,
        nullable: column.nullable,
        default: column.default,
        generated: match column.generated {
            Generated::No => GeneratedDto::No,
            Generated::IdentityAlways => GeneratedDto::IdentityAlways,
            Generated::IdentityByDefault => GeneratedDto::IdentityByDefault,
            Generated::Stored => GeneratedDto::Stored,
        },
        comment: column.comment,
    }
}

fn index(index: IndexDetail) -> IndexDetailDto {
    IndexDetailDto {
        name: index.name,
        method: index.method,
        unique: index.unique,
        primary: index.primary,
        constraint: index.constraint,
        columns: index.columns,
        include: index.include,
        predicate: index.predicate,
        comment: index.comment,
        definition: index.definition,
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "row estimates are display values, exact below 2^53"
)]
fn partition(partition: PartitionDetail) -> PartitionDto {
    PartitionDto {
        schema: partition.schema,
        name: partition.name,
        bound: partition.bound,
        partitioned: partition.partitioned,
        row_estimate: partition.row_estimate.map(|rows| rows as f64),
    }
}

fn trigger(trigger: TriggerDetail) -> TriggerDto {
    TriggerDto {
        name: trigger.name,
        timing: match trigger.timing {
            TriggerTiming::Before => TriggerTimingDto::Before,
            TriggerTiming::After => TriggerTimingDto::After,
            TriggerTiming::InsteadOf => TriggerTimingDto::InsteadOf,
        },
        events: trigger
            .events
            .into_iter()
            .map(|event| {
                match event {
                    TriggerEvent::Insert => "INSERT",
                    TriggerEvent::Update => "UPDATE",
                    TriggerEvent::Delete => "DELETE",
                    TriggerEvent::Truncate => "TRUNCATE",
                }
                .to_owned()
            })
            .collect(),
        row_level: trigger.row_level,
        function: trigger.function,
        enabled: trigger.enabled,
        definition: trigger.definition,
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "sizes and planner estimates stay far below 2^53"
)]
fn table_info(info: TableInfo) -> TableInfoDto {
    TableInfoDto {
        kind: match info.kind {
            RelationKind::Table => RelationKindDto::Table,
            RelationKind::PartitionedTable => RelationKindDto::PartitionedTable,
            RelationKind::View => RelationKindDto::View,
            RelationKind::MaterializedView => RelationKindDto::MaterializedView,
            RelationKind::ForeignTable => RelationKindDto::ForeignTable,
        },
        row_estimate: info.row_estimate.map(|rows| rows as f64),
        total_bytes: info.total_bytes as f64,
        data_bytes: info.data_bytes as f64,
        index_bytes: info.index_bytes as f64,
        comment: info.comment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_driver_api::schema::ForeignTarget;

    fn constraint(kind: ConstraintKind, column: &str, definition: &str) -> ConstraintDetail {
        ConstraintDetail {
            name: format!("{column}_c"),
            kind,
            definition: definition.into(),
            columns: vec![column.into()],
            references: (kind == ConstraintKind::Foreign).then(|| ForeignTarget {
                schema: "public".into(),
                table: "users".into(),
                columns: vec!["id".into()],
            }),
        }
    }

    #[test]
    fn columns_gain_checks_foreign_keys_and_the_primary_key() -> Result<(), &'static str> {
        let detail = TableDetail {
            columns: vec![
                ColumnDetail {
                    name: "id".into(),
                    ..ColumnDetail::default()
                },
                ColumnDetail {
                    name: "user_id".into(),
                    ..ColumnDetail::default()
                },
            ],
            constraints: vec![
                constraint(ConstraintKind::Primary, "id", "PRIMARY KEY (id)"),
                constraint(ConstraintKind::Check, "id", "CHECK (id > 0)"),
                constraint(ConstraintKind::Check, "id", "CHECK (id < 9)"),
                constraint(ConstraintKind::Foreign, "user_id", "FOREIGN KEY …"),
            ],
            info: Some(TableInfo::default()),
            ..TableDetail::default()
        };
        let dto = detail_dto(detail, "-- ddl".into(), false).ok_or("the relation exists")?;
        assert_eq!(dto.primary_key, vec!["id"]);
        assert_eq!(
            dto.columns[0].check.as_deref(),
            Some("(id > 0) AND (id < 9)")
        );
        assert_eq!(
            dto.columns[1].foreign_key,
            Some(ForeignKeyDto {
                schema: "public".into(),
                table: "users".into(),
                column: "id".into(),
            })
        );
        assert!(dto.editable);
        Ok(())
    }

    #[test]
    fn views_and_read_only_connections_are_not_editable() {
        let view = TableDetail {
            info: Some(TableInfo {
                kind: RelationKind::View,
                ..TableInfo::default()
            }),
            ..TableDetail::default()
        };
        assert!(detail_dto(view, String::new(), false).is_some_and(|dto| !dto.editable));
        let table = TableDetail {
            info: Some(TableInfo::default()),
            ..TableDetail::default()
        };
        assert!(detail_dto(table, String::new(), true).is_some_and(|dto| !dto.editable));
        assert!(detail_dto(TableDetail::default(), String::new(), false).is_none());
    }

    #[test]
    fn partitions_keep_their_bounds_and_estimates() -> Result<(), &'static str> {
        let detail = TableDetail {
            partitions: vec![PartitionDetail {
                schema: "public".into(),
                name: "m_2024".into(),
                bound: "FOR VALUES FROM ('2024-01-01') TO ('2025-01-01')".into(),
                partitioned: false,
                row_estimate: Some(42),
            }],
            info: Some(TableInfo {
                kind: RelationKind::PartitionedTable,
                ..TableInfo::default()
            }),
            ..TableDetail::default()
        };
        let dto = detail_dto(detail, String::new(), false).ok_or("the relation exists")?;
        assert_eq!(dto.partitions.len(), 1);
        assert_eq!(dto.partitions[0].name, "m_2024");
        assert_eq!(dto.partitions[0].row_estimate, Some(42.0));
        Ok(())
    }
}
