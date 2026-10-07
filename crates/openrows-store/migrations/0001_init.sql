-- SPDX-License-Identifier: Apache-2.0
-- Local store v1 (docs/15 §2). Connection profiles live in JSON files
-- (docs/15 §3), so they have no table here. history_fts comes with search.

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE history (
  id            INTEGER PRIMARY KEY,
  profile_id    TEXT NOT NULL,
  database      TEXT,
  started_at    INTEGER NOT NULL,
  duration_ms   INTEGER,
  status        TEXT NOT NULL CHECK (status IN ('ok', 'error', 'cancelled')),
  rows          INTEGER,
  statement     TEXT NOT NULL,
  error_message TEXT,
  source        TEXT NOT NULL CHECK (source IN ('editor', 'browse', 'edit-commit', 'api'))
);

CREATE INDEX history_profile_time ON history(profile_id, started_at DESC);
