// SPDX-License-Identifier: Apache-2.0
//! Public extension, account, membership and grant catalog reads.
use super::{bound, optional, required};
use crate::session::PgSession;
use openrows_driver_api::DriverError;
use openrows_driver_api::inspection::{ExtensionSummary, GrantSummary, RoleSummary, attributes};

/// Installed extensions, joined to the server's available default version.
pub(crate) async fn extensions(
    session: &mut PgSession,
) -> Result<Vec<ExtensionSummary>, DriverError> {
    bound::query(session, include_str!("sql/extensions.sql"), &[])
        .await?
        .iter()
        .map(|row| {
            Ok(ExtensionSummary {
                name: required(row, 0)?,
                version: required(row, 1)?,
                available_version: optional(row, 2),
                schema: required(row, 3)?,
                description: optional(row, 4),
            })
        })
        .collect()
}

/// Public role attributes and direct memberships, with no credential data.
pub(crate) async fn roles(session: &mut PgSession) -> Result<Vec<RoleSummary>, DriverError> {
    let rows = bound::query(session, include_str!("sql/roles.sql"), &[]).await?;
    let members = bound::query(session, include_str!("sql/memberships.sql"), &[]).await?;
    rows.iter()
        .map(|row| {
            let name = required(row, 0)?;
            let memberships = members
                .iter()
                .filter(|member| optional(member, 0).as_deref() == Some(&name))
                .map(|member| required(member, 1))
                .collect::<Result<_, _>>()?;
            Ok(RoleSummary {
                name,
                scope: "cluster".to_owned(),
                attributes: attributes(
                    &[
                        "Login",
                        "Superuser",
                        "Create database",
                        "Create role",
                        "Inherit",
                        "Replication",
                        "Bypass RLS",
                        "Connection limit",
                        "Valid until",
                    ],
                    &row[1..],
                ),
                memberships,
            })
        })
        .collect()
}

/// Explicit ACL privileges in the connected database.
pub(crate) async fn grants(
    session: &mut PgSession,
    (name, scope): (&str, &str),
) -> Result<Vec<GrantSummary>, DriverError> {
    if scope != "cluster" {
        return Err(DriverError::Unsupported(
            "non-cluster PostgreSQL account".to_owned(),
        ));
    }
    bound::query(
        session,
        include_str!("sql/grants.sql"),
        &bound::bind(&[name]),
    )
    .await?
    .iter()
    .map(|row| {
        Ok(GrantSummary {
            scope: required(row, 0)?,
            object: required(row, 1)?,
            privilege: required(row, 2)?,
            state: required(row, 3)?,
            grantor: optional(row, 4),
        })
    })
    .collect()
}
