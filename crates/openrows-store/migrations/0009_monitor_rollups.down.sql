-- SPDX-License-Identifier: Apache-2.0
-- Reverses 0009_monitor_rollups.sql; rolled-up rows remain as single samples.
DROP INDEX monitor_history_tier;
ALTER TABLE monitor_history DROP COLUMN ok_count;
ALTER TABLE monitor_history DROP COLUMN sum_value;
ALTER TABLE monitor_history DROP COLUMN max_value;
ALTER TABLE monitor_history DROP COLUMN min_value;
ALTER TABLE monitor_history DROP COLUMN span_ms;
