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
- A per-connection **data-sharing policy** (off / schema / samples / results) with redaction, provider allowlists and audit log; prod defaults to schema-only.

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
