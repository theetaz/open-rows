# Security Policy

OpenRows handles database credentials and production data, so we treat security reports as a top priority.

## Reporting a vulnerability

- **Do not open a public issue.**
- Report privately through GitHub Security Advisories ("Report a vulnerability" on the repository's Security tab).
- Include affected version, platform, reproduction steps, and impact. Redact real credentials and data.

## What to expect

| Step | Target |
| --- | --- |
| Acknowledgement | within 72 hours |
| Initial assessment and severity | within 7 days |
| Fix or mitigation for high/critical issues | as fast as possible; coordinated with the reporter |
| Public disclosure | after a fix ships, or 90 days after the report, whichever is first, unless agreed otherwise |

Reporters are credited in the release notes unless they ask not to be.

## Scope

In scope: the desktop app, the `openrows` CLI, the driver host, the MCP server, update and signing infrastructure, and the collection/workspace file parsers.

Out of scope: vulnerabilities in databases, APIs or third-party tools the app connects to (report those upstream), and issues requiring a compromised local user account.

## Supported versions

Before 1.0, only the latest release on each channel (stable, beta) receives security fixes.

Design-level security details: [docs/12-security.md](docs/12-security.md).
