-- SPDX-License-Identifier: Apache-2.0
-- Local store v3 (docs/15 §2, FR-WS-01): full-text search over executed statements.
-- An external-content FTS5 index over history.statement, kept in step by triggers.
-- The index is rebuilt once here so statements recorded before v3 are searchable.

CREATE VIRTUAL TABLE history_fts USING fts5(statement, content='history', content_rowid='id');

INSERT INTO history_fts (history_fts) VALUES ('rebuild');

CREATE TRIGGER history_fts_insert AFTER INSERT ON history BEGIN
  INSERT INTO history_fts (rowid, statement) VALUES (new.id, new.statement);
END;

CREATE TRIGGER history_fts_delete AFTER DELETE ON history BEGIN
  INSERT INTO history_fts (history_fts, rowid, statement) VALUES ('delete', old.id, old.statement);
END;

CREATE TRIGGER history_fts_update AFTER UPDATE ON history BEGIN
  INSERT INTO history_fts (history_fts, rowid, statement) VALUES ('delete', old.id, old.statement);
  INSERT INTO history_fts (rowid, statement) VALUES (new.id, new.statement);
END;
