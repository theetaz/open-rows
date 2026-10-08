// SPDX-License-Identifier: Apache-2.0
//! Pre-execution safety gate (docs/04 §4, docs/12): read-only connections and
//! confirm-before-write. Pure and synchronous so every rule is unit-tested.
//!
//! Rules, per statement, from its [`Classification`]:
//! 1. Read-only connection: `Write`, `Ddl`, `Unknown` or dangerous statements
//!    are **blocked**; the whole run is refused and nothing executes.
//! 2. Otherwise dangerous statements always need confirmation, and with
//!    confirm-writes on, every `Write`, `Ddl` or `Unknown` statement does too.
//! 3. `Read` and `TxControl` statements run without asking.
use openrows_sql::{Classification, Dialect, classify};
use std::hash::{DefaultHasher, Hash, Hasher};

/// The connection's write-safety switches (FR-CONN-07).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SafetyPolicy {
    /// Refuse anything that may modify the database.
    pub read_only: bool,
    /// Ask before running anything that may modify the database.
    pub confirm_writes: bool,
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
    /// The statements and their verdicts.
    pub statements: Vec<CheckedStatement>,
}

impl SafetyPlan {
    /// Classify and judge `statements` under `policy`.
    #[must_use]
    pub fn check(policy: SafetyPolicy, statements: Vec<String>) -> Self {
        let statements = statements
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
        Self { statements }
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
    if policy.read_only && modifies {
        Verdict::Block
    } else if class.dangerous || (policy.confirm_writes && modifies) {
        Verdict::Confirm
    } else {
        Verdict::Allow
    }
}
