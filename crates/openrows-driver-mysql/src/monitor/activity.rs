// SPDX-License-Identifier: Apache-2.0
//! Bounded process projection and engine-specific `InnoDB` blocker edges, no `LOCK_DATA`.
use super::{Config, layout::Layout};
pub(super) fn query(config: &Config) -> String {
    let layout = config.layout;
    let process = if layout.modern_process && config.performance {
        "performance_schema.processlist"
    } else {
        "information_schema.PROCESSLIST"
    };
    let waits = if config.probed.waits {
        "LEFT JOIN performance_schema.threads th ON th.PROCESSLIST_ID = p.ID LEFT JOIN performance_schema.events_waits_current w ON w.THREAD_ID = th.THREAD_ID AND w.END_EVENT_ID IS NULL"
    } else {
        ""
    };
    let wait = if config.probed.waits {
        "CASE WHEN b.blockers IS NOT NULL THEN 'Lock: InnoDB' ELSE w.EVENT_NAME END"
    } else {
        "CASE WHEN b.blockers IS NOT NULL THEN 'Lock: InnoDB' ELSE NULL END"
    };
    let edges = blockers(layout, config.performance);
    format!(
        "SELECT p.ID, CASE WHEN p.COMMAND = 'Sleep' THEN CASE WHEN t.trx_id IS NULL THEN 'idle' ELSE 'idle in transaction' END ELSE 'active' END, {wait}, p.TIME * 1000, TIMESTAMPDIFF(MICROSECOND, t.trx_started, NOW()) / 1000, b.blockers, CASE WHEN OCTET_LENGTH(p.INFO) < 4096 THEN p.INFO ELSE NULL END, p.USER, p.DB, p.STATE, b.edge_count, 1, {uptime} FROM {process} p LEFT JOIN information_schema.INNODB_TRX t ON t.trx_mysql_thread_id = p.ID {waits} LEFT JOIN ({edges}) b ON b.waiter = p.ID WHERE p.ID <> CONNECTION_ID() ORDER BY p.TIME DESC, p.ID LIMIT 1000",
        uptime = if !layout.maria && !config.performance {
            "NULL".into()
        } else {
            super::sources::uptime(layout)
        }
    )
}
fn blockers(layout: Layout, performance: bool) -> &'static str {
    if layout.modern_locks && performance {
        "SELECT r.PROCESSLIST_ID waiter, GROUP_CONCAT(DISTINCT b.PROCESSLIST_ID) blockers, COUNT(*) edge_count FROM (SELECT REQUESTING_THREAD_ID, BLOCKING_THREAD_ID FROM performance_schema.data_lock_waits LIMIT 101) l LEFT JOIN performance_schema.threads r ON r.THREAD_ID = l.REQUESTING_THREAD_ID LEFT JOIN performance_schema.threads b ON b.THREAD_ID = l.BLOCKING_THREAD_ID GROUP BY r.PROCESSLIST_ID"
    } else if !layout.modern_locks {
        "SELECT r.trx_mysql_thread_id waiter, GROUP_CONCAT(DISTINCT b.trx_mysql_thread_id) blockers, COUNT(*) edge_count FROM (SELECT requesting_trx_id, blocking_trx_id FROM information_schema.INNODB_LOCK_WAITS LIMIT 101) l LEFT JOIN information_schema.INNODB_TRX r ON r.trx_id = l.requesting_trx_id LEFT JOIN information_schema.INNODB_TRX b ON b.trx_id = l.blocking_trx_id GROUP BY r.trx_mysql_thread_id"
    } else {
        "SELECT NULL waiter, NULL blockers, NULL edge_count"
    }
}
