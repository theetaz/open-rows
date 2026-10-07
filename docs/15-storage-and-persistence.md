# 15 — Storage and Persistence

## 1. What is stored where

```mermaid
flowchart LR
    subgraph appdata["App data dir (per user, per machine)"]
        settings["settings.json<br/>keybindings.json"]
        profiles["connections/*.json<br/>(no secrets)"]
        db[("openrows.sqlite<br/>history, drafts, change sets,<br/>schema snapshots, workspace state")]
        backups["backups/openrows.sqlite.N"]
    end
    subgraph cache["Cache dir"]
        spill["spill/<session>/*.arrow"]
        resp["responses/*.body"]
        thumbs["previews/"]
    end
    subgraph ws["Project folder (user chosen, git-friendly)"]
        wsmeta[".openrows/project.yaml"]
        queries["queries/**/*.sql"]
        apis["apis/<collection>/**"]
        shared["connections/*.yaml<br/>(shared, secret-free)"]
    end
    subgraph kc["OS keychain"]
        secrets["passwords, tokens,<br/>API secrets"]
    end
```

| Platform | App data | Cache | Logs |
| --- | --- | --- | --- |
| macOS | `~/Library/Application Support/com.prabhavalabs.openrows/` | `~/Library/Caches/com.prabhavalabs.openrows/` | `~/Library/Logs/OpenRows/` |
| Windows | `%APPDATA%\OpenRows\Client\` | `%LOCALAPPDATA%\OpenRows\Client\Cache\` | `%LOCALAPPDATA%\OpenRows\Client\Logs\` |
| Linux | `$XDG_DATA_HOME/openrows/` | `$XDG_CACHE_HOME/openrows/` | `$XDG_STATE_HOME/openrows/logs/` |

Paths come from Tauri's path resolver / the `directories` crate; the bundle identifier is `com.prabhavalabs.openrows`.

## 2. Local store (SQLite)

Single SQLite database, WAL mode, `synchronous=NORMAL`, accessed only by the core through one writer task and a small reader pool.

```sql
-- Simplified schema (v1)
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE history (
  id            INTEGER PRIMARY KEY,
  profile_id    TEXT NOT NULL,
  database      TEXT,
  started_at    INTEGER NOT NULL,          -- unix ms
  duration_ms   INTEGER,
  status        TEXT NOT NULL,             -- ok | error | cancelled
  rows          INTEGER,
  statement     TEXT NOT NULL,
  error_message TEXT,
  source        TEXT NOT NULL              -- editor | browse | edit-commit | api
);
CREATE INDEX history_profile_time ON history(profile_id, started_at DESC);
CREATE VIRTUAL TABLE history_fts USING fts5(statement, content='history', content_rowid='id');

