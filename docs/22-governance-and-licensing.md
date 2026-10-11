# 22 — Governance and Licensing

## 1. Licence

- The project is licensed under the **Apache License 2.0**: permissive, business-friendly, includes an explicit patent grant and patent-retaliation clause.
- Every source file carries an SPDX header: `// SPDX-License-Identifier: Apache-2.0`.
- `LICENSE` (full Apache-2.0 text) and `NOTICE` live at the repository root (to be added with the code skeleton in M0).
- Third-party notices are generated at build time (`cargo-about` for Rust, `license-checker`/`rollup-plugin-license` for npm) and shown in the app's About → Licences screen.

## 2. Dependency licence policy

| Category | Policy |
| --- | --- |
| MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, MPL-2.0 (file-level copyleft, unmodified use), OFL-1.1 (fonts), CC0, Python-2.0, NCSA | **Allowed** |
| EPL-2.0, LGPL (dynamic linking only) | **Case-by-case**, requires ADR and maintainer approval (e.g. `elkjs` for ER layout — EPL-2.0; fallback `dagre`, MIT, if not approved) |
| GPL, AGPL, SSPL, BSL/BUSL, Commons Clause, proprietary | **Not allowed** in shipped binaries |
| Proprietary database client libraries (e.g. Oracle Instant Client) | **Never bundled**; loaded at runtime only if the user installs them |

Python-2.0 (PSF licence, permissive and OSI-approved) is allowed for `argparse`, a dependency of `sql-formatter` used only by its command-line tool; it is not bundled into the app.
NCSA (University of Illinois/NCSA licence, permissive and OSI-approved) is allowed for `libfuzzer-sys`, used only by the separate `fuzz/` workspace; it is never built into the app.

