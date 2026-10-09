// SPDX-License-Identifier: Apache-2.0
//! Read-only viewers for `PostgreSQL` schema objects.
use super::{bound, optional};
use crate::session::PgSession;
use openrows_driver_api::DriverError;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity, attributes, section};
use openrows_driver_api::objects::RoutineKind;

/// Inspect a schema object without executing its definition.
pub(crate) async fn object_detail(
    session: &mut PgSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    match object.kind {
        ObjectKind::View | ObjectKind::MaterializedView => view(session, object).await,
        ObjectKind::Function | ObjectKind::Procedure => routine(session, object).await,
        ObjectKind::Sequence => sequence(session, object).await,
        ObjectKind::Type => user_type(session, object).await,
        ObjectKind::Table | ObjectKind::ForeignTable => Err(DriverError::Unsupported(
            "table inspection uses table_detail".to_owned(),
        )),
    }
}

async fn view(
    session: &mut PgSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let rows = bound::query(
        session,
        include_str!("sql/inspect-view.sql"),
        &bound::bind(&[&object.schema, &object.name]),
    )
    .await?;
    let row = rows.first().ok_or_else(missing)?;
    let columns = session.table_columns(&object.schema, &object.name).await?;
    let attributes = if object.kind == ObjectKind::MaterializedView {
        attributes(&["Populated"], &row[1..2])
    } else {
        Vec::new()
    };
    Ok(ObjectDetail {
        attributes,
        definition: optional(row, 3),
        sections: vec![section(
            "Columns",
            &["Name", "Type", "Nullable", "Default", "Comment"],
            columns
                .into_iter()
                .map(|column| {
                    vec![
                        Some(column.name),
                        Some(column.data_type),
                        Some(column.nullable.to_string()),
                        column.default,
                        column.comment,
                    ]
                })
                .collect(),
        )],
    })
}

async fn routine(
    session: &mut PgSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let kind = if object.kind == ObjectKind::Procedure {
        RoutineKind::Procedure
    } else {
        RoutineKind::Function
    };
    let code = if kind == RoutineKind::Procedure {
        "p"
    } else {
        "f"
    };
    let mut params = bound::bind(&[&object.schema, &object.name, code]);
    params.push(object.arguments.clone());
    let rows = bound::query(session, include_str!("sql/inspect-routine.sql"), &params).await?;
    let row = rows.first().ok_or_else(missing)?;
    Ok(ObjectDetail {
        attributes: attributes(
            &[
                "Signature",
                "Arguments",
                "Return type",
                "Language",
                "Volatility",
                "Security definer",
            ],
            row,
        ),
        definition: session
            .routine_definition(
                (&object.schema, &object.name),
                kind,
                object.arguments.as_deref(),
            )
            .await?,
        sections: Vec::new(),
    })
}

async fn sequence(
    session: &mut PgSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let rows = bound::query(
        session,
        include_str!("sql/inspect-sequence.sql"),
        &bound::bind(&[&object.schema, &object.name]),
    )
    .await?;
    let row = rows.first().ok_or_else(missing)?;
    Ok(ObjectDetail {
        attributes: attributes(
            &[
                "Data type",
                "Start",
                "Increment",
                "Minimum",
                "Maximum",
                "Cycle",
                "Cache",
                "Last value",
            ],
            row,
        ),
        ..ObjectDetail::default()
    })
}

async fn user_type(
    session: &mut PgSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let params = bound::bind(&[&object.schema, &object.name]);
    let rows = bound::query(session, include_str!("sql/inspect-type.sql"), &params).await?;
    let row = rows.first().ok_or_else(missing)?;
    let items = bound::query(session, include_str!("sql/inspect-type-items.sql"), &params).await?;
    let sections = ["Enum labels", "Composite fields", "Domain constraints"]
        .into_iter()
        .map(|name| {
            section(
                name,
                &["Name", "Type / definition"],
                items
                    .iter()
                    .filter(|item| optional(item, 0).as_deref() == Some(name))
                    .map(|item| item[1..].to_vec())
                    .collect(),
            )
        })
        .filter(|section| !section.rows.is_empty())
        .collect();
    Ok(ObjectDetail {
        attributes: attributes(
            &[
                "Kind",
                "Base type",
                "Not null",
                "Default",
                "Range subtype",
                "Comment",
            ],
            row,
        ),
        sections,
        definition: None,
    })
}

fn missing() -> DriverError {
    DriverError::Server {
        code: "42704".to_owned(),
        message: "Object no longer exists or is not visible".to_owned(),
    }
}
