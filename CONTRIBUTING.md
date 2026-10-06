# Contributing to OpenRows

Thanks for helping build a free, fast, open database and API client. This guide covers how we work with git, how to write commits and pull requests, and where to find the design documentation.

- Design docs: [docs/README.md](docs/README.md)
- Repository layout and tooling: [docs/18-repository-and-dev-workflow.md](docs/18-repository-and-dev-workflow.md)
- Licence, clean-room policy and governance: [docs/22-governance-and-licensing.md](docs/22-governance-and-licensing.md)
- Code of conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- Reporting vulnerabilities: [SECURITY.md](SECURITY.md)

## 1. First-time setup

```bash
git clone <repo-url> open-db-client
cd open-db-client
git config core.hooksPath scripts/git-hooks
```

The last command enables the repository's git hooks (currently `commit-msg`, which checks commit messages against the rules below). Toolchain setup (`just setup`) will be added with the code skeleton in milestone M0.

## 2. Branching model

- `main` is always releasable. It is protected: changes land only through pull requests with passing checks.
- Work happens on short-lived branches named `<type>/<short-description>`, using the same types as commit messages:

  | Example branch | Purpose |
  | --- | --- |
  | `feat/grid-column-pinning` | New feature |
  | `fix/pg-numeric-decode` | Bug fix |
  | `docs/networking-routes` | Documentation |
  | `refactor/session-actor-errors` | Internal restructuring |
  | `ci/macos-notarization` | CI and pipelines |

- Rebase on `main` to stay current; avoid merge commits inside feature branches.
- Pull requests are **squash-merged**. The squash commit message is the PR title (plus body), so the PR title must itself be a valid Conventional Commit header.

## 3. Commit messages

We use [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/). The `commit-msg` hook enforces the format.

```text
<type>(<scope>)!: <subject>

<body: what changed and why, wrapped at ~72 columns>

<footer: BREAKING CHANGE: ..., Refs: #123, Signed-off-by: ...>
```

### 3.1 Rules

