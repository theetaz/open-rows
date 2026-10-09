// SPDX-License-Identifier: Apache-2.0
//! The ER diagram of a database file through the per-table default.
mod support;

use openrows_driver_api::Catalog;
use support::{TestResult, database};

const SCHEMA: &str = "CREATE TABLE Artist (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
    CREATE TABLE album (id INTEGER PRIMARY KEY, artist_id INTEGER REFERENCES artist(id),
        parent INTEGER REFERENCES album);
    CREATE TABLE loose (note TEXT);
    CREATE VIEW named AS SELECT name FROM Artist";

#[tokio::test]
async fn the_diagram_reads_tables_and_resolves_key_targets() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut session = database(&dir, SCHEMA).await?;
    let diagram = Catalog::schema_diagram(&mut session, "main", 10).await?;
    let names: Vec<_> = diagram.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Artist", "album", "loose"]);
    let artist = &diagram.tables[0];
    assert!(artist.columns[0].primary_key && !artist.columns[0].nullable);
    let keys: Vec<_> = diagram
        .foreign_keys
        .iter()
        .map(|k| (k.columns.join(","), k.target.table.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            ("parent".to_owned(), "album"),
            ("artist_id".to_owned(), "Artist")
        ]
    );
    let capped = Catalog::schema_diagram(&mut session, "main", 1).await?;
    assert!(capped.truncated && capped.foreign_keys.is_empty());
    Ok(())
}
