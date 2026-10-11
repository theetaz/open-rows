# 0009 — Native AI with BYOK, agent-CLI bridges and MCP

- Status: Accepted — validation deferred (M45-09)
- Date: 2026-10-05; reviewed 2026-10-07 (M0-11)
- Supersedes: AI assistant listed as post-1.0 in [19-roadmap.md](../19-roadmap.md)

## Context

Users want AI help (NL→SQL, explain, fix, optimise) inside the client. They already pay for AI in different ways: API keys with various providers, local models, or subscriptions to AI coding CLIs (Claude Code, Codex CLI, Cursor CLI, Gemini CLI). We must not run an AI service ourselves, and production data must stay protected.

## Decision

- AI is a core 1.0 feature, implemented in `openrows-ai` and `openrows-mcp`.
- Three provider kinds behind one event model: **API keys** (native + OpenAI-compatible adapters), **local models**, and **agent-CLI bridges** that drive the user's installed CLI headlessly, sandboxed to our MCP tools only.
- The app is an **MCP server** (read-only DB tools, `propose_sql` instead of write tools) and an **MCP client** (user-configured servers).
- A per-connection **data-sharing policy** (None / SchemaOnly / SchemaAndStats / SampleRows) with redaction, provider allowlists and audit log; all connections default to None, and production sharing requires explicit confirmation.

Details: [23-ai-assistant.md](../23-ai-assistant.md).

## Consequences

- ✅ Zero AI cost for the project; users reuse what they have.
- ✅ Same UX regardless of provider; MCP makes tools reusable by external agents.
- ✅ Writes are structurally impossible from the model; humans approve everything that mutates.
- ⚠️ CLI bridges depend on third-party CLI flags and terms that change; adapters must be versioned and can be disabled per CLI.
- ⚠️ More surface for data leakage — mitigated by policy defaults, previews and tests on exact payloads.

## Alternatives considered

- **Hosted AI service by the project:** costs money, requires accounts, conflicts with local-first.
- **API keys only:** excludes users who only have CLI subscriptions.
- **Giving the model write tools with confirmation:** rejected; proposals routed through the normal editor + safe-mode path are clearer and auditable.

## Validation / evidence — 2026-10-07 (M0-11 review)

