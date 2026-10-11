-- SPDX-License-Identifier: Apache-2.0
CREATE TABLE monitor_profiles (profile_id TEXT PRIMARY KEY);
CREATE TABLE monitor_history (
    profile_id TEXT NOT NULL,
    database_name TEXT NOT NULL,
    metric TEXT NOT NULL,
    collected_ms INTEGER NOT NULL,
    value REAL,
    state TEXT NOT NULL,
    PRIMARY KEY (profile_id, database_name, metric, collected_ms)
) WITHOUT ROWID;
CREATE INDEX monitor_history_age ON monitor_history(collected_ms);
