// SPDX-License-Identifier: Apache-2.0
//! Read-only schema object viewers, with bound names.
use super::rows::{bind, format_type, optional, text};
use crate::session::SqlServerSession;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity, attributes, section};
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::{Catalog, DriverError};

/// Inspect a supported kind without executing its definition.
pub(crate) async fn object_detail(
    session: &mut SqlServerSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    match object.kind {
        ObjectKind::View => view(session, object).await,
        ObjectKind::Function | ObjectKind::Procedure => routine(session, object).await,
        ObjectKind::Sequence => sequence(session, object).await,
        ObjectKind::Type => user_type(session, object).await,
        ObjectKind::Table | ObjectKind::ForeignTable | ObjectKind::MaterializedView => {
            Err(DriverError::Unsupported("this object viewer".to_owned()))
        }
    }
}

async fn view(
    session: &mut SqlServerSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let rows = session
        .text_query(
            include_str!("sql/inspect-view.sql"),
            &bind(&[&object.schema, &object.name]),
        )
        .await?;
    let row = rows.first().ok_or_else(missing)?;
    let columns = session.table_columns(&object.schema, &object.name).await?;
    Ok(ObjectDetail {
        definition: optional(row, 0),
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
        attributes: Vec::new(),
    })
}

async fn routine(
    session: &mut SqlServerSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let params = bind(&[&object.schema, &object.name]);
    let rows = session
        .text_query(include_str!("sql/inspect-routine.sql"), &params)
        .await?;
    let row = rows.first().ok_or_else(missing)?;
    let args = session
        .text_query(include_str!("sql/inspect-arguments.sql"), &params)
        .await?;
    let kind = if object.kind == ObjectKind::Procedure {
        RoutineKind::Procedure
    } else {
        RoutineKind::Function
    };
    let returns = args
        .iter()
        .find(|row| text(row, 8) == "0")
        .map(|row| Some(format_type(row, 1)));
    let mut fields = attributes(&["Kind", "Language", "Execute as"], row);
    fields.extend(attributes(&["Return type"], &[returns.flatten()]));
    fields.extend(attributes(
        &["Signature"],
        &[Some(signature(object, &args))],
    ));
    Ok(ObjectDetail {
        attributes: fields,
        sections: vec![section(
            "Arguments",
            &["Name", "Type", "Mode", "Read only"],
            args.iter()
                .filter(|row| text(row, 8) != "0")
                .map(|row| {
                    vec![
                        optional(row, 0),
                        Some(format_type(row, 1)),
                        optional(row, 6),
                        optional(row, 7),
                    ]
                })
                .collect(),
        )],
        definition: session
            .routine_definition((&object.schema, &object.name), kind, None)
            .await?,
    })
}

fn signature(object: &ObjectIdentity, args: &[Vec<Option<String>>]) -> String {
    let arguments = args
        .iter()
        .filter(|row| text(row, 8) != "0")
        .map(|row| format!("{} {}", text(row, 0), format_type(row, 1)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}.{}({arguments})", object.schema, object.name)
}

async fn sequence(
    session: &mut SqlServerSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let rows = session
        .text_query(
            include_str!("sql/inspect-sequence.sql"),
            &bind(&[&object.schema, &object.name]),
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
                "Cached",
            ],
            row,
        ),
        ..ObjectDetail::default()
    })
}

async fn user_type(
    session: &mut SqlServerSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let params = bind(&[&object.schema, &object.name]);
    let rows = session
        .text_query(include_str!("sql/inspect-type.sql"), &params)
        .await?;
    let row = rows.first().ok_or_else(missing)?;
    let fields = session
        .text_query(include_str!("sql/inspect-type-fields.sql"), &params)
        .await?;
    let base_type = optional(row, 2).map(|_| {
        format_type(
            &[
                optional(row, 2),
                optional(row, 2),
                optional(row, 3),
                optional(row, 4),
                optional(row, 5),
            ],
            0,
        )
    });
    Ok(ObjectDetail {
        attributes: attributes(
            &["Kind", "Base type", "Nullable", "Collation"],
            &[
                optional(row, 0),
                base_type,
                optional(row, 6),
                optional(row, 7),
            ],
        ),
        sections: vec![section(
            "Fields",
            &["Name", "Type", "Nullable"],
            fields
                .iter()
                .map(|row| {
                    vec![
                        optional(row, 0),
                        Some(format_type(row, 1)),
                        optional(row, 6),
                    ]
                })
                .collect(),
        )],
        definition: None,
    })
}

fn missing() -> DriverError {
    DriverError::Server {
        code: "42704".to_owned(),
        message: "Object no longer exists or is not visible".to_owned(),
    }
}
