// SPDX-License-Identifier: Apache-2.0
//! Bound routine metadata and existing view definition reads.
use super::bind;
use crate::session::MySqlSession;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::inspection::{
    ObjectDetail, ObjectIdentity, attributes, section, view_detail,
};
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::{Catalog, DriverError};

/// Inspect a view or stored routine without executing its definition.
pub(crate) async fn object_detail(
    session: &mut MySqlSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    match object.kind {
        ObjectKind::View => view_detail(session.table_detail(&object.schema, &object.name).await?),
        ObjectKind::Function | ObjectKind::Procedure => routine(session, object).await,
        ObjectKind::Sequence => super::sequence::sequence(session, object).await,
        ObjectKind::Type
        | ObjectKind::Table
        | ObjectKind::ForeignTable
        | ObjectKind::MaterializedView => {
            Err(DriverError::Unsupported("this object viewer".to_owned()))
        }
    }
}

async fn routine(
    session: &mut MySqlSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    let kind = if object.kind == ObjectKind::Procedure {
        RoutineKind::Procedure
    } else {
        RoutineKind::Function
    };
    let rows = session
        .text_query(
            include_str!("sql/inspect-routine.sql"),
            &bind(&[&object.schema, &object.name, kind.keyword()]),
        )
        .await?;
    let row = rows.first().ok_or_else(|| DriverError::Server {
        code: "42704".to_owned(),
        message: "Routine no longer exists or is not visible".to_owned(),
    })?;
    let args = session
        .text_query(
            include_str!("sql/inspect-arguments.sql"),
            &bind(&[&object.schema, &object.name]),
        )
        .await?;
    let mut fields = attributes(
        &[
            "Kind",
            "Return type",
            "Language",
            "Deterministic",
            "SQL data access",
            "Security",
            "Comment",
        ],
        row,
    );
    let arguments = args
        .iter()
        .map(|row| row.iter().flatten().cloned().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(", ");
    fields.extend(attributes(
        &["Signature"],
        &[Some(format!(
            "{}.{}({arguments})",
            object.schema, object.name
        ))],
    ));
    Ok(ObjectDetail {
        attributes: fields,
        sections: vec![section("Arguments", &["Name", "Mode", "Type"], args)],
        definition: session
            .routine_definition((&object.schema, &object.name), kind, None)
            .await?,
    })
}
