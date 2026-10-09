-- SPDX-License-Identifier: Apache-2.0
-- Reverses 0003_history_search.sql. The migration runner only applies up steps;
-- the migration test runs this file to prove the up step can be undone cleanly.

DROP TRIGGER history_fts_update;
DROP TRIGGER history_fts_delete;
DROP TRIGGER history_fts_insert;
DROP TABLE history_fts;
