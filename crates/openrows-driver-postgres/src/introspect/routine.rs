// SPDX-License-Identifier: Apache-2.0
//! A function's or procedure's definition for the routine editor.
use super::{literal, optional};
use crate::session::PgSession;
use openrows_driver_api::DriverError;
use openrows_driver_api::objects::RoutineKind;

/// `pg_get_functiondef` of one overload; `{arguments}` is `NULL` for any.
const ROUTINE_SQL: &str = "SELECT pg_get_functiondef(p.oid) FROM pg_catalog.pg_proc p \
    JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
    WHERE n.nspname = {schema} AND p.proname = {name} AND p.prokind IN ({kinds}) \
    AND ({arguments}::text IS NULL OR pg_catalog.pg_get_function_identity_arguments(p.oid) = {arguments}) \
    ORDER BY p.oid LIMIT 1";

impl PgSession {
    /// `CREATE OR REPLACE FUNCTION|PROCEDURE …` of `schema`.`name` with the
    /// identity `arguments` (any overload when `None`); `None` when missing.
    ///
    /// # Errors
    /// `Unsupported` for names containing NUL; transport, protocol or server
    /// failures.
    pub async fn routine_definition(
        &mut self,
        (schema, name): (&str, &str),
        kind: RoutineKind,
        arguments: Option<&str>,
    ) -> Result<Option<String>, DriverError> {
        let rows = self
            .text_query(&routine_sql((schema, name), kind, arguments)?)
            .await?;
        Ok(rows.first().and_then(|row| optional(row, 0)))
    }
}

fn routine_sql(
    (schema, name): (&str, &str),
    kind: RoutineKind,
    arguments: Option<&str>,
) -> Result<String, DriverError> {
    let kinds = match kind {
        RoutineKind::Function => "'f', 'w'",
        RoutineKind::Procedure => "'p'",
    };
    let arguments = arguments.map_or_else(|| Ok("NULL".to_owned()), literal)?;
    Ok(ROUTINE_SQL
        .replace("{schema}", &literal(schema)?)
        .replace("{name}", &literal(name)?)
        .replace("{kinds}", kinds)
        .replace("{arguments}", &arguments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routine_lookups_quote_their_values() -> Result<(), DriverError> {
        let sql = routine_sql(("app", "it's"), RoutineKind::Procedure, Some("integer"))?;
        assert!(sql.contains("p.proname = 'it''s'"));
        assert!(sql.contains("IN ('p')"));
        assert!(sql.contains("= 'integer'"));
        let any = routine_sql(("app", "f"), RoutineKind::Function, None)?;
        assert!(any.contains("NULL::text IS NULL"));
        Ok(())
    }
}
