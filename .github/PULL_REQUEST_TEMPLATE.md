<!--
PR title must be a Conventional Commit header, e.g. "feat(grid): pin columns".
It becomes the squash-merge commit message. See CONTRIBUTING.md#4-pull-requests.
Describe the change only: no co-author sections, signatures or generated-by footers.
-->

## What

<!-- One or two sentences on what this PR changes. -->

## Why

<!-- The problem or requirement. Link issues (Fixes #123) and ADRs / requirement IDs (FR-GRID-02). -->

## How

<!-- Notable implementation decisions, trade-offs, alternatives considered. -->

## Acceptance report

<!-- One row per acceptance criterion from the task spec. See docs/25-engineering-standards.md#3-acceptance-report -->

| Criterion | Result | Evidence |
| --- | --- | --- |
|  |  |  |

## Quality gates

<!-- fmt · clippy · eslint · tsc · tests · dead-code scan · licence check -->

## Benchmarks (vs main)

<!-- Required for performance-relevant changes. Delete otherwise. -->

## How to try it

<!-- Exact commands and click-path with expected results for local verification. -->

## Screenshots

<!-- Required for UI changes: light and dark theme. Delete if not applicable. -->

## Checklist

- [ ] Title is a valid Conventional Commit header
- [ ] Commits pass the `commit-msg` hook
- [ ] Tests added or updated, all checks pass
- [ ] Documentation updated (see CONTRIBUTING.md#6-documentation-is-part-of-the-change)
- [ ] No secrets, personal data or local tool configuration files in the diff
- [ ] Breaking changes are marked with `!` and described
- [ ] Size/complexity limits met, no dead code or debug output
- [ ] Clean-up done (worktree, temp files, containers)
