# 0005 — Apache-2.0 licence and DCO

- Status: Accepted
- Date: 2026-10-05; reviewed 2026-10-07 (M0-11)

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

## Validation / evidence — 2026-10-07 (M0-11 review)

Accepted; no spike applies. The decision is in effect in the repository:

- Licence: `LICENSE` (Apache-2.0 text) and `NOTICE` at the root; `license = "Apache-2.0"` in the workspace `Cargo.toml` and the root `package.json`; every Rust and TypeScript source file outside generated code carries `SPDX-License-Identifier: Apache-2.0` (261 files checked on 2026-10-07).
- Dependency allowlist: `deny.toml` `[licenses] allow` applies the [22 §2](../22-governance-and-licensing.md#2-dependency-licence-policy) allowlist to Rust crates and denies every other licence; `just deny` (`cargo deny --locked check`) runs in `just check`.
- DCO: [CONTRIBUTING.md](../../CONTRIBUTING.md) requires a sign-off (`commit -s`); all 19 commits on `main` up to #27 carry a `Signed-off-by` trailer.

Not automated yet: the CI DCO check and the npm licence check described in [22](../22-governance-and-licensing.md). Both belong to the full CI pipeline restored by X-01 before the repository goes public. Until then the sign-off is checked in review.
