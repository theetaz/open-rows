# 0005 — Apache-2.0 licence and DCO

- Status: Accepted
- Date: 2026-10-05

## Context

We want maximum adoption by individuals and companies, an explicit patent grant, and low friction for contributors.

## Decision

- License the project under **Apache License 2.0**.
- Require **DCO sign-off** on commits instead of a CLA.
- Enforce a dependency licence allowlist ([22-governance-and-licensing.md](../22-governance-and-licensing.md)).

## Consequences

- ✅ Permissive and widely understood; patent grant protects users and contributors.
- ✅ Companies can adopt and contribute without legal review hurdles typical of copyleft.
- ⚠️ Others may build proprietary forks. Accepted trade-off; the community edition stays free and full-featured.

## Alternatives considered

- **MIT:** simpler, but no explicit patent grant.
- **GPL-3.0 / AGPL:** guarantees forks stay open, but deters corporate contributors and complicates some dependency choices.
- **MPL-2.0:** file-level copyleft middle ground; less familiar to many contributors.
