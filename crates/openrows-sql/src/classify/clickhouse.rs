// SPDX-License-Identifier: Apache-2.0
//! `ClickHouse` reads and server/data mutations (FR-APP-07).
use super::{Classification, Sql, StatementKind, heads};

pub(super) fn dispatch(sql: Sql<'_>) -> Classification {
    match sql.word(0).as_deref() {
        Some("SELECT" | "SHOW" | "DESCRIBE" | "DESC" | "EXPLAIN" | "EXISTS") => {
            Classification::new(StatementKind::Read)
        }
        Some("WITH") => heads::with(sql),
        Some("INSERT" | "ALTER" | "OPTIMIZE" | "SYSTEM" | "KILL" | "DELETE" | "UPDATE") => {
            Classification::dangerous(StatementKind::Write)
        }
        Some("DROP" | "TRUNCATE" | "DETACH" | "RENAME" | "ATTACH") => {
            Classification::dangerous(StatementKind::Ddl)
        }
        Some("CREATE" | "GRANT" | "REVOKE") => Classification::new(StatementKind::Ddl),
        Some("SET" | "USE") => Classification::new(StatementKind::Write),
        _ => Classification::new(StatementKind::Unknown),
    }
}
