# 0006 — File-per-request YAML collections for the API client

- Status: Accepted — validation deferred (M4-02)
- Date: 2026-10-05; reviewed 2026-10-07 (M0-11)

## Context

Users want API collections versioned in git alongside code, reviewable in PRs, shareable without accounts or cloud sync. Single-file JSON exports produce huge diffs and frequent merge conflicts.

## Decision

- A collection is a **folder**; each request is **one YAML file** (`*.req.yaml`, `*.ws.yaml`, `*.grpc.yaml`), folders may have `_folder.yaml`, environments live in `environments/*.yaml`.
- Secrets are never in files: environment files list secret names; values live in the OS keychain (or env vars in CI).
- Stable key ordering, defaults omitted, unknown keys preserved, versioned with `openrows: <n>`; JSON Schemas published.

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

## Validation / evidence — 2026-10-07 (M0-11 review)

Accepted as a design decision; there is no M0 spike and no code yet. [11 §3](../11-api-client.md#3-collection-file-format) specifies the format and no doc contradicts this ADR. Validation is deferred to:

- **M4-02** (collection format, `openrows-collection`): tests for the format rules above (stable key ordering, omitted defaults, unknown keys preserved, ambiguous scalars quoted, YAML 1.2 core schema, `openrows: <n>` versioning) and for secret values never being written to files.
- **M4-09** (M4 exit, journey 7): import a Postman collection, run it with an environment, tests pass.

If M4-02 shows that YAML cannot meet these rules, this ADR is revised explicitly before M4-03 builds on it.
