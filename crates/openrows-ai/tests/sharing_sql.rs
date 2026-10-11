// SPDX-License-Identifier: Apache-2.0
//! Every supported SQL lexer applies the context privacy contract.
mod support;
use openrows_ai::{AiControl, sharing::*};
use openrows_sql::Dialect;
use support::sharing::*;
#[test]
fn all_engine_lexers_remove_literals_and_comments_before_identifiers() -> Result {
    let store = SharingStore::memory(1000)?;
    store.save("connection", policy(SharingLevel::SchemaOnly), false)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let config = provider();
    for (dialect, sql) in [
        (
            Dialect::Postgres,
            "SELECT E'private', $$private$$, $tag$private$tag$, 123 -- private",
        ),
        (
            Dialect::Duckdb,
            "SELECT 'private', $$private$$, 123 -- private",
        ),
        (
            Dialect::MySql,
            "SELECT \"private\", 'private', 0x123 # private",
        ),
        (
            Dialect::SqlServer,
            "SELECT N'private', 0x123, [safe] /* private */",
        ),
        (
            Dialect::Sqlite,
            "SELECT 'private', 123, [safe] /* private */",
        ),
        (Dialect::ClickHouse, "SELECT 'private', 123 /* private */"),
    ] {
        let mut requested = request(
            &config,
            &control,
            ContextBundle {
                query: Some(sql.into()),
                ..ContextBundle::default()
            },
        );
        requested.dialect = dialect;
        let preview = store.gate(requested)?.preview();
        assert!(!preview.payload.contains("private") && !preview.payload.contains("123"));
    }
    Ok(())
}

#[test]
fn ambiguous_sqlite_quotes_unterminated_identifiers_and_boolean_literals_are_hidden() -> Result {
    let store = SharingStore::memory(1000)?;
    store.save("connection", policy(SharingLevel::SchemaOnly), false)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let config = provider();
    let mut requested = request(
        &config,
        &control,
        ContextBundle {
            query: Some("SELECT \"private\", TRUE, FALSE, NULL".into()),
            ..ContextBundle::default()
        },
    );
    requested.dialect = Dialect::Sqlite;
    let preview = store.gate(requested)?.preview();
    for literal in ["private", "TRUE", "FALSE", "NULL"] {
        assert!(!preview.payload.contains(literal));
    }
    let preview = store
        .gate(request(
            &config,
            &control,
            ContextBundle {
                query: Some("SELECT public.table.\"private".into()),
                ..ContextBundle::default()
            },
        ))?
        .preview();
    assert!(!preview.payload.contains("private"));
    Ok(())
}
