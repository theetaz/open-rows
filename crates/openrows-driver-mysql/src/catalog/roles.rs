// SPDX-License-Identifier: Apache-2.0
//! Public account attributes and direct memberships, never password hashes.
use super::{bind, optional, text};
use crate::session::MySqlSession;
use openrows_driver_api::inspection::{RoleSummary, attributes};
use openrows_driver_api::{Connection, DriverError};

/// Accounts scoped by host, with public attributes and role memberships.
pub(crate) async fn roles(session: &mut MySqlSession) -> Result<Vec<RoleSummary>, DriverError> {
    let sql = if session.server().product == "MariaDB" {
        include_str!("sql/roles-mariadb.sql")
    } else {
        include_str!("sql/roles-mysql.sql")
    };
    let mut roles = Vec::new();
    for row in session.text_query(sql, &[]).await? {
        roles.push(role(session, &row).await?);
    }
    Ok(roles)
}

async fn role(
    session: &mut MySqlSession,
    row: &[Option<String>],
) -> Result<RoleSummary, DriverError> {
    let (name, scope) = (text(row, 0), text(row, 1));
    let maria = session.server().product == "MariaDB";
    let mut fields = attributes(&["Account locked", "Password expired"], &row[2..]);
    if maria {
        fields.extend(attributes(&["Role"], &[optional(row, 4)]));
    }
    let sql = if maria {
        include_str!("sql/memberships-mariadb.sql")
    } else {
        include_str!("sql/memberships-mysql.sql")
    };
    let memberships = match session.text_query(sql, &bind(&[&name, &scope])).await {
        Ok(rows) => rows.iter().map(|row| text(row, 0)).collect(),
        Err(error) if membership_denied(&error) => {
            fields.extend(attributes(
                &["Membership visibility"],
                &[Some("Not visible to this login".to_owned())],
            ));
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    Ok(RoleSummary {
        name,
        scope,
        attributes: fields,
        memberships,
    })
}

fn membership_denied(error: &DriverError) -> bool {
    matches!(error, DriverError::Server { message, .. }
        if [1142, 1143, 1227].iter().any(|code|
            message.ends_with(&format!("(MySQL error {code})"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_permission_errors_hide_memberships() {
        for code in [1142, 1143, 1227] {
            let error = DriverError::Server {
                code: "42000".into(),
                message: format!("denied (MySQL error {code})"),
            };
            assert!(membership_denied(&error));
        }
        assert!(!membership_denied(&DriverError::Protocol("broken".into())));
    }
}
