// SPDX-License-Identifier: Apache-2.0
//! Saved queries of each connection (docs/15 §2, M1-21): a name and SQL,
//! listed in the sidebar's Queries tab.

use rusqlite::{Row, params};
use uuid::Uuid;

use super::{
    LocalStore,
    error::{StoreError, sqlite},
};

/// A saved query; timestamps are Unix epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQueryRecord {
    /// Stable id (UUID v7).
    pub id: String,
    /// Connection profile the query belongs to.
    pub profile_id: String,
    /// Name shown in the sidebar.
    pub name: String,
    /// The query text.
    pub sql: String,
    /// When it was saved first.
    pub created_at_ms: i64,
    /// Last rename or edit.
    pub updated_at_ms: i64,
}

/// What a new saved query holds.
#[derive(Debug, Clone, Copy)]
pub struct NewSavedQuery<'a> {
    /// Connection profile the query belongs to.
    pub profile_id: &'a str,
    /// Name shown in the sidebar.
    pub name: &'a str,
    /// The query text.
    pub sql: &'a str,
    /// Creation time, Unix epoch milliseconds.
    pub now_ms: i64,
}

const INSERT: &str = "INSERT INTO saved_queries \
     (id, profile_id, name, sql, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)";

const SELECT: &str = "SELECT id, profile_id, name, sql, created_at, updated_at \
     FROM saved_queries WHERE profile_id = ?1 ORDER BY name COLLATE NOCASE, created_at";

fn record(row: &Row<'_>) -> rusqlite::Result<SavedQueryRecord> {
    Ok(SavedQueryRecord {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        name: row.get(2)?,
        sql: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
    })
}

impl LocalStore {
    /// Save a new query and return it with its id.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails (e.g. an empty name).
    pub fn create_saved_query(
        &self,
        query: NewSavedQuery<'_>,
    ) -> Result<SavedQueryRecord, StoreError> {
        let saved = SavedQueryRecord {
            id: Uuid::now_v7().to_string(),
            profile_id: query.profile_id.to_owned(),
            name: query.name.to_owned(),
            sql: query.sql.to_owned(),
            created_at_ms: query.now_ms,
            updated_at_ms: query.now_ms,
        };
        self.lock()?
            .execute(
                INSERT,
                params![
                    saved.id,
                    saved.profile_id,
                    saved.name,
                    saved.sql,
                    saved.created_at_ms
                ],
            )
            .map_err(sqlite("save query"))?;
        Ok(saved)
    }

    /// Saved queries of one connection, by name.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the read fails.
    pub fn saved_queries(&self, profile_id: &str) -> Result<Vec<SavedQueryRecord>, StoreError> {
        let conn = self.lock()?;
        let mut statement = conn.prepare(SELECT).map_err(sqlite("list saved queries"))?;
        let rows = statement
            .query_map([profile_id], record)
            .map_err(sqlite("list saved queries"))?;
        rows.collect::<Result<_, _>>()
            .map_err(sqlite("list saved queries"))
    }

    /// Rename a saved query; `false` when it does not exist.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails (e.g. an empty name).
    pub fn rename_saved_query(
        &self,
        id: &str,
        name: &str,
        now_ms: i64,
    ) -> Result<bool, StoreError> {
        self.lock()?
            .execute(
                "UPDATE saved_queries SET name = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, name, now_ms],
            )
            .map(|changed| changed > 0)
            .map_err(sqlite("rename saved query"))
    }

    /// Delete a saved query; `false` when it did not exist.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn delete_saved_query(&self, id: &str) -> Result<bool, StoreError> {
        self.lock()?
            .execute("DELETE FROM saved_queries WHERE id = ?1", [id])
            .map(|changed| changed > 0)
            .map_err(sqlite("delete saved query"))
    }
}
