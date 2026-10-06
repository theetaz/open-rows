# 16 — Platform and Distribution

## 1. Strategy: macOS first, portable always

The core engine is OS-agnostic and tested on macOS, Windows and Linux from the first commit (NFR-PORT-03). The UI ships on macOS first; Windows and Linux follow once the macOS alpha stabilises (see [19-roadmap.md](19-roadmap.md)). Platform-specific code lives behind small traits in `openrows-platform` (keychain extras, file reveal, appearance, biometric unlock).

## 2. Webview differences

| | macOS | Windows | Linux |
| --- | --- | --- | --- |
| Engine | WKWebView (Safari/WebKit) | WebView2 (Chromium, Evergreen) | WebKitGTK 4.1 |
| JS/canvas performance | Very good | Very good | Varies by distro/GPU; weakest target |
| DevTools | Safari Web Inspector | Edge DevTools | WebKit inspector |
| Known pitfalls | Older WebKit on older macOS versions — check feature support (e.g. `OffscreenCanvas` text in workers needs Safari 16.4+/macOS 13.3+; fallback path required for macOS 12) | Large IPC payload performance different from WebKit; test both | Hardware acceleration bugs on some drivers (DMABUF renderer); font rendering differences |

Rules:

- CI runs the UI test suite on all three engines (Playwright WebKit + Chromium approximate webview engines; real-app smoke tests per OS).
- Feature detection, never UA sniffing. Each modern API used has a documented fallback or a minimum-version gate.
- Supported minimums: macOS 12 Monterey; Windows 10 1809+ with WebView2; Linux with WebKitGTK 4.1 (Ubuntu 22.04+, Fedora 38+, Debian 12+).

## 3. macOS specifics