CREATE TABLE drafts (
  tab_id      TEXT PRIMARY KEY,
  kind        TEXT NOT NULL,               -- query | api-request
  content     TEXT NOT NULL,
  file_ref    TEXT,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE change_sets (
  id          TEXT PRIMARY KEY,
  tab_id      TEXT NOT NULL,
  profile_id  TEXT NOT NULL,
  target      TEXT NOT NULL,               -- JSON EditTarget
  ops         BLOB NOT NULL,               -- versioned binary (postcard) of ops + undo stack
  updated_at  INTEGER NOT NULL
);

CREATE TABLE schema_snapshots (
  profile_id  TEXT NOT NULL,
  scope       TEXT NOT NULL,               -- object path
  server_ver  TEXT NOT NULL,
  fingerprint TEXT,
  payload     BLOB NOT NULL,               -- compressed
  fetched_at  INTEGER NOT NULL,
  PRIMARY KEY (profile_id, scope)
);

CREATE TABLE workspace_state (
  window_id   TEXT PRIMARY KEY,
  state       TEXT NOT NULL,               -- JSON: tabs, layout
  updated_at  INTEGER NOT NULL
);

CREATE TABLE usage_stats (
  profile_id  TEXT NOT NULL,
  object      TEXT NOT NULL,
  uses        INTEGER NOT NULL,
  last_used   INTEGER NOT NULL,
  PRIMARY KEY (profile_id, object)
);

CREATE TABLE api_history (
  id          INTEGER PRIMARY KEY,
  request_ref TEXT,
  method      TEXT, url TEXT, status INTEGER,
  started_at  INTEGER NOT NULL,
  timings     TEXT,                        -- JSON
  response_meta TEXT,                      -- headers, size; body stored only if small or user-pinned
  body_ref    TEXT
);
```

Retention defaults: history 90 days or 100 000 entries per profile (whichever first); API history 30 days; schema snapshots evicted when the profile is deleted or after 60 days unused.

## 3. Connection profiles

Two layers, merged at runtime:

| Layer | Location | Contains |
| --- | --- | --- |
| **Personal** | App data `connections/<id>.json` | Everything for the user's own connections (minus secrets). |
| **Shared** | Project `connections/*.yaml` | Team-shared definitions: driver, host, port, database, SSH bastion host, TLS mode, env tag. No usernames or secrets by default. Each user binds personal credentials to a shared definition locally. |

```json
{
  "openrows": 1,
  "id": "4f0b5b4e-…",
  "name": "Orders – staging",
  "driver": "postgres",
  "group": "Orders",
  "envTag": { "kind": "staging", "color": "#d97706" },
  "params": { "host": "orders-db.staging.internal", "port": 5432, "database": "orders", "user": "app_user" },
  "secrets": { "password": "keychain" },
  "ssh": { "host": "bastion.staging", "user": "app_user", "auth": "agent" },
  "tls": { "mode": "verify-full" },
  "safety": { "readOnly": false, "confirmWrites": false },
  "session": { "mode": "per-tab", "maxSessions": 4, "startup": ["SET search_path = orders, public"] }
}
```

## 4. Settings

- `settings.json`: user-level preferences (theme, fonts, grid density, editor options, history retention, telemetry opt-in, update channel).
- Project `.openrows/project.yaml` may override a safe subset (formatting style, default page size) so teams share conventions.
- JSON Schema provided; settings UI and file stay in sync (file watcher).

<a id="5-workspaces"></a>

## 5. Projects

A Project is a folder the user opens with **Open Project Folder** (or the default personal Project in app data). A Workspace is a connection to a server/host and can contain multiple databases; it is separate from the Project folder:

```text
my-team-project/
  .openrows/
    project.yaml        # name, defaults, trusted flag is stored locally, NOT here
  connections/            # shared, secret-free connection definitions
  queries/                # .sql files, any folder structure
  apis/                   # API collections (see 11-api-client.md)
  snippets/
  .gitignore              # generated: .env, .openrows/local/
```

"Trusted" state (allowing scripts, `ors.db.query`) is stored in the local store keyed by Project path + git remote, never in the Project itself — otherwise a malicious repo could mark itself trusted.

## 6. Migrations

- Local store: numbered SQL migrations embedded in the binary (`openrows-store/migrations/0001_init.sql` …), applied at startup inside a transaction after a file-level backup.
- Files (profiles, settings, collections): `openrows: <version>` field; loaders upgrade in memory and write back only on the next user-initiated save, keeping a `.bak` the first time a file is upgraded across a major version.
- Downgrades: a newer local store version opened by an older app → read-only safe mode with a clear message, never corruption.

## 7. Backup and portability

- "Export everything" produces an archive of profiles (optionally with encrypted secrets), settings, snippets, and scratch queries.
- History is excluded by default (sensitive and large) but can be included.

## 8. Implementation notes (D-03: local store and connection profiles)

First slice of M1-02 (local store) and M1-03 (profiles). Nothing here depends on Tauri; the app wiring comes with the connection UI.

| Area | What exists |
| --- | --- |
| Crate | `crates/openrows-store`: `LocalStore` (SQLite) and the `profile` module (model, `postgres://` parser, file repository, `ProfileService`). The profile model lives here rather than in `openrows-core` so it can ship before the core crate; it moves (or is re-exported) when `openrows-core` takes over connection orchestration. |
| SQLite | `rusqlite` 0.40 with bundled SQLite; `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`. One writer connection behind a `Mutex` (single-writer design); no reader pool yet. |
| Schema v1 | `migrations/0001_init.sql`: `meta` and `history` (+ `history_profile_time` index) from §2, with `CHECK` constraints on `status` / `source`. `history_fts`, drafts, change sets, snapshots, workspace state, usage stats and API history arrive with their features as later numbered migrations. |
| Migrations | Embedded with `include_str!`; the applied version is `PRAGMA user_version` (mirrored to `meta.schema_version`). Pending steps run in one transaction; re-opening is a no-op. Before upgrading a non-empty database it is copied with `VACUUM INTO` to `backups/openrows.sqlite.v<old>` next to the DB (consistent with WAL, unlike a raw file copy). A database newer than the build fails with `StoreError::NewerSchema` and is not touched; the read-only safe mode of §6 is the caller's job. |
| History API | `record_history(&HistoryRecord) -> id`, `recent_history(profile_id, limit)` (newest first). No retention sweep or FTS yet. |
| Profiles | Personal profiles are JSON files `connections/<uuid-v7>.json`, as §3 specifies, rather than SQLite rows: they are human-readable, diff-able, easy to export, and match the shared `connections/*.yaml` layer. Groups are a `group` string on each profile; `groups()` lists the distinct names. Writes are atomic (temp file + rename) and `0600` on Unix. `load_all` returns readable profiles plus a list of unreadable files, so one corrupt file never hides the rest. Files with `openrows` > 1 are rejected (`UnsupportedFormat`). |
| Format | As in §3 with `envTag` as `{ kind, label?, color }`, `params.applicationName` and `params.options` (other libpq parameters). `ssh` and `session` are not in the model yet (M1-05/M1-06). |
| Secrets | `secrets.password` is only `"keychain"` or absent; the value lives in `openrows-secrets` under `conn/<id>/password` (docs/12 §8). |

