-- SPDX-License-Identifier: Apache-2.0
-- Local store v4 (docs/15 §2, M1-21): saved queries of each connection,
-- listed in the sidebar's Queries tab. The SQL is user data and stays local.

CREATE TABLE saved_queries (
  id          TEXT PRIMARY KEY,
  profile_id  TEXT NOT NULL,
  name        TEXT NOT NULL CHECK (length(name) > 0),
  sql         TEXT NOT NULL,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE INDEX saved_queries_profile ON saved_queries(profile_id, name);
