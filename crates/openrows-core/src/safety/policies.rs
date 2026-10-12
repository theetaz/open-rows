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

#[test]
fn mongodb_native_reads_and_writes_use_all_six_policies() {
    use openrows_types::safety::Safety;
    for mode in [
        Safety::Unrestricted,
        Safety::ReadOnly,
        Safety::ConfirmAll,
        Safety::ConfirmWrites,
        Safety::AuthenticateAll,
        Safety::AuthenticateWrites,
    ] {
        let policy = SafetyPolicy {
            mode,
            dialect: Dialect::MongoDb,
        };
        let read = SafetyPlan::check(policy, vec!["db.x.find({a:1}).limit(10)".into()]);
        assert_eq!(
            read.gate(),
            if matches!(mode, Safety::ConfirmAll | Safety::AuthenticateAll) {
                Gate::Confirm
            } else {
                Gate::Run
            }
        );
        for source in [
            "db.x.insertOne({a:1})",
            "db.x.updateOne({a:1},{$set:{a:2}})",
            "db.x.deleteOne({a:1})",
            "db.x.aggregate([{$out:'x'}])",
            "db.x.find({$where:'function() { return true; }'})",
            "db.x.find({}).unknown()",
        ] {
            let write = SafetyPlan::check(policy, vec![source.into()]);
            let expected = if mode == Safety::ReadOnly {
                Gate::Block
            } else if mode == Safety::Unrestricted {
                Gate::Run
            } else {
                Gate::Confirm
            };
            assert_eq!(write.gate(), expected, "{mode:?} {source}");
        }
    }
}
