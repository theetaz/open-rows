# 24 — Networking and Protocol Handling

Every feature of a database or API client sits on top of the network layer. Users connect through bastions, VPNs, proxies, cloud IAM tunnels, flaky Wi-Fi and laptop sleep cycles. This layer must be **correct, observable and resilient**, and identical for databases, API requests and AI providers.

## 1. Layering

```mermaid
flowchart TB
    subgraph consumers["Consumers"]
        drv["Database drivers<br/>(PG, MySQL, TDS, Mongo, Redis, CQL...)"]
        http["API engine<br/>(HTTP/1.1, HTTP/2, WS, SSE, gRPC)"]
        ai["AI providers / MCP HTTP"]
    end
    subgraph net["openrows-net"]
        route["Route planner<br/>(profile → ordered hops)"]
        hops["Hop implementations"]
        tls["TLS (rustls)"]
        dns["Resolver"]
        mon["Path monitor<br/>(network change, sleep/wake)"]
        diag["Diagnostics & timing"]
        pool["Tunnel pool<br/>(shared SSH sessions)"]
    end
    drv --> route
    http --> route
    ai --> route
    route --> hops
    hops --> dns
    hops --> tls
    hops --> pool
    mon --> pool
    hops --> diag
```

Drivers never open sockets themselves. They ask `openrows-net` for a **stream** (`AsyncRead + AsyncWrite`) to a logical endpoint; `openrows-net` builds it from the profile's route.

## 2. Routes and hops

A route is an ordered list of hops resolved from the connection profile:

