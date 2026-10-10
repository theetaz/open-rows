// SPDX-License-Identifier: Apache-2.0
//! Parser-based execution policies (FR-APP-11). Authentication is performed
//! at the desktop boundary against the immutable checked run; it is never
//! accepted as a webview-supplied token. Dangerous statements always confirm.
use openrows_sql::{Classification, Dialect, classify};
use std::hash::{DefaultHasher, Hash, Hasher};

/// The connection's write-safety switches (FR-CONN-07).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SafetyPolicy {
    /// Explicit execution policy.
    pub mode: openrows_types::safety::Safety,
    /// The engine's SQL dialect, whose rules classify each statement.
    pub dialect: Dialect,
}

/// What the gate decided for one statement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Runs without asking.
    Allow,
    /// Runs only after the user confirms.
    Confirm,
    /// Never runs on this connection.
    Block,
}

/// A statement with its classification and verdict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStatement {
    /// Statement text as it will be executed (no trailing `;`).
    pub sql: String,
    /// What the statement does.
    pub class: Classification,
    /// The gate's decision.
    pub verdict: Verdict,
}

/// What the whole run needs before it may start.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    /// Every statement is allowed.
    Run,
    /// At least one statement needs confirmation; none is blocked.
    Confirm,
    /// At least one statement is blocked; nothing may run.
    Block,
}

/// Checked statements of one run, in execution order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafetyPlan {
    /// Whether native authentication is required for this exact run.
    pub authentication_required: bool,
    /// The statements and their verdicts.
    pub statements: Vec<CheckedStatement>,
}

impl SafetyPlan {
    /// Classify and judge `statements` under `policy`.
    #[must_use]
    pub fn check(policy: SafetyPolicy, statements: Vec<String>) -> Self {
        let statements: Vec<CheckedStatement> = statements
            .into_iter()
            .map(|sql| {
                let class = classify(&sql, policy.dialect);
                CheckedStatement {
                    verdict: verdict(policy, class),
                    sql,
                    class,
                }
            })
            .collect();
        let authentication_required = policy.mode.authenticates()
            && statements.iter().any(|s: &CheckedStatement| {
                policy.mode == openrows_types::safety::Safety::AuthenticateAll
                    || s.class.kind != openrows_sql::StatementKind::Read
            });
        Self {
            authentication_required,
            statements,
        }
    }

    /// The strictest verdict across all statements.
    #[must_use]
    pub fn gate(&self) -> Gate {
        let has = |wanted| {
            self.statements
                .iter()
                .any(|statement| statement.verdict == wanted)
        };
        if has(Verdict::Block) {
            Gate::Block
        } else if has(Verdict::Confirm) {
            Gate::Confirm
        } else {
            Gate::Run
        }
    }

    /// Binds a confirmation to exactly these statements on `scope` (e.g. the
    /// session id): any edit in between yields a different token.
    #[must_use]
    pub fn confirmation_token(&self, scope: &str) -> String {
        let mut hasher = DefaultHasher::new();
        scope.hash(&mut hasher);
        for statement in &self.statements {
            statement.sql.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    /// May the run start, given the token the user confirmed (if any)?
    #[must_use]
    pub fn allows(&self, scope: &str, confirmed: Option<&str>) -> bool {
        match self.gate() {
            Gate::Run => true,
            Gate::Confirm => confirmed == Some(self.confirmation_token(scope).as_str()),
            Gate::Block => false,
        }
    }
}

/// The gate's rule for one classified statement.
#[must_use]
pub fn verdict(policy: SafetyPolicy, class: Classification) -> Verdict {
    let modifies = class.is_write() || class.dangerous;
    if policy.mode.read_only() && modifies {
        Verdict::Block
    } else if class.dangerous
        || matches!(
            policy.mode,
            openrows_types::safety::Safety::ConfirmAll
                | openrows_types::safety::Safety::AuthenticateAll
        )
        || (policy.mode.confirm_writes()
            && (modifies
                || (policy.mode.authenticates()
                    && class.kind != openrows_sql::StatementKind::Read)))
    {
        Verdict::Confirm
    } else {
        Verdict::Allow
    }
}

#[cfg(test)]
mod clickhouse_tests {
    use super::*;
    #[test]
    fn read_only_connections_block_clickhouse_mutations() {
        let policy = SafetyPolicy {
            mode: openrows_types::safety::Safety::ReadOnly,
            dialect: Dialect::ClickHouse,
        };
        for sql in [
            "INSERT INTO t VALUES (1)",
            "ALTER TABLE t UPDATE x=1 WHERE id=2",
            "ALTER TABLE t DELETE WHERE id=1",
            "OPTIMIZE TABLE t FINAL",
            "SYSTEM RELOAD DICTIONARY d",
            "KILL QUERY WHERE query_id='x'",
            "DROP TABLE t",
            "TRUNCATE TABLE t",
        ] {
            assert_eq!(
                SafetyPlan::check(policy, vec![sql.to_owned()]).gate(),
                Gate::Block,
                "{sql}"
            );
        }
        assert_eq!(
            SafetyPlan::check(policy, vec!["SELECT 1".to_owned()]).gate(),
            Gate::Run
        );
    }
}

#[cfg(test)]
mod policies;
