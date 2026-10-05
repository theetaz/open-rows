# 0006 — File-per-request YAML collections for the API client

- Status: Accepted
- Date: 2026-10-05

## Context

Users want API collections versioned in git alongside code, reviewable in PRs, shareable without accounts or cloud sync. Single-file JSON exports produce huge diffs and frequent merge conflicts.

## Decision

- A collection is a **folder**; each request is **one YAML file** (`*.req.yaml`, `*.ws.yaml`, `*.grpc.yaml`), folders may have `_folder.yaml`, environments live in `environments/*.yaml`.
- Secrets are never in files: environment files list secret names; values live in the OS keychain (or env vars in CI).
- Stable key ordering, defaults omitted, unknown keys preserved, versioned with `odb: <n>`; JSON Schemas published.

Full spec: [11-api-client.md §3](../11-api-client.md#3-collection-file-format).

## Consequences

- ✅ Small, readable diffs; parallel edits rarely conflict.
- ✅ Hand-editable; works with any editor and code review tool.
- ✅ Works offline and with any git host.
- ⚠️ YAML pitfalls (implicit typing such as `no` → false): serializer always quotes ambiguous scalars; loader uses YAML 1.2 core schema.
- ⚠️ Many small files for large collections — acceptable; the app indexes them with a file watcher.

## Alternatives considered

- **Single JSON file per collection:** simple, poor diffs and conflicts.
- **Custom DSL (like `.bru`):** compact and readable, but needs its own parser/tooling ecosystem; YAML gets editor support and schemas for free.
- **`.http` files:** great for simple requests, too limited for auth, scripts and tests; supported as import (later).
