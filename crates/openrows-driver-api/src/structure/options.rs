// SPDX-License-Identifier: Apache-2.0
//! Dialect-aware validation and rendering of structure options.
use super::{AlterError, StagedChanges};
use crate::schema::NewKey;
use openrows_sql::Dialect;

/// Refuse options the engine cannot execute, including mixed concurrent DDL.
/// # Errors
/// Unsupported options, invalid expressions or non-atomic concurrent batches.
pub fn validate_options(
    staged: &StagedChanges<'_>,
    dialect: Dialect,
    count: usize,
) -> Result<(), AlterError> {
    if !staged.concurrent_indexes.is_empty() && (dialect != Dialect::Postgres || count != 1) {
        return Err(AlterError::Unsupported("A concurrent index must be staged alone on PostgreSQL; it cannot be rolled back with other changes.".into()));
    }
    if !staged.reorders.is_empty() && dialect != Dialect::MySql {
        return Err(AlterError::Unsupported("Column reordering requires rebuilding the table; dependent objects and row identity cannot be preserved safely here.".into()));
    }
    for edits in staged.columns.values() {
        if let Some(expression) = edits.using_expression {
            if dialect != Dialect::Postgres {
                return Err(AlterError::Unsupported(
                    "USING conversions are supported only on PostgreSQL.".into(),
                ));
            }
            if !openrows_sql::ddl::valid_using_expression(expression) {
                return Err(AlterError::InvalidExpression(expression.into()));
            }
        }
    }
    Ok(())
}

/// Render a named primary or unique key with quoted column identifiers.
/// # Errors
/// Invalid identifiers.
pub fn key_sql(key: &NewKey, primary: bool, dialect: Dialect) -> Result<String, AlterError> {
    let quote = |name: &str| {
        dialect
            .quote_ident(name)
            .map_err(|_| AlterError::InvalidName)
    };
    let named = key
        .name
        .as_deref()
        .map(|n| quote(n).map(|q| format!("CONSTRAINT {q} ")))
        .transpose()?
        .unwrap_or_default();
    let columns = key
        .columns
        .iter()
        .map(|c| quote(c))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let kind = if primary { "PRIMARY KEY" } else { "UNIQUE" };
    Ok(format!("{named}{kind} ({columns})"))
}

/// Execute a validated plan on a fresh, idle connection.
/// # Errors
/// The original statement or commit error; autocommit changes may remain applied.
pub async fn execute_plan(
    connection: &mut dyn crate::Connection,
    plan: &super::AlterPlan,
) -> Result<(), crate::DriverError> {
    use super::AlterRun;
    match plan.run {
        AlterRun::Transaction => {
            return crate::editor(connection)?
                .commit_ddl(&plan.statements)
                .await;
        }
        AlterRun::StrictMySql => super::strict_mode::require(connection).await?,
        AlterRun::Autocommit => {}
    }
    for sql in &plan.statements {
        connection
            .execute(sql, &mut crate::DiscardSink)
            .await
            .map_err(autocommit_error)?;
    }
    Ok(())
}

fn autocommit_error(error: crate::DriverError) -> crate::DriverError {
    if matches!(error, crate::DriverError::Network(_)) {
        crate::DriverError::Server {
            code: "08007".into(),
            message: "The DDL connection was lost. The statement may already be applied; reload the structure and inspect it before editing again.".into(),
        }
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DriverError;

    #[test]
    fn a_lost_autocommit_acknowledgement_requires_reload() {
        let network = DriverError::Network(std::io::ErrorKind::BrokenPipe.into());
        assert!(
            matches!(autocommit_error(network), DriverError::Server { code, .. } if code == "08007")
        );
        let violation = DriverError::Server {
            code: "23505".into(),
            message: "duplicate".into(),
        };
        assert!(
            matches!(autocommit_error(violation), DriverError::Server { code, .. } if code == "23505")
        );
    }
}