| Area | Plan |
| --- | --- |
| Architecture | Universal binary (`aarch64-apple-darwin` + `x86_64-apple-darwin`, `lipo`), or separate downloads if size matters; DMG ≤ 40 MB target. |
| Window chrome | Native traffic lights with a transparent, overlay titlebar (`titleBarStyle: Overlay`) so the toolbar sits in the titlebar area; vibrancy for sidebar (`NSVisualEffectView` via `window-vibrancy`); on macOS 26+ the Liquid Glass material (`NSGlassEffectView`) behind a transparent webview via a small objc2 bridge in `openrows-platform`, with CSS `backdrop-filter` glass inside the webview and solid fallbacks when "Reduce transparency" is on; full-screen and Stage Manager friendly. See [20-ux-foundations.md §3.2](20-ux-foundations.md#32-macos-native-design-methodology-macos-26-era). |
| Menus | Native menu bar generated from the command registry (App, File, Edit, View, Query, Connection, Window, Help) with user shortcuts. |
| Keyboard | ⌘-based defaults; respects system text editing shortcuts (⌥⌫, ⌃A/⌃E) in text fields and editor. |
| Appearance | Follows system light/dark; accent colour from system where appropriate. |
| Keychain | Generic password items under service `com.prabhavalabs.openrows`; access restricted to the signed app. Optional Touch ID for revealing/using prod secrets (P1). |
| SSH agent | Uses `SSH_AUTH_SOCK` from launchd; supports macOS Keychain-backed keys via agent; 1Password/Secretive agents work through the same socket. |
| File access | Not sandboxed (Developer ID distribution) so SSH keys, `~/.ssh/config`, `pgpass`, local SQLite files work without friction. A Mac App Store build (sandboxed, security-scoped bookmarks) is a post-1.0 option. |
| Signing & notarization | Developer ID Application certificate, hardened runtime, entitlements: `com.apple.security.network.client` is only needed if sandboxed; for hardened runtime we avoid `allow-jit`/`disable-library-validation` unless required by FFI drivers loaded in the driver host (Oracle Instant Client is user-installed and unsigned by us → load inside the separately-signed `openrows-driver-host` helper with `disable-library-validation`, keeping the main app strict). Notarized via `notarytool` in CI; stapled. |
| Deep links | `openrows://` scheme for OAuth callbacks and "open connection" links (with confirmation). |
| Services / Quick actions | P2: "Open in OpenRows" for `.sqlite`/`.db`/`.duckdb` files via document types in `Info.plist`. |

### Implemented M0-T4b native material

`apps/desktop/src-tauri/src/material.rs` applies Tauri's `Effect::Sidebar` through
its existing `window-vibrancy` integration. This installs an `NSVisualEffectView`
behind the transparent WKWebView. `FollowsWindowActiveState` follows the native
window; the effect radius is 26 px, native decorations and shadow remain enabled,
and `titleBarStyle: Overlay` retains the real traffic lights. DOM traffic lights
are not drawn. The webview root is transparent; the table, grid, SQL and sheet
surfaces use opaque theme tokens for legibility. Rail, sidebar and inspector use
the glass tokens over the desktop material.

Transparent WKWebView requires `app.macOSPrivateApi: true` in `tauri.conf.json`
and Tauri's `macos-private-api` Cargo feature. This uses private WebKit APIs and
is appropriate for the planned Developer ID distribution; a Mac App Store build
would need to remove that dependency on private API and redesign the material
boundary. This task uses NSVisualEffectView on all supported macOS versions;
NSGlassEffectView and Windows/Linux vibrancy are not implemented.

The macOS-only `objc2-app-kit` dependency exposes the safe `NSWorkspace` Reduce
Transparency accessor (no custom unsafe FFI). At setup, first-paint readiness and
window focus, the app checks the native preference, clears native effects when
reduced, and sets `data-transparency="reduced"` on the document root so glass
becomes solid. Native accessibility changes made in System Settings are reflected
when returning focus to OpenRows. The CSS reduced-transparency media query is also
retained. No IPC command is added. Other platforms compile without the native
material module. See Apple's [Reduce Transparency accessor](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducetransparency)
and [Tauri window effects](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html#method.set_effects).

The review sheet uses native HTML dialog focus containment and dismissal; a fixed
modal, inert-background and focus-loop fallback supports WebKit before dialog
support (macOS 12.0–12.2). Build/tests cannot prove desktop translucency, native
rounding/shadow, or light/dark appearance over different wallpapers: verify these
locally along with Reduce Transparency and inactive-window material changes.

## 4. Windows specifics (pre-1.0)

- Installers: MSI (WiX) for enterprise and NSIS `.exe` for consumers; per-user install by default (no admin).
- WebView2 Evergreen bootstrapper embedded; offline installer variant for air-gapped environments.
- Code signing with an EV or OV certificate (or Azure Trusted Signing) to avoid SmartScreen warnings.
- Credential Manager for secrets; Windows OpenSSH agent named pipe and Pageant support for SSH.
- Snap layouts, Mica backdrop (optional), high-contrast themes mapped to our high-contrast tokens.
- ARM64 build.

## 5. Linux specifics (pre-1.0)

- Packages: AppImage (universal), `.deb`, `.rpm`; Flatpak on Flathub; optionally Snap.
- Secret Service (GNOME Keyring/KWallet) via `keyring`; encrypted file fallback (see [12-security.md](12-security.md)).
- Wayland and X11; document known WebKitGTK workarounds (e.g. `WEBKIT_DISABLE_DMABUF_RENDERER=1`) and apply automatically when a known-bad configuration is detected.
- Respect XDG directories and desktop file integration.

## 6. Updates

```mermaid
sequenceDiagram
    participant App
    participant Feed as Update feed (GitHub Releases, static JSON)
    participant User

    App->>Feed: GET latest.json (on launch + every 24 h, if enabled)
    Feed-->>App: version, notes, platform URLs, signatures
    App->>App: compare semver + channel (stable / beta)
    App->>User: non-blocking "Update available" with notes
    User->>App: Install
    App->>Feed: download artifact
    App->>App: verify Ed25519 signature (pinned public key) + OS code signature
    App->>App: replace bundle, relaunch (restores session)
```

- Tauri updater plugin; channels: `stable`, `beta`, `nightly`.
- Updates never install silently while queries are running or change sets are pending; the app asks to finish or postpone.
- Package-manager installs (Homebrew cask, winget, Flathub, apt repo) disable the in-app updater and defer to the package manager.

## 7. Distribution channels

| Channel | Platform | Milestone |
| --- | --- | --- |
| GitHub Releases (DMG, zip) | macOS | Alpha |
| Homebrew cask | macOS | Alpha |
| winget, Scoop, MSI/NSIS | Windows | Beta |
| AppImage, deb, rpm, Flathub | Linux | Beta |
| Mac App Store (sandboxed variant) | macOS | Post-1.0 (evaluate) |

## 8. Release process

1. Release branch cut from `main`; version bumped across Cargo workspace and `package.json` files by a script.
2. CI builds all targets from the tag, signs, notarizes (macOS), generates SBOM (CycloneDX for Rust and npm), checksums, and updater signatures.
3. Artifacts uploaded to a draft GitHub Release; smoke tests run against the produced artifacts on each OS.
4. Maintainer publishes; `latest.json` updated last so clients never see a half-published release.
5. Changelog generated from Conventional Commits and curated.
