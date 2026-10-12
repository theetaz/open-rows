-- SPDX-License-Identifier: Apache-2.0
-- Local store v8 (docs/28 §5, M25-06): analytics boards of each connection.
-- `definition` is the validated, secret-free board JSON. `reviewed` is 0 for
-- an imported board until the user approved running its queries.

CREATE TABLE boards (
  id          TEXT PRIMARY KEY,
  profile_id  TEXT NOT NULL,
  name        TEXT NOT NULL CHECK (length(name) > 0),
  definition  TEXT NOT NULL,
  reviewed    INTEGER NOT NULL CHECK (reviewed IN (0, 1)),
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE INDEX boards_profile ON boards(profile_id, name);
