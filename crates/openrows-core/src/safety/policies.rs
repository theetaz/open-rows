// SPDX-License-Identifier: Apache-2.0
//! Six-policy classification and approval-binding regression matrix.
use super::*;
use openrows_types::safety::Safety;

#[test]
fn six_policies_cover_read_write_ddl_dangerous_unknown_and_transaction_control() {
    let statements = [
        "SELECT 1",
        "UPDATE t SET a=1 WHERE id=1",
        "CREATE TABLE t (id int)",
        "DROP TABLE t",
        "CALL external_action()",
        "BEGIN",
    ];
    let policies = [
        (
            Safety::Unrestricted,
            [
                Gate::Run,
                Gate::Run,
                Gate::Run,
                Gate::Confirm,
                Gate::Run,
                Gate::Run,
            ],
        ),
        (Safety::ConfirmAll, [Gate::Confirm; 6]),
        (
            Safety::ConfirmWrites,
            [
                Gate::Run,
                Gate::Confirm,
                Gate::Confirm,
                Gate::Confirm,
                Gate::Confirm,
                Gate::Run,
            ],
        ),
        (Safety::AuthenticateAll, [Gate::Confirm; 6]),
        (
            Safety::AuthenticateWrites,
            [
                Gate::Run,
                Gate::Confirm,
                Gate::Confirm,
                Gate::Confirm,
                Gate::Confirm,
                Gate::Confirm,
            ],
        ),
        (
            Safety::ReadOnly,
            [
                Gate::Run,
                Gate::Block,
                Gate::Block,
                Gate::Block,
                Gate::Block,
                Gate::Run,
            ],
        ),
    ];
    for (mode, expected) in policies {
        for (index, sql) in statements.iter().enumerate() {
            let plan = SafetyPlan::check(
                SafetyPolicy {
                    mode,
                    dialect: Dialect::Postgres,
                },
                vec![(*sql).into()],
            );
            assert_eq!(plan.gate(), expected[index], "{mode:?}: {sql}");
            assert_eq!(
                plan.authentication_required,
                mode.authenticates() && (mode == Safety::AuthenticateAll || index != 0)
            );
        }
    }
}

#[test]
fn authentication_review_never_authorizes_extra_edited_or_other_session_statements() {
    let policy = SafetyPolicy {
        mode: Safety::AuthenticateAll,
        dialect: Dialect::Postgres,
    };
    let plan = |sql| SafetyPlan::check(policy, sql);
    let a = plan(vec!["SELECT 1".into()]);
    let token = a.confirmation_token("session-a");
    assert!(a.allows("session-a", Some(&token)));
    assert!(!a.allows("session-b", Some(&token)));
    assert!(!plan(vec!["SELECT 2".into()]).allows("session-a", Some(&token)));
    assert!(!plan(vec!["SELECT 1".into(), "SELECT 2".into()]).allows("session-a", Some(&token)));
}
