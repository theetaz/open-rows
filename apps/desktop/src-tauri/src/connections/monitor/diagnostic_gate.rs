// SPDX-License-Identifier: Apache-2.0
//! Immutable SQL/context/mode confirmation plus the existing native safety boundary.
use super::diagnostic::MonitorExplain;
use crate::connections::IpcError;
use crate::queries::explain_dto::ExplainReply;
use openrows_core::{
    explain::{Admission, admit, confirmation_token},
    safety::SafetyPolicy,
};
use openrows_explain::{Engine, Mode};
use openrows_store::profile::ConnectionProfile;

/// Admit one bounded, complete reviewed query or DML statement.
pub(super) fn statement(request: &MonitorExplain) -> Result<String, IpcError> {
    let dialect = openrows_sql::Dialect::Postgres;
    if request.sql.len() > 16_384
        || request.schema.is_empty()
        || request.schema.len() > 63
        || request.schema.contains('\0')
        || request.explain_id.len() > 64
        || request.statement_id.len() > 256
        || request.target.database.len() > 63
        || request.target.profile_id.len() > 64
    {
        return Err(IpcError::invalid("Diagnostic SQL/context exceeds limits"));
    }
    crate::queries::history_privacy::validate_reuse(&request.sql)?;
    if !openrows_sql::placeholders(&request.sql, dialect).is_empty() {
        return Err(IpcError::invalid(
            "Supply complete SQL and every redacted binding before Explain",
        ));
    }
    let statements =
        openrows_core::script::statements(&request.sql, openrows_core::script::Scope::All, dialect);
    if statements.len() != 1 {
        return Err(IpcError::invalid("Review exactly one complete statement"));
    }
    let sql = statements
        .into_iter()
        .next()
        .ok_or_else(|| IpcError::invalid("Missing statement"))?;
    let class = openrows_sql::classify(&sql, dialect);
    if !openrows_sql::postgres_explainable(&sql)
        || !matches!(
            class.kind,
            openrows_sql::StatementKind::Read | openrows_sql::StatementKind::Write
        )
    {
        return Err(IpcError::invalid(
            "Explain supports a query or DML statement only",
        ));
    }
    Ok(sql)
}
/// Bind required consent to the exact SQL, policy and diagnostic context.
pub(super) fn gate(
    request: &MonitorExplain,
    profile: &ConnectionProfile,
    generation: &str,
    sql: &str,
) -> Option<ExplainReply> {
    let policy = crate::queries::commands::policy(profile);
    let mode = if request.analyze {
        Mode::Analyze
    } else {
        Mode::Plan
    };
    match admit(policy, Engine::Postgres, mode, sql) {
        Admission::Run => None,
        Admission::Block(message) | Admission::Refuse(message) => {
            Some(ExplainReply::Blocked { message })
        }
        Admission::Confirm => {
            let scope = format!(
                "{:?}",
                (
                    profile.id,
                    &request.target.database,
                    &request.schema,
                    generation,
                    request.analyze,
                    &profile.env_tag
                )
            );
            Some(confirmed(policy, &scope, sql))
        }
    }
}
fn confirmed(policy: SafetyPolicy, scope: &str, sql: &str) -> ExplainReply {
    let token = confirmation_token(policy, scope, sql);
    ExplainReply::ConfirmationRequired {
        token,
        statement: openrows_sql::normalize_monitor_query(sql)
            .unwrap_or_else(|| "Reviewed SQL (text withheld)".into()),
    }
}

/// Fixed disposable-session context never disables the server's read-only default.
pub(super) fn settings(schema: &str, read_only: bool) -> Vec<String> {
    let schema = schema.replace('"', "\"\"");
    let mut settings = vec![
        format!("SET search_path = pg_catalog, \"{schema}\""),
        "SET standard_conforming_strings = on".into(),
        "SET statement_timeout = 15000".into(),
        "SET lock_timeout = 1000".into(),
    ];
    if read_only {
        settings.push("SET default_transaction_read_only = on".into());
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_store::profile::{EnvKind, EnvTag, ServerParams};
    use openrows_types::safety::Safety;
    fn request() -> MonitorExplain {
        MonitorExplain {
            target: super::super::MonitorTarget {
                profile_id: "p".into(),
                database: "db".into(),
            },
            statement_id: "q".into(),
            explain_id: "e".into(),
            sql: "SELECT 1".into(),
            schema: "public".into(),
            analyze: false,
            confirm_token: None,
        }
    }
    #[test]
    fn plan_default_and_every_measured_read_confirm_with_context_and_policy_binding()
    -> Result<(), &'static str> {
        let mut profile = ConnectionProfile::new_postgres(
            "test",
            ServerParams::default(),
            EnvTag::new(EnvKind::Local),
        );
        let mut request = request();
        assert!(gate(&request, &profile, "1", &request.sql).is_none());
        request.analyze = true;
        let Some(ExplainReply::ConfirmationRequired { token, .. }) =
            gate(&request, &profile, "1", &request.sql)
        else {
            return Err("measured read must confirm");
        };
        request.confirm_token = Some(token);
        assert!(gate(&request, &profile, "1", &request.sql).is_some());
        let other = gate(&request, &profile, "2", &request.sql);
        assert!(
            matches!(other, Some(ExplainReply::ConfirmationRequired { token, .. }) if Some(token.as_str()) != request.confirm_token.as_deref())
        );
        request.schema = "changed".into();
        assert!(matches!(
            gate(&request, &profile, "1", &request.sql),
            Some(ExplainReply::ConfirmationRequired { token, .. }) if Some(token.as_str()) != request.confirm_token.as_deref()
        ));
        request.sql = "DELETE FROM t".into();
        profile.safety = Safety::ReadOnly;
        assert!(matches!(
            gate(&request, &profile, "1", &request.sql),
            Some(ExplainReply::Blocked { .. })
        ));
        Ok(())
    }
    #[test]
    fn diagnostic_context_preserves_read_only_and_quotes_the_exact_schema() {
        let read_only = settings("odd\"schema", true);
        assert_eq!(
            read_only[0],
            "SET search_path = pg_catalog, \"odd\"\"schema\""
        );
        assert!(
            read_only
                .iter()
                .any(|setting| setting == "SET default_transaction_read_only = on")
        );
        assert!(
            settings("public", false)
                .iter()
                .all(|setting| !setting.contains("default_transaction_read_only"))
        );
    }
    #[test]
    fn redacted_unbound_multiple_and_utility_statements_never_reach_diagnostics() {
        for sql in [
            "SELECT ?",
            "SELECT $1",
            "SELECT 1; DELETE FROM t",
            "SET work_mem = '1MB'",
            "VACUUM t",
            "SHOW work_mem",
            "COPY t TO STDOUT",
            "PREPARE q AS SELECT 1",
            "DECLARE c CURSOR FOR SELECT 1",
            "EXPLAIN SELECT 1",
            "BEGIN",
        ] {
            let mut request = request();
            request.sql = sql.into();
            assert!(statement(&request).is_err(), "{sql}");
        }
    }
    #[test]
    fn literal_and_comment_question_marks_and_json_array_operators_are_complete_sql() {
        for sql in [
            "SELECT '?'",
            "SELECT $$?$$",
            "SELECT 1 -- ?",
            "SELECT payload ?| ARRAY['key'] FROM events",
            "SELECT payload ?& ARRAY['key'] FROM events",
        ] {
            let mut request = request();
            request.sql = sql.into();
            assert!(statement(&request).is_ok(), "{sql}");
        }
    }
}
