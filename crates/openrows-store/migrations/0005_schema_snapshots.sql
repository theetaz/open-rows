-- SPDX-License-Identifier: Apache-2.0
-- Local store v5 (docs/15 §2, M1-16): persisted schema-cache snapshots, one
-- row per listed scope (databases, schemas, one schema's objects of some
-- kinds, the name index) of a connection's database, so the tree renders at
-- once on reconnect or relaunch and refreshes in the background.

CREATE TABLE schema_snapshots (
  profile_id  TEXT NOT NULL,
  database_name TEXT NOT NULL,
  scope       TEXT NOT NULL,
  payload     BLOB NOT NULL,
  fetched_at  INTEGER NOT NULL,
  PRIMARY KEY (profile_id, database_name, scope)
);

CREATE INDEX schema_snapshots_fetched ON schema_snapshots(fetched_at);
