# OpenRows — Design Documentation

This folder is the source of truth for what we are building and how. Every significant technical decision is recorded here or in an [ADR](adr/README.md) before code depends on it.

Diagrams are written in [Mermaid](https://mermaid.js.org/) so they render on GitHub and stay diffable.

## Reading order

| # | Document | What it answers |
| --- | --- | --- |
| 01 | [Vision and scope](01-vision-and-scope.md) | Why this exists, who it is for, what is in and out |
| 02 | [Requirements](02-requirements.md) | Functional requirements and measurable non-functional targets |
| 03 | [Architecture overview](03-architecture-overview.md) | System context, containers, process model, crate map |
| 04 | [Core engine](04-core-engine.md) | Sessions, actors, query pipeline, value model, cancellation |
| 05 | [Database drivers](05-database-drivers.md) | Driver contract, per-database strategy, support tiers |
| 06 | [IPC and result pipeline](06-ipc-and-result-pipeline.md) | How millions of rows reach the screen without freezing it |
| 07 | [Frontend architecture](07-frontend-architecture.md) | React app structure, state, workers, layout |
| 08 | [Data grid](08-data-grid.md) | The canvas grid: rendering, selection, editing, accessibility |
| 09 | [Query editor](09-query-editor.md) | CodeMirror 6, autocomplete, statement splitting, explain |
| 10 | [Schema and data editing](10-schema-and-data-editing.md) | Introspection, caching, change sets, DDL, safe mode |
| 11 | [API client](11-api-client.md) | Protocols, collection file format, scripting, CLI |
| 12 | [Security](12-security.md) | Threat model, credentials, SSH/TLS, webview hardening |
| 13 | [Performance](13-performance.md) | Budgets, techniques, benchmarks, profiling |
| 14 | [Reliability and observability](14-reliability-and-observability.md) | Failure handling, reconnection, logging, crash reports |
| 15 | [Storage and persistence](15-storage-and-persistence.md) | Local app data, history, Project files, migrations |
| 16 | [Platform and distribution](16-platform-and-distribution.md) | macOS first, then Windows/Linux; signing, updates |
| 17 | [Testing strategy](17-testing-strategy.md) | Unit, conformance, integration matrix, E2E, perf CI |
| 18 | [Repository and dev workflow](18-repository-and-dev-workflow.md) | Monorepo layout, tooling, CI, conventions |
| 19 | [Roadmap](19-roadmap.md) | Milestones from foundations to 1.0 and beyond |
| 20 | [UX foundations](20-ux-foundations.md) | Information architecture and principles for the UI design phase |
| 21 | [Extensibility](21-extensibility.md) | Plugin model for drivers, exporters and themes |
| 22 | [Governance and licensing](22-governance-and-licensing.md) | License, clean-room policy, contribution rules |
| 23 | [AI assistant, agents and MCP](23-ai-assistant.md) | BYOK providers, agent-CLI bridges, MCP server/client, data-sharing policy |
| 24 | [Networking and protocols](24-networking-and-protocols.md) | Routes and hops, SSH, TLS, cloud tunnels, sleep/wake resilience, protocol handling |
| 25 | [Engineering standards and delivery](25-engineering-standards.md) | Task specs, acceptance reports, code-quality limits, performance policy, local green-light gate, clean-up |
| 26 | [Feature inventory and parity plan](26-feature-parity.md) | Public-source TablePlus benchmark, requirement mappings, 1.0 gaps, deliberate deferrals and OpenRows improvements |
| 27 | [Delivery plan](27-delivery-plan.md) | Ordered task backlog from the current state to 1.0, per milestone, with requirement IDs, dependencies, gates and status |
| 28 | [Database monitoring and AI harness](28-database-monitoring-and-ai-harness.md) | Live health/performance, history/alerts, analytics boards/reports, provider choices and evidence-backed AI investigations |
| — | [Glossary](glossary.md) | Terms used across these docs |
| — | [ADRs](adr/README.md) | Architecture Decision Records |

## Conventions

- **MUST / SHOULD / MAY** follow RFC 2119 meaning.
- Numeric targets are measured on the reference machines defined in [13-performance.md](13-performance.md#1-reference-hardware).
- Rust crates use the prefix `openrows-`, npm packages the scope `@openrows/`, and the CLI is `openrows`. In API-client scripts the runtime object is `ors` (with a `pm` compatibility shim).
- When a doc and an ADR disagree, the newer accepted ADR wins and the doc gets fixed.
