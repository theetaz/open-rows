# 12 — Security

A database client holds the keys to production. Security failures here are catastrophic for users, so the bar is higher than for a typical desktop app.

## 1. Assets

| Asset | Where it lives |
| --- | --- |
| Database passwords, SSH key passphrases, tokens, API secrets, OAuth tokens | OS keychain only |
| Connection profiles (hosts, users, options) | App data dir (JSON) / workspace files (shared definitions, no secrets) |
| Query text and history | Local store (SQLite) |
| Result data | Memory, spill temp files |
| API collections and responses | Workspace folder / local store |
| Update signing private key | Offline/HSM-backed CI secret, never on dev machines |

## 2. Threat model (STRIDE summary)

```mermaid
flowchart LR
    subgraph trust_local["Trusted: local user session"]
        UI["Webview UI"]
        Core["Rust core"]
        KC[("Keychain")]
    end
    subgraph semi["Semi-trusted inputs"]
        WSF["Workspace files from git<br/>(collections, saved queries)"]
        DBData["Data returned by databases"]
        APIResp["API responses"]
    end
    subgraph untrusted["Untrusted network"]
        Net["Network path<br/>(Wi-Fi, proxies)"]
        Upd["Update feed"]
    end
    WSF -->|parse, scripts sandboxed| Core
    DBData -->|rendered as text/canvas, never HTML| UI
    APIResp -->|sandboxed preview| UI
    Core -->|TLS / SSH| Net
    Core -->|signature verified| Upd
    Core <--> KC
    UI <-->|capability-scoped IPC| Core
```

| Threat | Example | Mitigation |
| --- | --- | --- |
| **Spoofing** server | MITM on café Wi-Fi impersonates DB or SSH server | TLS verify-full by default with OS trust store (`rustls-platform-verifier`); SSH host key check against `known_hosts` with TOFU prompt showing SHA256 fingerprint; changed host key = hard stop with explanation. |
| **Tampering** with updates | Malicious update payload | Tauri updater signature verification (Ed25519/minisign) + platform code signing + notarization; HTTPS-only feed; pinned public key in the binary. |
| **Tampering** via workspace files | Malicious collection in a cloned repo runs scripts | Scripts sandboxed (no FS/process); untrusted collections open in restricted mode (scripts disabled) until user trusts the folder; `ors.db.query` off by default. |
| **Repudiation** | "I didn't run that DELETE" | Local history records every executed statement with timestamp and connection (user-controllable). |
| **Information disclosure** — XSS in webview | DB cell contains `<script>` or a crafted SVG | Cells rendered on canvas or as text nodes, never `innerHTML`; strict CSP; HTML/SVG/PDF previews only in sandboxed iframes without script permission and with an opaque origin. |
| **Information disclosure** — logs | Password or query values in logs | Structured logging with redaction layer: known secret fields never logged; query text logged only at `trace` level and off by default; connection URLs redacted. |
| **Information disclosure** — crash reports | Data in panic messages | Opt-in only; reports exclude query text, values, hosts by default; user can review the payload before sending. |
| **Denial of service** | Huge result exhausts RAM | Result store budget + spill + row cap; response body size caps; script time/memory limits. |
| **Elevation of privilege** — webview → native | Compromised UI calls dangerous commands | Tauri 2 capabilities: per-window allowlist of commands; no shell/FS plugins exposed to the webview beyond scoped dialogs; inputs validated in Rust. |
| **Supply chain** | Malicious crate/npm package | Lockfiles committed; `cargo-deny` (advisories, licences, bans, sources), `cargo-audit`, `cargo-vet` imports for critical crates; `pnpm audit`, no install scripts by default (`pnpm` `onlyBuiltDependencies` allowlist); GitHub Actions pinned by SHA; SBOM per release. |

## 3. Credentials

```mermaid
sequenceDiagram
    participant UI
    participant Core
    participant KC as OS Keychain

    UI->>Core: save profile {host, user, password: "•••"}
    Core->>KC: set(service="com.prabhavalabs.openrows", account="conn/<profileId>/password", secret)
    Core->>Core: write profile JSON with passwordRef = "keychain:conn/<id>/password"
    Note over UI: The webview never receives the stored password back
    UI->>Core: connect(profileId)
    Core->>KC: get(...)
    KC-->>Core: secret (zeroized after use)
    Core->>Core: authenticate
```