1. **Header** is `<type>(<scope>): <subject>`; scope is optional but encouraged.
2. **Header length** is at most 72 characters.
3. **Subject** is in the imperative mood ("add", not "added"/"adds"), lower-case first word, no trailing period.
4. **Body** is separated from the header by a blank line and explains *why*, not only *what*.
5. **Breaking changes** use `!` after the type/scope and a `BREAKING CHANGE:` footer describing the migration.
6. **One logical change per commit.** Formatting-only changes go in their own `style` commit.
7. **Attribution:** commit messages and PR descriptions describe the change only. Do **not** add `Co-authored-by:` trailers, co-author sections, signatures, or "generated with/by …" footers. The hook rejects them.
8. **Sign-off:** add a DCO sign-off with `git commit -s` (see [docs/22-governance-and-licensing.md](docs/22-governance-and-licensing.md#4-contributions)). This is a certificate of origin from the committer, not a co-author credit.

### 3.2 Types

| Type | Use for | Appears in changelog |
| --- | --- | --- |
| `feat` | A user-visible feature | Features |
| `fix` | A user-visible bug fix | Bug fixes |
| `perf` | A performance improvement | Performance |
| `refactor` | Code change that neither fixes a bug nor adds a feature | — |
| `docs` | Documentation only | — |
| `test` | Adding or fixing tests | — |
| `build` | Build system, dependencies, packaging | — |
| `ci` | CI configuration and scripts | — |
| `style` | Formatting, whitespace; no code meaning change | — |
| `chore` | Maintenance that fits nowhere else | — |
| `revert` | Reverts a previous commit | Reverts |

### 3.3 Scopes

Use the crate, package or area you touched, without the `openrows-` / `@openrows/` prefix:

| Area | Scopes |
| --- | --- |
| Core crates | `core`, `types`, `sql`, `results`, `schema`, `store`, `secrets`, `net`, `platform`, `cli` |
| Drivers | `driver-pg`, `driver-mysql`, `driver-sqlite`, `driver-mssql`, `driver-clickhouse`, `driver-duckdb`, `driver-libsql`, `driver-mongodb`, `driver-redis`, `driver-oracle`, `driver-cassandra`, `driver-dynamodb`, `driver-elastic`, `driver-host` |
| API client | `http`, `script`, `collections` |
| AI | `ai`, `mcp` |
| Desktop app | `app`, `ipc`, `ui`, `grid`, `editor` |
| Repository | `docs`, `adr`, `deps`, `release`, `repo` |

### 3.4 Examples

```text
feat(grid): pin columns to the left edge

Pinned columns render on their own canvas layer so horizontal
scrolling never repaints them.

Refs: #142
Signed-off-by: Jane Doe <jane@example.com>
```

```text
fix(driver-pg): keep numeric precision above 38 digits

Values that do not fit Decimal128 are now stored as tagged text
instead of being rounded through f64.
```

```text
feat(ipc)!: return result windows as Arrow IPC bytes

BREAKING CHANGE: result_window now returns an ArrayBuffer instead of
JSON rows. Callers must decode with @openrows/arrow-worker.
```

```text
docs(adr): accept ADR-0004 canvas grid renderer
```

Bad examples the hook rejects: `Updated stuff`, `feat: Add grid.` (trailing period), `fix(pg) wrong` (missing colon), messages containing `Co-authored-by:`.

## 4. Pull requests

### 4.1 Title and description

- **Title** is a Conventional Commit header (it becomes the squash commit header), e.g. `feat(editor): add vim mode toggle`.
- **Description** uses the template in [.github/PULL_REQUEST_TEMPLATE.md](.github/PULL_REQUEST_TEMPLATE.md): what, why, how it was tested, screenshots for UI changes (light and dark), and linked issues/ADRs.
- The same attribution rule applies: no co-author sections, signatures or generated-by footers in PR titles, descriptions or comments.

### 4.2 Before opening a PR

- [ ] Branch is rebased on `main`.
- [ ] Commits follow the rules above (the hook passes).
- [ ] Tests added or updated; all checks pass locally.
- [ ] Docs updated (`docs/`, ADR if a decision changed) — see §6.
- [ ] No secrets, credentials, personal data or local tool configuration files in the diff (`git diff --cached --stat` and review every file).
- [ ] UI changes include screenshots in both themes.

### 4.3 Review and merge

- One approving review is required; two for security-sensitive code, FFI crates, the change-set apply path, value codecs, and update/signing configuration ([docs/18](docs/18-repository-and-dev-workflow.md#commits-and-prs)).
- Authors resolve review threads by pushing fixes; reviewers resolve their own threads.
- Merge with **Squash and merge** only. Edit the squash message so it is a clean Conventional Commit (no "Co-authored-by" lines added by the hosting UI).

## 5. Releases and versioning

- Semantic Versioning. Pre-1.0 releases are `0.x.y` on `alpha`/`beta` channels.
- Changelogs are generated from commit types (`feat`, `fix`, `perf`, `revert`, breaking changes) and curated before release.
- Tags are `vX.Y.Z`, created by maintainers from `main`. Details: [docs/16-platform-and-distribution.md](docs/16-platform-and-distribution.md#8-release-process).

## 6. Documentation is part of the change

Every PR that changes behaviour, architecture, configuration or user-facing UI updates the relevant document in the same PR:

| You changed… | Update… |
| --- | --- |
| A significant or hard-to-reverse decision | New or superseding ADR in `docs/adr/` |
| Architecture, crates, IPC | `docs/03`–`docs/07`, `docs/18` |
| A driver | `docs/05-database-drivers.md` support table |
| Security-relevant behaviour | `docs/12-security.md` |
| Performance budgets or techniques | `docs/13-performance.md` |
| UI structure or interaction rules | `docs/20-ux-foundations.md` |
| AI / MCP behaviour or data sharing | `docs/23-ai-assistant.md` |
| Networking / routes / protocols | `docs/24-networking-and-protocols.md` |
| Git workflow or contribution rules | This file and `docs/18` |

## 7. Clean-room rule

Do not copy code, assets, icons or text from proprietary applications. Learn from open-source projects only under compatible licences and keep their notices. See [docs/22-governance-and-licensing.md](docs/22-governance-and-licensing.md#3-clean-room-policy).