Accepted as a design decision; there is no M0 spike and no code yet. [19](../19-roadmap.md) and [27 §8](../27-delivery-plan.md#8-m45--native-ai-and-mcp) already schedule AI as milestone M4.5, as this ADR requires. Validation is deferred to the M4.5 tasks:

- M45-01 (API-key and local providers) and M45-04 (agent-CLI bridges sandboxed to the app's MCP tools).
- M45-02 (data-sharing policy with golden-payload tests), M45-03 (read-only MCP server with `propose_sql`) and M45-07 (MCP client).
- M45-08 (no write path from model output, tested).
- **M45-09** (M4.5 exit): golden payloads green, no write path, NL→SQL eval baseline per provider.

If a provider kind cannot meet the data-sharing or write-safety gates, this ADR is revised explicitly for that kind.

## Scope and provider eligibility clarification — 2026-10-11

The [monitoring and AI harness specification](../28-database-monitoring-and-ai-harness.md)
adds database health/query investigations, validated report/chart/board proposals
and bounded opt-in in-app scheduled checks. [Accepted ADR-0011](0011-performance-monitor.md) supplies M2.5 source, privacy, budget and advisor contracts; its validation is pending. M2.5 supplies deterministic monitoring
and analytics first; M45-10..12 add the AI path. This preserves the read/propose-only
decision and adds a separate telemetry-sharing permission, evidence references,
usage limits and cancellation on lock or policy revocation. M45-09 validates the
expanded scope; no implementation evidence is claimed here.

Subscription discovery means explicit installed-tool/account probes, not
credential scraping or payment-subscription enumeration. Codex's
[official app-server account APIs](https://learn.chatgpt.com/docs/app-server#authentication-modes)
support managed login and reported plan state, so they are preferred to copying
tokens or inferring authentication from a file.

Claude's [current plan help](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan)
describes subscription usage, while its [SDK product guidance](https://code.claude.com/docs/en/agent-sdk/overview#get-started)
requires prior approval for third-party products offering claude.ai login/rate
limits. A Claude subscription bridge is conditional on that eligibility; the
Anthropic API-key adapter remains the baseline. Other named CLI candidates are
also disabled until their current eligibility and isolation are verified.
An empty working directory or a tool allowlist alone does not prove sandboxing.
No unsupported integration can silently fall back to API billing or another
provider. Exact adapters are revalidated before release.

## Implementation evidence — M45-01 (2026-10-11)

The API-key/local foundation is implemented in `crates/openrows-ai` and Settings
→ AI; [docs/23 §10](../23-ai-assistant.md#10-implemented-m45-01-api-key-and-local-provider-settings)
records transport and secret boundaries. Metadata-only health checks, default-off
permission, cancellation/timeouts, capabilities with unknown limits, keychain-only
keys, no redirects and seven adapter mock matrices are covered by
`cargo nextest run -p openrows-ai`; UI lifecycle and settings-reference tests are
in `apps/desktop/src/features/settings/ai`. OpenAI follows the official Responses
recommendation with storage disabled; compatible gateways use Chat Completions.
No database sharing, model tool execution, CLI bridge or MCP implementation is
claimed. The ADR's policy, write-safety and full integration validation remains
M45-02..09, and live provider access remains a manual opt-in smoke.

## Implementation evidence — M45-02 (2026-10-11)

`openrows-ai::sharing` now owns the default-deny per-connection policy, bounded
SQL/identifier/column redaction, typed aggregate/plan/sample bundles, exact
preview and metadata-only audit. Provider generation requires a private gated
payload; compile-fail doctests reject raw bundles/deserialisation. Production
consent and saved-provider allowlists are native checks; local-only requires a
loopback local protocol, and telemetry is an independent permission. Policy or
provider changes invalidate old payloads; active work cancels on revocation.
The safer default of None for **all** connections supersedes the original
schema-on-production default above, per the M45-02 owner brief.

Golden/refusal/transport/audit tests live in `crates/openrows-ai/tests/sharing*`;
UI editing/consent/preview/purge tests in `features/settings/ai/Sharing.test.tsx`.
[23 §11](../23-ai-assistant.md#11-implemented-m45-02-connection-context-sharing)
records exact caps and local storage. MCP execution, assistant conversations,
account bridges and end-to-end M45-09 validation remain separately scheduled.

## Evidence: Implemented (M45-03/08)

The Tauri-free `openrows-mcp` crate uses the official Apache-2.0-licensed rmcp 3.4.0
for stable MCP 2026-07-28 and legacy negotiation. A spawned stdio CLI forwards
to the running app's explicit loopback port with a scoped bearer; direct HTTP
is independently disabled by default. Hash-only, expiring provider/connection
tokens support revoke/disconnect and immediate cancellation. The existing
sharing gate/redaction/audit is mandatory before releasing database data.
A shared conservative SELECT validator plus disposable engine read protection
prevents writes; proposals have only an editor-queue path. Native lock/off/close
boundaries, protocol/UI tests, persistent state checks for PostgreSQL,
MySQL/MariaDB, SQLite/libSQL file and injection fixtures are implemented.

[23 §12](../23-ai-assistant.md#12-implemented-m45-0308-built-in-mcp-and-execution-safety)
records limits and deliberate fail-closed gaps: no remote listener/write scope,
no external local-only attestation, restricted SELECT syntax and unsupported
remote libSQL/SQL Server/ClickHouse/DuckDB execution. The in-app assistant,
CLI-as-provider adapters and MCP client remain their own delivery tasks.

## Evidence: Implemented (M45-05)

The native assistant uses the existing provider routing/keychain layer and sharing
boundary, with a Tauri-free bounded agent loop and the same executor as MCP.
Seven native protocol mock round trips preserve exact golden context and keep tool
results in data roles. Explicit tool capability metadata gates the finite loop;
unknown models use single-shot schema context. The lazy per-tab dock streams text,
shows tool evidence and returns editor-only SQL proposals. Limits/cancellation,
injection/refusal, no-write engine fixtures, follow-up redaction and UI actions are
covered by the suites linked in [23 §13](../23-ai-assistant.md#13-implemented-m45-05-query-assistant-with-visible-tool-trace).

The positive AST subset admits joins, non-modifying CTEs/subqueries, aggregates,
windows and UNION; all base tables undergo live safety checks. Unknown functions
remain refused on PostgreSQL, MySQL/MariaDB and SQLite; ambiguous sample provenance
is fully redacted. No dependency on provider accounts/CLI bridges is introduced.
Live smoke is opt-in, synthetic and requires the owner's real key; it was not run.