- Secret values held in `secrecy::SecretString` / `zeroize` types; never `Debug`-printed.
- "Ask every time" option per secret: not stored at all; prompted on connect and held only for the session lifetime.
- macOS: Keychain items created with access restricted to our signed app (designated requirement), so other apps get a system prompt. Optional Touch ID gate for prod connections via `LocalAuthentication` (P1, through `security-framework`/objc bindings).
- Linux without Secret Service: fall back to an encrypted file vault unlocked by a user passphrase (Argon2id + XChaCha20-Poly1305), with a clear warning.
- Export of profiles never includes secrets unless the user explicitly chooses "include secrets" and sets an export passphrase (same vault format).

## 4. Network security

### 4.1 TLS

- `rustls` everywhere; minimum TLS 1.2; system roots plus user-added CAs per connection.
- Modes exposed per connection (mapped to each driver's terms): `disable`, `prefer`, `require`, `verify-ca`, `verify-full` (default where the database supports TLS and the host isn't localhost; PostgreSQL `sslmode` semantics are matched exactly for PG).
- "Skip verification" is a separate explicit toggle that shows a persistent warning badge on the connection.
- Client certificate auth with key material from files or keychain.

### 4.2 SSH tunnels

- `russh` client: auth via agent (macOS launchd agent, 1Password/Secretive agents via `SSH_AUTH_SOCK`), key files (OpenSSH, PEM, PKCS#8; ed25519/ecdsa/rsa-sha2), password, keyboard-interactive (for 2FA prompts).
- Jump hosts (`ProxyJump`) chained; `~/.ssh/config` parsing for `Host`, `HostName`, `User`, `Port`, `IdentityFile`, `ProxyJump`, `IdentitiesOnly`.
- Local listener bound to `127.0.0.1` on an ephemeral port, or (preferred) **in-process stream** passed directly to the driver when the driver accepts a custom stream (avoids exposing a local port to other local processes). Drivers that need a socket address get the loopback listener, accepting only the one expected connection.
- Host key verification against `~/.ssh/known_hosts` plus app-managed known hosts; never auto-accept changed keys.

### 4.3 Proxies

SOCKS5 and HTTP CONNECT proxies supported for both database and API connections; system proxy detection for API requests.

## 5. Webview hardening (Tauri 2)

- `frontendDist` bundled; no remote URLs loaded in app windows.
- CSP: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'` (inline styles needed for CodeMirror; no inline scripts); `connect-src ipc: http://ipc.localhost`; `img-src 'self' data: blob:`; `frame-src` limited to sandboxed blob previews.
- Tauri **isolation pattern** enabled to validate IPC messages in an isolated iframe.
- Capabilities file per window kind; plugins limited to: dialog (scoped), clipboard, updater, window-state, deep-link (for OAuth redirect), single-instance.
- External links open in the system browser after URL scheme validation (`https`, `http`, `mailto` only).
- OAuth 2.0 redirects use a loopback listener with PKCE, or the system browser + `openrows://` deep link; never an embedded login webview (users should see the real browser URL bar).

## 6. Data at rest

- Local store is a plain SQLite DB in the user's app data dir (protected by OS account permissions and FileVault/BitLocker). It contains history and cached schema metadata — potentially sensitive.
- Per-connection "don't record history" and "don't cache schema" toggles; global "clear all history".
- Optional encrypted local store (SQLCipher-compatible build) is a P2 item.
- Spill files and response temp files are created with `0600` permissions in a per-user temp dir and deleted on close; startup sweeps leftovers.

## 7. Secure development lifecycle

- Threat model reviewed for every feature touching credentials, IPC, scripts, network, or file parsing.
- Fuzzing: SQL splitter, MQL parser, collection file parser, importers (Postman/HAR/OpenAPI), protocol decoders (`cargo-fuzz`, run nightly).
- `unsafe` Rust forbidden outside designated FFI crates (`#![forbid(unsafe_code)]` elsewhere); FFI crates require two reviewers.
- Security policy (`SECURITY.md`) with private disclosure channel and 90-day disclosure timeline.
- Releases built only in CI from tagged commits; reproducibility checks on Linux builds where feasible.
