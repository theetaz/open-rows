-- SPDX-License-Identifier: Apache-2.0
-- Reverses 0001_init.sql. The migration runner only applies up steps; the
-- migration test runs every down file in reverse order to prove each step
-- can be undone cleanly and re-applied.

DROP INDEX history_profile_time;
DROP TABLE history;
DROP TABLE meta;
