// SPDX-License-Identifier: Apache-2.0
//! Analytics boards of each connection (docs/28 §5, M25-06): the validated
//! board JSON and whether the user approved running its queries.

use rusqlite::{OptionalExtension, Row, params};
use uuid::Uuid;

use super::{
    LocalStore,
    error::{StoreError, sqlite},
};

/// A stored board; timestamps are Unix epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRecord {
    /// Stable id (UUID v7).
    pub id: String,
    /// Connection profile the board belongs to.
    pub profile_id: String,
    /// Display name, also inside `definition`.
    pub name: String,
    /// Validated board JSON.
    pub definition: String,
    /// `false` for an imported board until the user approved its queries.
    pub reviewed: bool,
    /// When it was created.
    pub created_at_ms: i64,
    /// Last edit or approval.
    pub updated_at_ms: i64,
}

/// What a new board holds.
#[derive(Debug, Clone, Copy)]
pub struct NewBoard<'a> {
    /// Connection profile the board belongs to.
    pub profile_id: &'a str,
    /// Display name.
    pub name: &'a str,
    /// Validated board JSON.
    pub definition: &'a str,
    /// Whether its queries may run without review.
    pub reviewed: bool,
    /// Creation time, Unix epoch milliseconds.
    pub now_ms: i64,
}

const COLUMNS: &str = "id, profile_id, name, definition, reviewed, created_at, updated_at";

fn record(row: &Row<'_>) -> rusqlite::Result<BoardRecord> {
    Ok(BoardRecord {
        id: row.get(0)?,
        profile_id: row.get(1)?,
        name: row.get(2)?,
        definition: row.get(3)?,
        reviewed: row.get(4)?,
        created_at_ms: row.get(5)?,
        updated_at_ms: row.get(6)?,
    })
}

impl LocalStore {
    /// Store a new board and return it with its id.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails.
    pub fn create_board(&self, board: NewBoard<'_>) -> Result<BoardRecord, StoreError> {
        let stored = BoardRecord {
            id: Uuid::now_v7().to_string(),
            profile_id: board.profile_id.to_owned(),
            name: board.name.to_owned(),
            definition: board.definition.to_owned(),
            reviewed: board.reviewed,
            created_at_ms: board.now_ms,
            updated_at_ms: board.now_ms,
        };
        self.lock()?
            .execute(
                &format!("INSERT INTO boards ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)"),
                params![
                    stored.id,
                    stored.profile_id,
                    stored.name,
                    stored.definition,
                    stored.reviewed,
                    stored.created_at_ms
                ],
            )
            .map_err(sqlite("save board"))?;
        Ok(stored)
    }

    /// Boards of one connection, by name.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the read fails.
    pub fn boards(&self, profile_id: &str) -> Result<Vec<BoardRecord>, StoreError> {
        let conn = self.lock()?;
        let sql = format!(
            "SELECT {COLUMNS} FROM boards WHERE profile_id = ?1 \
             ORDER BY name COLLATE NOCASE, created_at"
        );
        let mut statement = conn.prepare(&sql).map_err(sqlite("list boards"))?;
        let rows = statement
            .query_map([profile_id], record)
            .map_err(sqlite("list boards"))?;
        rows.collect::<Result<_, _>>()
            .map_err(sqlite("list boards"))
    }

    /// One board, or `None` when it does not exist.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the read fails.
    pub fn board(&self, id: &str) -> Result<Option<BoardRecord>, StoreError> {
        self.lock()?
            .query_row(
                &format!("SELECT {COLUMNS} FROM boards WHERE id = ?1"),
                [id],
                record,
            )
            .optional()
            .map_err(sqlite("read board"))
    }

    /// Replace a board's name and definition, keeping its review state;
    /// `false` when it does not exist.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails.
    pub fn update_board(
        &self,
        id: &str,
        (name, definition): (&str, &str),
        now_ms: i64,
    ) -> Result<bool, StoreError> {
        self.lock()?
            .execute(
                "UPDATE boards SET name = ?2, definition = ?3, updated_at = ?4 WHERE id = ?1",
                params![id, name, definition, now_ms],
            )
            .map(|changed| changed > 0)
            .map_err(sqlite("update board"))
    }

    /// Record that the user reviewed and approved a board's queries.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the write fails.
    pub fn approve_board(&self, id: &str, now_ms: i64) -> Result<bool, StoreError> {
        self.lock()?
            .execute(
                "UPDATE boards SET reviewed = 1, updated_at = ?2 WHERE id = ?1",
                params![id, now_ms],
            )
            .map(|changed| changed > 0)
            .map_err(sqlite("approve board"))
    }

    /// Delete a board; `false` when it did not exist.
    ///
    /// # Errors
    /// Returns a `Sqlite` error when the delete fails.
    pub fn delete_board(&self, id: &str) -> Result<bool, StoreError> {
        self.lock()?
            .execute("DELETE FROM boards WHERE id = ?1", [id])
            .map(|changed| changed > 0)
            .map_err(sqlite("delete board"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_boards_stay_unreviewed_until_approved() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let store = LocalStore::open(&dir.path().join("test.sqlite"))?;
        let new = NewBoard {
            profile_id: "p1",
            name: "Sales",
            definition: "{}",
            reviewed: false,
            now_ms: 1,
        };
        let board = store.create_board(new)?;
        store.create_board(NewBoard {
            profile_id: "p2",
            ..new
        })?;
        assert_eq!(store.boards("p1")?, vec![board.clone()]);
        assert!(store.update_board(&board.id, ("Renamed", "{\"a\":1}"), 2)?);
        let edited = store.board(&board.id)?;
        assert_eq!(edited.as_ref().map(|b| b.reviewed), Some(false));
        assert!(store.approve_board(&board.id, 3)?);
        assert_eq!(store.board(&board.id)?.map(|b| b.reviewed), Some(true));
        assert!(store.delete_board(&board.id)?);
        assert_eq!(store.board(&board.id)?, None);
        assert!(!store.approve_board(&board.id, 4)?);
        Ok(())
    }
}