Public CA certificates of cloud database services (`crates/openrows-net/certs/`: the Amazon RDS global bundle; `DigiCert` Global Root G2, `DigiCert` Global Root CA and Microsoft RSA Root CA 2017 for Azure) are public trust-anchor data, not code. The providers publish them for anyone to download and embed in clients so TLS can be verified; they carry no licence terms and are redistributed unchanged. Sources, fingerprints and the refresh process are in [docs/24 §13](24-networking-and-protocols.md#13-implementation-notes-m1-06-and-m1-07-completion).

Enforced by `cargo-deny` (`[licenses]` allowlist) and an npm licence check in CI.


### Driver-host dependencies (M2-01a)

| Dependency | Purpose | Licence | Alternatives / scope |
| --- | --- | --- | --- |
| `postcard` 1.1.3 (`alloc`, default features off) | Bounded protocol-1.0 control/bootstrap serialization | MIT OR Apache-2.0 | Selected by ADR-0010; bincode offers no advantage for lockstep binaries, protobuf adds schema/codegen maintenance, JSON weakens compact framing. |
| `cobs` 0.3, `embedded-io` 0.4/0.6 | Postcard's transitive codec/I/O dependencies | MIT OR Apache-2.0 | No direct application API or extra serialization format. |

`socket2` 0.6 is an additional direct reuse (MIT OR Apache-2.0) for safe Unix socket-buffer configuration. Profiling found repeated 8 KiB macOS data-socket stalls; bounded 512 KiB buffers per direction avoid those stalls without shared-memory transport or raw platform FFI. Standard Tokio exposes no socket-buffer setter.

Host crates reuse existing serde, thiserror, Tokio, Arrow, zeroize, ring, tempfile and platform dependencies (`libc`, `rustix`, `windows-sys`, `socket2`). Criterion, proptest and flatbuffers are existing test/benchmark dependencies. The lifecycle crate retains all workspace quality lints with `unsafe_code = deny` instead of the workspace's `forbid`, allowing narrowly documented macOS/Windows platform FFI modules; no native database dependency enters the protocol/client or desktop through this change. The host's SQLite dependency is an existing driver. DuckDB and packaging/signing dependencies remain separate tasks.

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

### DuckDB dependencies (M2-04a)

| Dependency | Purpose | Licence | Alternatives / scope |
| --- | --- | --- | --- |
| `duckdb` / `libduckdb-sys` 1.10506.0, bundled only | Host-only DuckDB 1.5.6 native engine with reproducible C++ build | MIT | Direct C API would duplicate lifecycle/Arrow binding work; dynamic system linking would require user installs. Optional JSON/Parquet/ICU extensions and default features are not enabled. |
| `arrow-ipc` 58 (renamed `arrow-ipc-58`) and existing workspace version 60 | Bounded native Arrow 58 → workspace Arrow 60 ABI bridge | Apache-2.0 | Upgrading the entire workspace would be unrelated contract churn; manual per-cell reconstruction loses native nested/type fidelity. Version 58 is a host-only feature dependency; version 60 decodes into the shared representation. |
| `same-file` 1.0.6 | Parent-only stable cross-platform existing-file identity, including hard links | MIT OR Unlicense (MIT selected) | Path canonicalization alone misses hard links; bespoke platform FFI adds unsafe maintenance. |
| `arrow-cast` 60 (tests) | Expected-value fidelity formatting through the shared Arrow representation | Apache-2.0 | Formatting SQL strings would not verify values actually received over the host wire. |
| Existing `tauri-build` 2, `proc-macro2` 1 and `syn` 2 (host build-only) | Unify desktop build-dependency features before compiling bundled C++, so staging and workspace checks reuse one native build | MIT OR Apache-2.0 | Sharing the target directory alone still changes the native build-script fingerprint. An empty build entry resolves these features without running Tauri bundling; no runtime use or new locked package/version. Building all workspace targets first would duplicate broader check work. |

The lockfile adds the binding's Arrow 58/native build transitive graph; cargo-deny checks the complete resolved licence/advisory graph. No new npm dependency.

**Owner decision 2026-10-10 — scoped exception approved:** `libduckdb-sys`'s bundled build pulls `webpki-roots` 1.0.9 through its `ureq` build-dependency. These public CA certificates use CDLA-Permissive-2.0 and are build-time-only downloader data, never shipped in the app or helper. `deny.toml` permits this licence only for `webpki-roots` through a [package exception](https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html#exceptions); the global allowlist is unchanged. The exception permits the reproducible bundled native build without replacing the maintained binding or pinning stale trust anchors. Runtime-only dependency graph checks verify that this crate cannot enter either shipped binary; a runtime use would require a new owner decision.

### M45-01 provider transport dependencies

No new third-party packages or TLS stack are added to the lockfile. The new
`openrows-ai` crate directly reuses existing Hyper/Hyper-util/http-body-util
(MIT), bytes/Tokio (MIT), URL (MIT OR Apache-2.0), serde/serde_json/thiserror
(MIT OR Apache-2.0), and optional Specta (MIT) for generated IPC metadata types.
The internal `openrows-net` and `openrows-secrets` crates (Apache-2.0) supply
routes/full TLS and zeroizing keychain credentials. A vendor SDK or reqwest
would duplicate protocol/lifecycle or HTTP/TLS machinery; direct raw TCP would
duplicate the existing parser and TLS security boundary. All selected licences
are on the §2 allowlist; `cargo deny` checks the resolved workspace.

### M45-02 direct dependency reuse

`openrows-ai` adds direct reuse of existing locked `regex` (MIT OR Apache-2.0)
for bounded user glob/regex rules, `sha2` (MIT OR Apache-2.0) for exact context
hashes, and `rusqlite` with the existing bundled feature (MIT) for transactional
local policy/audit storage. Bespoke pattern/hash code risks security bugs;
JSON append files would require custom atomic retention/recovery logic. The
internal `openrows-sql` crate (Apache-2.0) reuses history's lexer/redactor.
Existing `tempfile` (MIT OR Apache-2.0) and `criterion` (MIT OR Apache-2.0) are
test/opt-in benchmark only. No new external package/version or npm dependency.

### X-ECDSA direct dependency reuse

`openrows-net` directly reuses the already locked `p256`, `p384` and `p521`
0.14 packages (Apache-2.0 OR MIT) to validate private scalar ranges and derive
public points when loading OpenSSH ECDSA keys. No new package/version or
cryptographic implementation is introduced. A dependency upgrade still has the
same 32-byte scalar minimum; vendoring the complete SSH library would add a
large maintenance surface. A focused loader uses the existing reexported SSH
encoding, cipher and KDF APIs and keeps RSA/Ed25519 on their prior paths.

The explicit-domain PKCS#8 compatibility path reuses the existing `pkcs8`/`der`
codec (Apache-2.0 OR MIT) and validates against three small public NIST domain
parameter DER files. They are mathematical constants generated by OpenSSL 3,
not private keys or copied implementation code; source and generation commands
are in `crates/openrows-net/src/auth/ecdsa_parameters/README.md`.

### M45-05 SQL admission dependency

`openrows-sql` adds exact-pinned `sqlparser` 0.63.0 (Apache-2.0) with only `std`
and `visitor`; its visitor derive uses `sqlparser_derive` 0.6.0 (Apache-2.0).
Both licences are on §2's allowlist. A maintained dialect AST makes it possible
to inspect every nested query/expression/base relation before admission. Extending
the handwritten single-table token grammar would duplicate dialect parsing and
risk missing nested side effects. The parser is bounded to 16 KiB input,
1,024 significant tokens, 32 nesting levels and a recursion limit of 64; optional default recursive-stack
features and runtime engine libraries are not enabled. No new npm dependency.
