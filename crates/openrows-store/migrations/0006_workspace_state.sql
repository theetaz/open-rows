-- SPDX-License-Identifier: Apache-2.0
-- Local store v6 (docs/15 §2, M1-21, M1-26): session and workspace state of
-- each window, so query tabs, open connections and their databases survive a
-- relaunch. One row per window and part (`queryTabs`, `session`); the state is
-- the webview's JSON. Query text is user data and stays local.

CREATE TABLE workspace_state (
  window_id   TEXT NOT NULL,
  part        TEXT NOT NULL,
  state       TEXT NOT NULL,
  updated_at  INTEGER NOT NULL,
  PRIMARY KEY (window_id, part)
);
