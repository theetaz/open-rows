// SPDX-License-Identifier: Apache-2.0
//! SHOW GRANTS fallback rows in the shared explicit-permission shape.
use super::grant_tokens::{groups, invalid, tokens};
use openrows_driver_api::DriverError;
use openrows_driver_api::inspection::GrantSummary;

/// Parse one server GRANT line; quoted names cannot become keywords.
pub(super) fn parse(line: &str) -> Result<Vec<GrantSummary>, DriverError> {
    let tokens = tokens(line)?;
    if tokens.first() != Some(&"GRANT") {
        return Err(invalid());
    }
    let to = tokens
        .iter()
        .position(|token| *token == "TO")
        .ok_or_else(invalid)?;
    let state = if tokens[to..]
        .windows(3)
        .any(|words| words == ["WITH", "GRANT", "OPTION"])
    {
        "GRANT WITH GRANT OPTION"
    } else if tokens[to..]
        .windows(3)
        .any(|words| words == ["WITH", "ADMIN", "OPTION"])
    {
        "GRANT WITH ADMIN OPTION"
    } else {
        "GRANT"
    };
    let Some(on) = tokens[..to].iter().position(|token| *token == "ON") else {
        return groups(&tokens[1..to])?
            .into_iter()
            .map(|role| Ok(row("role", &role.concat(), "MEMBER", state)))
            .collect();
    };
    let target = &tokens[on + 1..to];
    let scope = scope(target)?;
    let object = target.concat();
    let mut rows = Vec::new();
    for privilege in groups(&tokens[1..on])? {
        rows.extend(permission(privilege, scope, &object, state)?);
    }
    Ok(rows)
}

fn scope(target: &[&str]) -> Result<&'static str, DriverError> {
    let dots = target.iter().filter(|token| **token == ".").count();
    if target == ["*", ".", "*"] {
        Ok("global")
    } else if dots == 1 && target.last() == Some(&"*") {
        Ok("schema")
    } else if dots == 1 {
        Ok("table")
    } else if target.contains(&"@") {
        Ok("proxy")
    } else {
        Err(invalid())
    }
}

fn permission(
    tokens: &[&str],
    scope: &str,
    object: &str,
    state: &str,
) -> Result<Vec<GrantSummary>, DriverError> {
    if tokens.is_empty() {
        return Err(invalid());
    }
    let Some(open) = tokens.iter().position(|token| *token == "(") else {
        return Ok(vec![row(scope, object, &tokens.join(" "), state)]);
    };
    if tokens.last() != Some(&")") || open == 0 {
        return Err(invalid());
    }
    let privilege = tokens[..open].join(" ");
    groups(&tokens[open + 1..tokens.len() - 1])?
        .into_iter()
        .map(|column| {
            if column.len() != 1 {
                return Err(invalid());
            }
            Ok(row(
                "column",
                &format!("{object}.{}", column[0]),
                &privilege,
                state,
            ))
        })
        .collect()
}

fn row(scope: &str, object: &str, privilege: &str, state: &str) -> GrantSummary {
    GrantSummary {
        scope: scope.to_owned(),
        object: object.to_owned(),
        privilege: privilege.to_owned(),
        state: state.to_owned(),
        grantor: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_driven_grants_cover_options_columns_and_roles() -> Result<(), DriverError> {
        let cases = [
            (
                "GRANT SELECT, BACKUP_ADMIN ON *.* TO 'u'@'%' WITH GRANT OPTION",
                vec![
                    ("global", "*.*", "SELECT", "GRANT WITH GRANT OPTION"),
                    ("global", "*.*", "BACKUP_ADMIN", "GRANT WITH GRANT OPTION"),
                ],
            ),
            (
                "GRANT CREATE ON `db`.* TO 'u'@'%'",
                vec![("schema", "`db`.*", "CREATE", "GRANT")],
            ),
            (
                "GRANT SELECT (`a``\\සි`, `b`), UPDATE (`b`) ON `db`.`t` TO 'u'@'%' WITH GRANT OPTION",
                vec![
                    (
                        "column",
                        "`db`.`t`.`a``\\සි`",
                        "SELECT",
                        "GRANT WITH GRANT OPTION",
                    ),
                    (
                        "column",
                        "`db`.`t`.`b`",
                        "SELECT",
                        "GRANT WITH GRANT OPTION",
                    ),
                    (
                        "column",
                        "`db`.`t`.`b`",
                        "UPDATE",
                        "GRANT WITH GRANT OPTION",
                    ),
                ],
            ),
            (
                "GRANT 'r''\\\\é'@'%', 'second'@'localhost' TO 'u'@'%'",
                vec![
                    ("role", "'r''\\\\é'@'%'", "MEMBER", "GRANT"),
                    ("role", "'second'@'localhost'", "MEMBER", "GRANT"),
                ],
            ),
            (
                "GRANT `parent` TO 'u'@'%' WITH ADMIN OPTION",
                vec![("role", "`parent`", "MEMBER", "GRANT WITH ADMIN OPTION")],
            ),
            (
                "GRANT INSERT ON `db`.`t` TO 'u'@'%'",
                vec![("table", "`db`.`t`", "INSERT", "GRANT")],
            ),
        ];
        for (line, expected) in cases {
            let rows = parse(line)?;
            let actual: Vec<_> = rows
                .iter()
                .map(|row| {
                    (
                        row.scope.as_str(),
                        row.object.as_str(),
                        row.privilege.as_str(),
                        row.state.as_str(),
                    )
                })
                .collect();
            assert_eq!(actual, expected, "{line}");
        }
        Ok(())
    }

    #[test]
    fn malformed_grants_fail_as_typed_protocol_errors() {
        for line in [
            "GRANT",
            "GRANT SELECT ON `broken TO u",
            "GRANT SELECT (a ON db.t TO u",
        ] {
            assert!(matches!(parse(line), Err(DriverError::Protocol(_))));
        }
    }
}
