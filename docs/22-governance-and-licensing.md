# 22 — Governance and Licensing

## 1. Licence

- The project is licensed under the **Apache License 2.0**: permissive, business-friendly, includes an explicit patent grant and patent-retaliation clause.
- Every source file carries an SPDX header: `// SPDX-License-Identifier: Apache-2.0`.
- `LICENSE` (full Apache-2.0 text) and `NOTICE` live at the repository root (to be added with the code skeleton in M0).
- Third-party notices are generated at build time (`cargo-about` for Rust, `license-checker`/`rollup-plugin-license` for npm) and shown in the app's About → Licences screen.

## 2. Dependency licence policy

| Category | Policy |
| --- | --- |
| MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0 (file-level copyleft, unmodified use), OFL-1.1 (fonts), CC0 | **Allowed** |
| EPL-2.0, LGPL (dynamic linking only) | **Case-by-case**, requires ADR and maintainer approval (e.g. `elkjs` for ER layout — EPL-2.0; fallback `dagre`, MIT, if not approved) |
| GPL, AGPL, SSPL, BSL/BUSL, Commons Clause, proprietary | **Not allowed** in shipped binaries |
| Proprietary database client libraries (e.g. Oracle Instant Client) | **Never bundled**; loaded at runtime only if the user installs them |

Enforced by `cargo-deny` (`[licenses]` allowlist) and an npm licence check in CI.

## 3. Clean-room policy

The project is inspired by the database-client category but must be original.

**Contributors MUST NOT:**
- Copy, decompile, disassemble or reverse-engineer proprietary applications (including their binaries, private APIs or file formats beyond documented import/export formats).
- Copy proprietary source code, icons, illustrations, screenshots, sounds, UI text, documentation text, or themes.
- Submit code they do not have the right to contribute (e.g. employer-owned without permission).

**Contributors MAY:**
- Use publicly observable product behaviour as general inspiration for features and UX patterns (e.g. "staged edits with commit preview" is a general idea, not protected expression).
- Read and learn from **open-source** projects with compatible licences; any code reused must keep its licence notice and be recorded in third-party notices. Code from incompatible licences (GPL etc.) must not be copied.
- Implement documented import formats (Postman collections, HAR, OpenAPI, etc.) from their public specifications.

Trademarks: database product names are used nominatively ("connects to PostgreSQL"). Vendor logos used only where the vendor's trademark guidelines allow; otherwise neutral original glyphs.

## 4. Contributions

- **DCO** (Developer Certificate of Origin) sign-off on every commit (`Signed-off-by:`), checked by a CI bot. No CLA, to keep contribution friction low.
- `CONTRIBUTING.md`: setup, conventions ([18-repository-and-dev-workflow.md](18-repository-and-dev-workflow.md)), how to propose an ADR, how to add a driver.
- `CODE_OF_CONDUCT.md`: Contributor Covenant 2.1.
- `SECURITY.md`: private vulnerability reporting via GitHub Security Advisories; acknowledgement within 72 h; coordinated disclosure.
- Issue templates: bug (with diagnostics bundle guidance), feature request, driver request, performance report.
- Labels: `good first issue`, `help wanted`, `driver:<name>`, `area:<grid|editor|api|…>`, `p0/p1/p2`, `security`, `data-integrity`.

## 5. Governance model

- **Phase 1 (pre-1.0): BDFL + maintainers.** Project founder has final say; maintainers own areas (core, drivers, UI, API client, release).
- **Phase 2 (post-1.0): maintainer council.** Decisions by lazy consensus; disputes resolved by majority vote of maintainers. Area owners listed in `MAINTAINERS.md`; `CODEOWNERS` enforces reviews.
- Becoming a maintainer: sustained, high-quality contributions and reviews over ~3 months, nominated by an existing maintainer.
- Roadmap is public (GitHub Projects); RFCs for large features via discussions + ADR.

## 6. Sustainability

- Always free and fully open source — no "pro" tier, no feature paywalls, no open-core split.
- Funding via GitHub Sponsors / Open Collective for infrastructure (CI Macs, code-signing certificates, Apple Developer membership), with transparent spending.
- Corporate sponsors may fund work but get no special licensing rights.
