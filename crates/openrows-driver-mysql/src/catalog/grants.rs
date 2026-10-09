// SPDX-License-Identifier: Apache-2.0
//! Bound explicit privileges, supplementing `MySQL`'s missing dynamic privileges.
use super::{bind, optional, text};
use crate::session::MySqlSession;
use openrows_driver_api::inspection::GrantSummary;
use openrows_driver_api::{Connection, DriverError};
use openrows_sql::{Dialect, ddl::string_literal};

/// Look up the listed account before constructing any server account spelling.
pub(crate) async fn grants(
    session: &mut MySqlSession,
    (user, host): (&str, &str),
) -> Result<Vec<GrantSummary>, DriverError> {
    let accounts = session
        .text_query(
            "SELECT User, Host FROM mysql.user WHERE User = ? AND Host = ?",
            &bind(&[user, host]),
        )
        .await?;
    let account = accounts.first().ok_or_else(|| DriverError::Server {
        code: "42704".to_owned(),
        message: "Account no longer exists or is not visible".to_owned(),
    })?;
    let account = account_spelling(&text(account, 0), &text(account, 1))?;
    let mut grants = session
        .text_query(
            include_str!("sql/grants.sql"),
            &bind(&[&account, &account, &account, &account]),
        )
        .await?
        .iter()
        .map(|row| grant(row))
        .collect::<Result<Vec<_>, _>>()?;
    // MySQL 8+ dynamic privileges are absent from USER_PRIVILEGES. Only these
    // missing global entries use SHOW; memberships stay in their own section.
    if session.server().product == "MySQL"
        && session
            .server()
            .version
            .split('.')
            .next()
            .and_then(|major| major.parse::<u16>().ok())
            .is_some_and(|major| major >= 8)
    {
        supplement(session, &account, &mut grants).await?;
    }
    grants.sort_by(|a, b| {
        (&a.scope, &a.object, &a.privilege).cmp(&(&b.scope, &b.object, &b.privilege))
    });
    Ok(grants)
}

async fn supplement(
    session: &mut MySqlSession,
    account: &str,
    grants: &mut Vec<GrantSummary>,
) -> Result<(), DriverError> {
    let rows = session.text_query(&show_sql(account), &[]).await?;
    for line in rows.iter().map(|row| text(row, 0)) {
        for grant in super::grant_parser::parse(&line)? {
            let present = grants.iter().any(|existing| {
                existing.scope == grant.scope && existing.privilege == grant.privilege
            });
            if grant.scope == "global"
                && !present
                && !matches!(grant.privilege.as_str(), "USAGE" | "ALL PRIVILEGES")
            {
                grants.push(grant);
            }
        }
    }
    Ok(())
}

fn grant(row: &[Option<String>]) -> Result<GrantSummary, DriverError> {
    let schema = optional(row, 1);
    let object = grant_object(
        schema.as_deref(),
        optional(row, 2).as_deref(),
        optional(row, 3).as_deref(),
    )?;
    Ok(GrantSummary {
        scope: text(row, 0),
        object,
        privilege: text(row, 4),
        state: if text(row, 5) == "YES" {
            "GRANT WITH GRANT OPTION"
        } else {
            "GRANT"
        }
        .to_owned(),
        grantor: None,
    })
}

fn grant_object(
    schema: Option<&str>,
    table: Option<&str>,
    column: Option<&str>,
) -> Result<String, DriverError> {
    let dialect = Dialect::MySql;
    let result = match (schema, table, column) {
        (Some(schema), Some(table), Some(column)) => {
            dialect.qualified(schema, table).and_then(|table| {
                dialect
                    .quote_ident(column)
                    .map(|column| format!("{table}.{column}"))
            })
        }
        (Some(schema), Some(table), None) => dialect.qualified(schema, table),
        (Some(schema), None, None) => dialect
            .quote_ident(schema)
            .map(|schema| format!("{schema}.*")),
        _ => return Ok("*.*".to_owned()),
    };
    result.map_err(|error| DriverError::Protocol(error.to_string()))
}

fn account_spelling(user: &str, host: &str) -> Result<String, DriverError> {
    let literal = |name| {
        string_literal(Dialect::MySql, name)
            .ok_or_else(|| DriverError::Protocol("NUL in catalog account".to_owned()))
    };
    Ok(format!("{}@{}", literal(user)?, literal(host)?))
}

fn show_sql(account: &str) -> String {
    format!("SHOW GRANTS FOR {account}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_account_spelling_escapes_quotes_backslashes_and_unicode() -> Result<(), DriverError> {
        let account = account_spelling("u'\\é", "h'\\සි")?;
        assert_eq!(account, "'u''\\\\é'@'h''\\\\සි'");
        assert_eq!(show_sql(&account), "SHOW GRANTS FOR 'u''\\\\é'@'h''\\\\සි'");
        assert!(account_spelling("a\0", "%").is_err());
        Ok(())
    }

    #[test]
    fn grant_objects_use_dialect_helpers_for_every_identifier() -> Result<(), DriverError> {
        let name = "a`\\é";
        assert_eq!(grant_object(Some(name), None, None)?, "`a``\\é`.*");
        assert_eq!(
            grant_object(Some(name), Some(name), None)?,
            "`a``\\é`.`a``\\é`"
        );
        assert_eq!(
            grant_object(Some(name), Some(name), Some(name))?,
            "`a``\\é`.`a``\\é`.`a``\\é`"
        );
        Ok(())
    }
}