| Hop | Purpose | Implementation |
| --- | --- | --- |
| Direct TCP | Default | tokio `TcpStream`, Happy Eyeballs (RFC 8305) for dual-stack hosts |
| Unix socket / named pipe | Local PG/MySQL/Redis sockets | tokio `UnixStream` / Windows named pipes |
| SOCKS5 proxy | Corporate / dev proxies | Own client; remote DNS option |
| HTTP CONNECT proxy | Corporate proxies, incl. auth | Own client; Basic/NTLM (later) auth |
| SSH tunnel | Bastions | `russh` `direct-tcpip` channel; multi-hop `ProxyJump` chains |
| AWS SSM port forwarding | Private RDS/EC2 without bastion | Spawn user's `aws ssm start-session` (Session Manager plugin) to a local port, managed lifecycle |
| Kubernetes port-forward | DBs inside clusters | Spawn `kubectl port-forward` (user's kubeconfig/context) or native API later |
| Cloud SQL / AlloyDB connector | GCP IAM auth | Spawn user's `cloud-sql-proxy` / `alloydb-auth-proxy`, or native connector later |
| Cloudflare Access / Tunnel | Zero-trust DBs | Spawn user's `cloudflared access tcp` |
| Custom command | Anything else (Teleport `tsh proxy db`, Tailscale, internal tools) | User-defined command template with `{local_port}` placeholder; health-checked |
| TLS | Encryption + auth | rustls with platform verifier, SNI, ALPN, mTLS |

```mermaid
flowchart LR
    App["Driver"] --> T1["TLS"]
    T1 --> S2["SSH hop 2<br/>db-bastion.internal"]
    S2 --> S1["SSH hop 1<br/>jump.example.com"]
    S1 --> P["SOCKS5 proxy"]
    P --> Net(("Internet"))
```

Rules:

- Spawned helper processes (SSM, kubectl, cloud proxies, custom) are **children of the app**, bound to `127.0.0.1` on an ephemeral port, health-checked before the driver connects, restarted on failure, and killed when the last session using them closes or the app quits.
- The route and each hop's state are visible in the connection's status popover ("Route: SOCKS5 → jump.example.com → db-bastion → TLS 1.3 → PostgreSQL").

## 3. SSH in depth

- **Session sharing (multiplexing):** all sessions of a profile (metadata + per-tab sessions) share one SSH connection with multiple channels, like OpenSSH `ControlMaster`. Saves handshakes, MFA prompts and bastion connection limits.
- **Auth:** agent (`SSH_AUTH_SOCK`, macOS launchd agent, 1Password/Secretive), key files (OpenSSH/PEM/PKCS#8, ed25519/ecdsa/rsa-sha2-256/512), FIDO2 `sk-` keys via agent, password, keyboard-interactive (MFA/OTP prompts surface as native dialogs), certificates (`*-cert.pub`).
- **Config:** parse `~/.ssh/config` (`Host`, `Match host`, `HostName`, `User`, `Port`, `IdentityFile`, `IdentitiesOnly`, `ProxyJump`, `ServerAliveInterval`, `Include`); profile fields override.
- **Host keys:** `known_hosts` (hashed entries supported) + app store; TOFU dialog with SHA256 fingerprint; changed key = hard stop.
- **Keepalive:** SSH-level keepalive (default 30 s) independent of DB keepalive.
- **Reconnect:** if the SSH transport drops, all its channels' sessions move to `Reconnecting`; the tunnel is re-established once and sessions reattach.

## 4. TLS

- `rustls` + `rustls-platform-verifier` (macOS Security framework, Windows CryptoAPI, Linux system roots).
- Per-connection: mode (`disable` … `verify-full`), custom CA bundle, client cert/key (files or keychain), SNI override, minimum version, optional certificate pinning (SHA-256 SPKI).
- Protocol-specific TLS negotiation handled by drivers: PG `SSLRequest` and direct TLS (PG 17 `sslnegotiation=direct`), MySQL capability-flag upgrade, TDS pre-login encryption negotiation, Mongo/Redis/HTTP direct TLS.
- Cloud CA bundles shipped for convenience (AWS RDS, Azure, GCP Cloud SQL server CAs) and updatable, so `verify-full` works out of the box.
- Certificate details visible in the status popover; expiring/invalid certificates produce actionable errors.

## 5. DNS

- System resolver by default (respects VPN/split-DNS on macOS).
- Optional per-profile host override (`/etc/hosts`-like) and resolver choice (system / DoH) for API requests.
- DNS SRV for `mongodb+srv://`; TXT options parsing.
- Resolution results and timing shown in diagnostics.

## 6. Network change, sleep and wake

```mermaid
stateDiagram-v2
    [*] --> Online
    Online --> Changed: interface change / VPN up-down
    Online --> Asleep: system will sleep
    Asleep --> Waking: system did wake
    Waking --> Probing
    Changed --> Probing
    Probing --> Online: probes ok
    Probing --> Reconnecting: probes fail
    Reconnecting --> Online: tunnels + idle sessions restored
    Reconnecting --> Offline: no route
    Offline --> Probing: path available
```

- macOS: `NWPathMonitor` (Network framework) + `NSWorkspace` sleep/wake notifications via `openrows-platform`; Windows: Network List Manager + power events; Linux: NetworkManager D-Bus / netlink. Implemented so far (M1-08, §14): a portable monitor in `openrows-net` that infers wake from a monotonic-clock gap and network changes from the default route's local address.
- On wake or path change, the app **proactively probes** sessions instead of waiting for TCP timeouts (which can take minutes), reconnecting idle sessions and tunnels immediately.
- Busy/in-transaction sessions are never silently replayed (see [14-reliability-and-observability.md](14-reliability-and-observability.md)).
- UI: connection health dot in the rail and context bar (green / amber reconnecting / red offline) with last-latency.

## 7. Protocol handling

| Protocol | Notes |
| --- | --- |
| PostgreSQL v3 | Extended query protocol, binary format, pipelining for metadata queries, `COPY` in/out streaming, notices, `LISTEN/NOTIFY` viewer, cancel via cancel key on a separate connection (through the same route) |
| MySQL | Compression (zlib/zstd) negotiated when bandwidth is low, `caching_sha2_password` with TLS or RSA key exchange, multi-result sets, `LOAD DATA LOCAL` only on explicit user import |
| TDS (SQL Server) | Pre-login, encryption modes, MARS off, attention-based cancel, `GO` batches split client-side |
| MongoDB OP_MSG | SRV discovery, replica-set/sharded topology monitoring, compression (zstd/snappy/zlib), read preference per connection |
| Redis RESP2/3 | Cluster slot routing, Sentinel discovery, pub/sub on dedicated connection, TLS |
| CQL | Token-aware routing, paging state, compression |
| ClickHouse HTTP | `ArrowStream` output, HTTP compression, `query_id` for cancel |
| HTTP/1.1, HTTP/2 | ALPN, connection pooling per origin, HTTP/2 multiplexing, gzip/br/zstd |
| WebSocket | RFC 6455, permessage-deflate, ping/pong |
| SSE | Reconnect with `Last-Event-ID` |
| gRPC | HTTP/2, reflection, deadlines, compression |
| HTTP/3 (QUIC) | Post-1.0 for API client (`quinn`/`h3`) |

**Adaptive behaviour:** measured RTT and throughput per route feed the driver: first-batch size, prefetch depth and compression are tuned (high latency → larger batches + pipelined metadata; low bandwidth → compression on).

## 8. Diagnostics

- **Test connection** walks every hop with timings (see [14-reliability-and-observability.md](14-reliability-and-observability.md#5-connection-diagnostics)).
- **Network inspector** (developer panel): per-session bytes in/out, round trips, latency histogram, TLS details, route.
- **API timing waterfall**: DNS, connect, TLS, TTFB, download — shared implementation.
- Optional **packet-free protocol trace** (logged protocol messages without values) for bug reports.

## 9. Security notes

- Local listeners only on loopback, ephemeral ports, single expected client where possible; prefer in-process streams (no listener at all).
- Helper commands are executed without a shell (argument vectors, no interpolation of untrusted strings); custom command templates show the program and literal template arguments in a purpose-specific Trust and run confirmation before first use; Test connection reports the substituted vector.
- Proxy and SSH credentials live in the keychain like all secrets.

## 10. Testing

- toxiproxy scenarios: latency, jitter, bandwidth caps, resets mid-stream, black-holed packets (half-open), DNS failures.
- SSH matrix: OpenSSH container with key/password/MFA/cert auth, 2-hop jump, host-key change.
- Sleep/wake and network-change simulations through `openrows-platform` test hooks.
- Helper-process lifecycle tests (crash, slow start, port conflict) with fake binaries.

## 11. Implementation notes (M0-S4 SSH spike)

The spike in `crates/openrows-net` proves `russh` for the SSH hop. It is a prototype of §3, not the M1-06 route builder: no multiplexing pool, reconnect, keepalive policy beyond russh's own, or non-SSH hops yet.

| Area | What the spike does |
| --- | --- |
| Library | `russh` 0.64 with the `ring` backend and **without** its `rsa` feature: the `rsa` crate carries RUSTSEC-2023-0071 (Marvin timing side channel, no fix). RSA *user* keys are signed by ring (constant time, `rsa-sha2-256/512`; SHA-1 `ssh-rsa` refused). RSA *host* keys are not offered during negotiation because verifying them needs the `rsa` crate; ed25519 and ECDSA host keys are. |
| Stream | `SshTunnel::open_stream(host, port)` returns a `TunnelStream` (`AsyncRead + AsyncWrite`) over a `direct-tcpip` channel; no local listener (docs/12 §4.2). |
| Hops | `SshTunnel::connect(&[SshHop])`: TCP to the first hop, each later hop over a channel of the previous one. `resolve_route` expands `ProxyJump` like OpenSSH (first jump's own `ProxyJump` applies, later ones are overridden; ≤ 8 hops, loops rejected). `SshHop::from_settings` uses OpenSSH's method order: agent (unless `IdentitiesOnly`), each `IdentityFile`, keyboard-interactive. |
| Auth | Password, key files (OpenSSH ed25519/ECDSA/RSA incl. passphrase-encrypted; unencrypted PEM: PKCS#1 RSA, SEC1 ECDSA; unencrypted PKCS#8 RSA; other formats get an error suggesting `ssh-keygen -p`), ssh-agent (`SSH_AUTH_SOCK` or explicit socket), keyboard-interactive via the `KeyboardInteractivePrompter` callback. Partial success (key then OTP) continues with the next method. Local method failures (missing agent, locked key) do not stop later methods; if nothing succeeds the first one is returned as a typed error (`PassphraseRequired`, `AgentNotConfigured`, …), otherwise `AuthFailed`. |
| Secrets | `SecretString` (zeroized on drop, redacted `Debug`, no `Display`); key file text and DER are zeroized. Once handed to russh (password, OTP answers) the copies are russh's to manage. |
| `~/.ssh/config` | Pure parser over a `ConfigSource` (fuzz-friendly, property-tested): `Host` patterns with `*`, `?`, `!`; `HostName` (`%h`, `%%`), `User`, `Port`, `IdentityFile` (accumulates, `~` expanded), `IdentitiesOnly`, `ProxyJump` (`none`, `user@host:port`, `[v6]:port`, `ssh://`), `Include` (relative to `~/.ssh`, globs, OpenSSH activation rules, depth 16). First value wins. `Match` blocks never apply; other keywords are ignored. |
| Host keys | Pure `known_hosts` checker: plain, wildcard/negated and hashed (`\|1\|salt\|hmac`) hosts, `[host]:port`, `@revoked` (hard stop), `@cert-authority` skipped, malformed lines skipped. Outcome `Trusted` / `Unknown { SHA256 fingerprint, other algorithms }` / `Changed` (hard stop, never reaches the UI) / `Revoked`. Unknown keys go to the `HostKeyPolicy` callback (`Reject`, `AcceptOnce`, `AcceptAndRemember`). The user's `known_hosts` is read-only; accepted keys are appended (0600) to an app-managed file. Known algorithms for a host are preferred during negotiation, as OpenSSH does. |
| Timeouts | docs/14 §4 per stage: DNS 5 s, TCP 10 s, handshake + auth 20 s excluding time spent in host-key and keyboard-interactive prompts, channel open 10 s, keepalive 30 s. Errors name the stage. |
| Tests | Unit and property tests for parsing, matching, timeouts, key formats and ring signatures; `just test-ssh` runs the OpenSSH container matrix (`crates/openrows-net/tests/ssh_container`): password + first use + hashed `known_hosts`, eight key formats incl. encrypted, agent, keyboard-interactive, key + OTP, wrong credentials, rejected and changed host keys, and a 2-hop jump on a Docker network built from an SSH config alias. Keys, passwords and the agent are created at test time. Tests skip when Docker is unavailable (`OPENROWS_REQUIRE_DOCKER=1` fails instead). |
| Manual probe | `cargo run -p openrows-net --example ssh_probe -- <alias> [<host> <port>]` resolves an alias from `~/.ssh/config`, asks on the terminal for first-use and keyboard-interactive answers, prints per-hop timings and optionally opens a tunnel. |

Measured on the container matrix (debug build, Docker Desktop on macOS, loopback): handshake + auth ≈ 40–60 ms per hop (≈ 0.6 s with an encrypted key, dominated by bcrypt-pbkdf in a debug build); the first channel open after authentication ≈ 42 ms on every hop, later opens ≈ 0.3–2 ms; echo round trip ≈ 0.3–2 ms. The OpenSSH client (`ssh -W`) shows the same ≈ 42 ms first-open delay against this server, so it is server-side behaviour, not russh.

Follow-ups for M1-06: session multiplexing and reconnect (§3), RSA host-key verification without the `rsa` crate (or an advisory fix upstream), host and user certificates, `Match`, `%` tokens beyond `%h`, passphrase-encrypted PEM (PKCS#1/PKCS#8) and PKCS#8 ECDSA keys, passphrase prompts through the prompter, Windows agents (Pageant / named pipe), Happy Eyeballs for the first hop.

## 12. Implementation notes (D-10 SSH routes, D-11 TLS)

D-10 and D-11 promote the spike into the connect path of the desktop app for PostgreSQL. This is the dogfood slice of M1-06/M1-07, not the full route builder.

| Area | What is implemented |
| --- | --- |
| Planner | `plan_route(SshRouteSpec, PlanContext)` turns a profile's `ssh` settings into hops: the host may be a `~/.ssh/config` alias (its `HostName`, `User`, `Port`, `IdentityFile`, `IdentitiesOnly`, `ProxyJump` apply); the profile's port, user and jump hosts override them. Destination auth: password (+ keyboard-interactive for MFA), key file (+ passphrase, then keyboard-interactive), agent (then the alias's identity files) or keyboard-interactive only. Jump hosts authenticate the OpenSSH way and also try the profile's key file. |
| Streams | Drivers get a `Dialer` (`TunnelDialer`): every `dial()` opens a `direct-tcpip` channel; no local listener. The PostgreSQL driver takes it as `ConnectOptions::via`; cancel requests use the same route. Every stream holds the tunnel, so it closes after the last session using it. |
| Sharing | The app keeps one tunnel per saved connection (`connections/tunnels.rs`): the metadata session, every per-database user session and table-edit sessions share it; concurrent connects wait for one attempt, so prompts appear once. A closed tunnel is replaced on the next connect; editing, deleting or disconnecting a connection stops sharing it. Test connection opens its own throwaway tunnel. |
| Prompts | Unknown host keys and keyboard-interactive challenges are `ssh-prompt-changed` events answered with `ssh_prompt_answer`; unanswered prompts cancel after 5 minutes. Trusted keys go to the app-managed `known_hosts` next to the profiles (0600); `~/.ssh/known_hosts` is read-only. Changed or revoked keys are a hard `hostKey` error that names the file and line; they never reach the UI as a question. |
| TLS | `openrows-net::tls::TlsClient`: rustls (ring provider, TLS 1.2/1.3) with libpq semantics. `disable`/`allow`/`prefer`/`require` do not verify the certificate, except `require` with a CA file, which verifies the chain; `verify-ca` checks the chain against the CA file (required); `verify-full` also checks the host name, against the CA file or the macOS trust store (`rustls-platform-verifier`). Client certificate and key (PKCS#8 incl. encrypted, PKCS#1, SEC1). Over SSH the certificate is checked against the profile's database host, not the tunnel. The driver sends `SSLRequest`, reads exactly one byte (CVE-2021-23222) and retries `allow` with TLS after a rejection. |
| Diagnostics | Test connection reports steps: SSH connect and login per hop, tunnel (or DNS and TCP when direct), TLS handshake (protocol, cipher, certificate subject, verification level; a warning when the certificate of a remote host is not verified or TLS is off), authentication and server version, each timed. The failing step carries the typed error and a hint on what to change. |
| Tests | Unit tests for planning, TLS verification levels and certificate files, diagnostics step mapping and hints, prompt flow and secret handling; `just test-ssh` adds `route::profile_route_dials_streams_through_one_shared_tunnel`; `just test-pg` runs PostgreSQL with TLS on (generated CA, server and client certificates) for every mode, client-certificate login, wrong CA, host-name mismatch and a routed stream. |

Still pending after D-10/D-11 was completed in M1-06/M1-07 (§13). Cloud and zero-trust hops and the status popover are now implemented in §15 (M2-24), including custom commands (§15). Remaining: the spike follow-ups above (RSA host keys, certificates, encrypted PEM keys, passphrase prompts through the prompter).

## 13. Implementation notes (M1-06 and M1-07 completion)

| Area | What is implemented |
| --- | --- |
| Proxies | `openrows-net::ProxySpec`: SOCKS5 (RFC 1928; no authentication or username/password per RFC 1929; host names are sent unresolved as `DOMAIN`, so the proxy resolves them) and HTTP `CONNECT` (optional `Proxy-Authorization: Basic`; the response head is read byte by byte, at most 8 KiB, so no tunnelled byte is consumed). Typed `ProxyError` (`Unreachable`, `AuthRequired`, `AuthFailed`, `Refused` with the SOCKS reply or HTTP status, `Protocol`, `Timeout`) names the proxy, never the password. DNS 5 s, TCP 10 s, proxy handshake 10 s. A proxy is always the first hop: `ProxyDialer` carries the database protocol directly (TLS runs over it, verified against the profile's host), or `TunnelOptions::proxy` carries the first SSH hop (jump hosts and TLS then work as before). PostgreSQL and MySQL/MariaDB use it through the existing `Dialer` (`via`). |
| Proxy profile | `ConnectionProfile.proxy` (`kind` `socks5`/`http`, `host`, `port`, optional `user`); the password is the keychain item `conn/<id>/proxy-password`, kept only while a user is set. A proxy cannot reach a local Unix socket (validation asks for SSH). The connection sheet has a "Proxy" disclosure before SSH: type, host, port (1080/8080 placeholders), user and masked password. |
| Unix socket over SSH | A database host that is a path is a socket on the SSH server: `Destination::Socket` opens a `direct-streamlocal@openssh.com` channel. PostgreSQL follows libpq (`/var/run/postgresql` → `/var/run/postgresql/.s.PGSQL.<port>`; a full `.s.PGSQL.*` path is kept); MySQL/MariaDB use the socket file as entered (`/var/run/mysqld/mysqld.sock`). `Dialer::is_socket` tells the drivers to skip TLS negotiation, as for local sockets. |
| Tunnel reconnect | `SharedTunnel` holds a profile's planned hops, options and current tunnel; concurrent callers wait for one attempt (prompts once). `TunnelDialer` asks it for a live tunnel on every dial: a closed transport is reconnected first; a channel open that fails because the transport is gone (closed, or channel-open timeout on a half-open connection) discards the tunnel, reconnects once and retries. If that fails the error says "the SSH tunnel is closed and could not be re-established: …"; the app's session open keeps the typed kind (`hostKey`, `cancelled`, `ssh`) and prefixes "The SSH tunnel was closed and could not be re-established." Open sessions on a dead tunnel reconnect through it since M1-08 (§14). |
| Cloud CA bundles | `tls::CloudCa` (`AwsRds`, `Azure`) embeds public PEM bundles from `crates/openrows-net/certs/`; a profile's `tls.cloudCa` replaces the system trust store like a CA file (and makes `require` verify the chain). The TLS section offers "Cloud CA: None / AWS RDS / Aurora / Azure Database"; choosing one hides the CA file field. GCP Cloud SQL is not bundled: its server CAs are per instance (download the instance's `server-ca.pem` and use it as the CA file). |
| Insecure badge | `TlsSettings::insecurity(host)`: TLS disabled, or a mode that does not verify the certificate, towards a non-local host. Profiles carry `insecure` reasons; `connection_open` adds `hostKeyNotSaved` while the profile's tunnel accepted an unknown host key with the new "Connect once" answer (`FirstUseDecision::AcceptOnce`, not written to `known_hosts`). The connection list shows an open-lock mark with the reasons in the row tooltip; the window toolbar's context bar shows a "Not secure" badge. |
| M1-07 diagnostics | `TlsDetails` adds `not_before`, `expires_in_days` and the subject alternative names (DNS and IP). Test connection returns `certificate` (subject, issuer, validity, days left, names, verification) and warns on the TLS step when the certificate expires within 30 days or has expired. After connecting it times three `SELECT 1` round trips and reports the median (step "Round trip (SELECT 1)" with min/max, and a latency badge: < 50 ms fast, < 200 ms moderate, else slow). Proxy hops appear in the step labels ("Connect to SSH server … via SOCKS5 proxy …", "Connect to db:5432 via HTTP proxy …"); proxy failures get a hint pointing at the Proxy section. |
| Tests | Unit: SOCKS5/HTTP encoding, credentials and refusals against the in-process `testing::TestProxy` (feature `test-proxy`), cloud CA parsing and trust replacement, certificate details and expiry, diagnostics labels, latency, insecure reasons, proxy settings and keychain lifecycle. `just test-ssh` adds SSH behind SOCKS5 and HTTP proxies, a Unix socket over SSH, reconnect after the server cut the session (and the error once the server is gone), and "connect once" host keys. `just test-pg` and `just test-mysql` connect through both proxy types with TLS. |

Cloud CA sources (public trust anchors, redistributed unchanged; see docs/22 §2):

| File | Source | SHA-256 fingerprints |
| --- | --- | --- |
| `aws-rds-global-bundle.pem` (111 roots, every commercial region) | https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem | per certificate; check with the command below |
| `azure-database-roots.pem` | https://cacerts.digicert.com/DigiCertGlobalRootG2.crt.pem, https://cacerts.digicert.com/DigiCertGlobalRootCA.crt.pem, https://www.microsoft.com/pkiops/certs/Microsoft%20RSA%20Root%20Certificate%20Authority%202017.crt (DER, converted to PEM) | G2 `CB:3C:CB:B7:…:B1:CB:5F`, Global Root CA `43:48:A0:E9:…:C7:01:61`, Microsoft RSA 2017 `C7:41:F7:0F:…:30:73:E0` |

Refresh (each release, or when a provider announces a CA rotation): download the files above into an empty temporary directory, convert the Microsoft certificate with `openssl x509 -inform DER -in <file> -out microsoft-rsa-2017.pem`, compare fingerprints with the providers' documentation (`openssl crl2pkcs7 -nocrl -certfile <bundle> | openssl pkcs7 -print_certs -noout`), replace the files in `crates/openrows-net/certs/` (Azure: concatenate the three PEM files) and run `cargo nextest run -p openrows-net tls::`, which checks that every bundle parses and that the AWS bundle still holds at least 100 roots.

## 14. Implementation notes (M1-08 resilience)

Details and tests are in [14-reliability-and-observability.md §10](14-reliability-and-observability.md#10-implementation-notes-m1-08-reconnect-and-resilience). Network-layer parts:

| Area | What is implemented |
| --- | --- |
| TCP | `configure_tcp`: no Nagle delay and OS keepalive (30 s idle, 10 s between probes on macOS, Linux and Windows) on direct database sockets, proxy connections and the first SSH hop. |
| SSH | russh keepalive every 30 s on every hop, plus `SshTunnel::probe` / `SharedTunnel::probe`: a `keepalive@openssh.com` ping per hop with a deadline; a tunnel that is closed or does not answer is dropped so the next `TunnelDialer` dial reconnects it (once, prompting once). |
| Path monitor | `PathSample`/`watch_path`: wake = wall clock ≥ 4 s ahead of the monotonic clock between 2 s samples; network change = a different local address for the default IPv4/IPv6 route. The app re-probes tunnels, metadata and sessions on either. |
| Test link | `testing::FaultyLink` (feature `test-proxy`): a loopback TCP relay that can cut every connection, black-hole traffic (half-open link) or refuse new connections. |

## 15. Implementation notes (M2-24 cloud and zero-trust hops)

`openrows-net::helper` manages one structured helper hop before database TLS. `ConnectionProfile.helper` is absent in old profiles, and stores only the fields needed by AWS SSM, kubectl, the Google auth proxies or cloudflared. SSH/proxy chaining, Unix sockets and local file engines are rejected. Database URLs cannot represent a cloud route: URL import edits the database endpoint and leaves the separate helper settings intact; save/load JSON retains the hop.

| Kind | Argument vector (each displayed token is one argument) | Database TLS |
| --- | --- | --- |
| SSM | `aws ssm start-session --target TARGET --document-name AWS-StartPortForwardingSession --parameters JSON` plus optional `--profile PROFILE --region REGION`; with a remote host the document is `AWS-StartPortForwardingSessionToRemoteHost`. JSON contains `portNumber`, `localPortNumber`, and optional `host`, each as a one-element string array. | Verification and SNI use the original database host; the helper only changes TCP routing. |
| Kubernetes | `kubectl --context CONTEXT --namespace NAMESPACE [--kubeconfig FILE] port-forward --address 127.0.0.1 RESOURCE LOCAL:REMOTE` | Verification and SNI use the original database host. |
| Cloud SQL | `cloud-sql-proxy --address 127.0.0.1 --port LOCAL [--auto-iam-authn] [--private-ip] [--credentials-file FILE] INSTANCE` | Selecting the proxy defaults database TLS to `disable`, with a hint: the auth proxy encrypts and authenticates the upstream Google connection. Explicit database TLS still uses the original host. |
| AlloyDB | `alloydb-auth-proxy --address 127.0.0.1 --port LOCAL [--auto-iam-authn] [--public-ip] [--credentials-file FILE] INSTANCE_URI` | Same TLS policy as Cloud SQL. Private IP is the tool's default; public selection adds `--public-ip`. |
| Cloudflare | `cloudflared access tcp --hostname HOSTNAME --url 127.0.0.1:LOCAL` | Verification and SNI use the original database host. Access credentials/service tokens stay in cloudflared's own configuration; no token fields are stored. |

The vectors follow the public [AWS Session Manager documentation](https://docs.aws.amazon.com/systems-manager/latest/userguide/session-manager-working-with-sessions-start.html), [kubectl reference](https://kubernetes.io/docs/reference/kubectl/generated/kubectl_port-forward/), [Cloud SQL Auth Proxy reference](https://github.com/GoogleCloudPlatform/cloud-sql-proxy/blob/main/docs/cmd/cloud-sql-proxy.md), [AlloyDB Auth Proxy](https://github.com/GoogleCloudPlatform/alloydb-auth-proxy) and [Cloudflare arbitrary TCP documentation](https://developers.cloudflare.com/cloudflare-one/access-controls/applications/non-http/cloudflared-authentication/arbitrary-tcp/).

| Area | Implemented behavior |
| --- | --- |
| Ownership | The app's pool holds weak references; dialers and streams hold leases. Two sessions of the same unchanged profile share startup and a process. The last lease kills the process group. Edited profiles use a new helper while old sessions retain the old one; both remain covered by the exit hook. |
| Startup | Allocate a port by binding IPv4 loopback port zero and releasing it, launch the executable directly with a typed argument vector, then check TCP readiness against a 30-second deadline. Dropping/cancelling startup kills the tree. As with any released ephemeral port, a concurrent external binder can cause startup failure. |
| Restart | Observe unexpected exit, kill leftover descendants and restart once on the same port while leases remain. A second exit is terminal for that helper lifetime and produces a typed network failure. Existing database streams fail; existing session resilience may reconnect idle sessions, without replaying SQL. |
| Shutdown | The Tauri `RunEvent::Exit` hook calls `Helpers::shutdown`, cancels pending startup, kills live process trees and waits for reaping. Unix children use their own process group; Windows uses `taskkill /T /F`. |
| Diagnostics | Test connection adds `Start <tool>`, exact escaped argument display and detected version; failures include bounded stderr and tool-specific login/context hints. The last 200 lines are kept, each bounded to 4 KiB; credential-like lines (`token=`, `password=`, `Authorization:`) are redacted before storage. Oversized lines are redacted in full. |
| Tools | Existing Settings ▸ Tools lists aws, session-manager-plugin, kubectl, cloud-sql-proxy, alloydb-auth-proxy and cloudflared with detected versions and path overrides, using the dump-tool install directories. An explicit broken override never falls back. SSM discovers the plugin and adds its directory to the child's PATH. |
| UI | Connection sheet Route ▸ Cloud / zero-trust exposes typed fields and missing-tool advice. The toolbar health button opens a connection status popover containing the configured route, e.g. `Route: kubectl port-forward svc/db → TLS → PostgreSQL`. |
| Verification | Golden argument and validation tests; fake-tool sharing, tree cleanup, cancellation, readiness timeout, shutdown, restart and scrubbed failures; profile save/load; UI settings/form/route tests. `just test-pg` routes real PostgreSQL through all four kinds (both Google flavours) and verifies the original TLS hostname; `just test-mysql` routes MySQL/MariaDB through fake kubectl. CI needs no real cloud tools/accounts. The fake binary is gated behind `test-helper`, never normal app builds. |

No native AWS/Kubernetes/GCP APIs, cloud credential storage or helper-plus-SSH/proxy chaining are implemented. Real cloud-provider authentication is left to the installed tools; fake integration tests establish routing and lifecycle behavior, not successful access to an external cloud account.

### Implemented (M2-24b): custom command hop

Custom routes store an absolute `program`, an ordered `args` vector and optional
`readyTimeoutSeconds` (default 30; 1–300). At least one argument must contain
`{local_port}` to tell the helper where to listen. `{remote_host}` and
`{remote_port}` use the profile database destination. Substitution is a single
pass per argument, with no shell, splitting or recursive expansion; unknown or
unclosed placeholders and NUL are rejected. The helper must bind IPv4 loopback.
The form provides a program picker, add/remove/up/down argument rows, placeholder
help and timeout. The original supervisor, sharing, single restart, scrubbed
stderr, diagnostics and status popover apply unchanged; a changed database
destination also acquires a new helper. Custom failures explain that the command
exited or never listened on `{local_port}`.

Trust reuses M2-21 `CommandTrust` with a separate hop purpose and a SHA-256 of the
program and template arguments (docs/12 §11). Imported profiles always start
untrusted. No custom version probe runs; diagnostics label its version as not
probed. Exchange v1 includes all helper kinds, with custom templates exported as
configuration and fresh imported ids (docs/15 §9). Literal-spawn, validation,
purpose-isolation, edit-invalidation and round-trip tests cover these boundaries;
`just test-pg` explicitly trusts a fake custom helper and runs `SELECT 1` through
it with original-host TLS verification.

OpenRows uses `rustix` for process-group termination and has no direct `nix`
dependency; transitive `nix` through `russh-cryptovec` → `russh` is accepted.
