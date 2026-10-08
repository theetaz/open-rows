-- SPDX-License-Identifier: Apache-2.0
-- Local store v2 (docs/15 §2, NFR-REL-01): the journal of staged table edits.
-- One row per table (profile + database + schema + table); the payload is the
-- webview's staged change set as JSON, including exact original values, so a
-- crash never loses staged work. Values are user data: the row stays local.

CREATE TABLE change_sets (
  id          TEXT PRIMARY KEY,
  profile_id  TEXT NOT NULL,
  target      TEXT NOT NULL,
  ops         TEXT NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE INDEX change_sets_profile ON change_sets(profile_id);
