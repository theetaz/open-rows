-- SPDX-License-Identifier: Apache-2.0
-- Tiered monitor history: raw samples keep span 0; older samples roll up into
-- coarser buckets that keep the minimum, maximum, sum and count of their valid
-- values, plus the last valid value and the most recent missing state.
ALTER TABLE monitor_history ADD COLUMN span_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE monitor_history ADD COLUMN min_value REAL;
ALTER TABLE monitor_history ADD COLUMN max_value REAL;
ALTER TABLE monitor_history ADD COLUMN sum_value REAL NOT NULL DEFAULT 0;
ALTER TABLE monitor_history ADD COLUMN ok_count INTEGER NOT NULL DEFAULT 0;
UPDATE monitor_history
SET min_value = value, max_value = value, sum_value = value, ok_count = 1
WHERE value IS NOT NULL;
CREATE INDEX monitor_history_tier ON monitor_history(span_ms, collected_ms);
