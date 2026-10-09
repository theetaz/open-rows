// SPDX-License-Identifier: Apache-2.0
//! Server logins, database principals, memberships and explicit permissions.
use super::rows::{bind, optional, text};
use crate::session::SqlServerSession;
use openrows_driver_api::DriverError;
use openrows_driver_api::inspection::{GrantSummary, RoleSummary, attributes};

/// Public account attributes and direct role memberships in both scopes.
pub(crate) async fn roles(session: &mut SqlServerSession) -> Result<Vec<RoleSummary>, DriverError> {
    let rows = session
        .text_query(include_str!("sql/roles.sql"), &[])
        .await?;
    let members = session
        .text_query(include_str!("sql/memberships.sql"), &[])
        .await?;
    Ok(rows
        .iter()
        .map(|row| {
            let (name, scope) = (text(row, 0), text(row, 1));
            let labels = if scope == "server" {
                ["Kind", "Disabled", "Default database", "Password expired"]
            } else {
                ["Kind", "Disabled", "Default schema", "Password expired"]
            };
            let memberships = members
                .iter()
                .filter(|member| text(member, 0) == name && text(member, 1) == scope)
                .map(|member| text(member, 2))
                .collect();
            RoleSummary {
                name,
                scope,
                attributes: attributes(&labels, &row[2..]),
                memberships,
            }
        })
        .collect())
}

/// Direct permissions, including explicit DENY and grant options.
pub(crate) async fn grants(
    session: &mut SqlServerSession,
    (name, scope): (&str, &str),
) -> Result<Vec<GrantSummary>, DriverError> {
    if !matches!(scope, "server" | "database") {
        return Err(DriverError::Unsupported(
            "invalid principal scope".to_owned(),
        ));
    }
    Ok(session
        .text_query(include_str!("sql/grants.sql"), &bind(&[name, scope]))
        .await?
        .iter()
        .map(|row| GrantSummary {
            scope: text(row, 0),
            object: text(row, 1),
            privilege: text(row, 2),
            state: text(row, 3),
            grantor: optional(row, 4),
        })
        .collect())
}
