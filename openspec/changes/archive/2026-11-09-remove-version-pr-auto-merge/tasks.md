# Tasks

## 1. Code

- [x] 1.1 Remove `--auto-merge`, `require_auto_merge`, `auto_merge`, `requires_status_check`, and `enable_auto_merge`, with their tests; proof: `rg -i 'auto.?merge' src tests` prints nothing, and the PR's CI passes

  2026-11-09 UTC: [PR 230][verctl#230] removed them in [39a26ee][39a26ee], and its [verify run][run-41494127104] passed.
  On main at [caea7d3][caea7d3], `git grep -i -E 'auto.?merge' -- src tests` finds only the sentence in `src/instructions.md` that says verctl never enables auto-merge, and the refusal test from task 1.2.
  The proof as written prints those two hits, so it was read against that intent, not taken literally.

- [x] 1.2 Add a test that `prepare --pr --auto-merge` is refused as an unexpected argument; proof: the test passes in the PR's CI

  2026-11-09 UTC: `prepare_refuses_auto_merge` in [`tests/status.rs`][status-tests] runs in the [verify run][run-41494127104] of [PR 230][verctl#230], which passed.

## 2. Workflows and actions

- [x] 2.1 Remove the `auto-merge` input from `release.yml` and `actions/version-pr/action.yml`, and the `hold` job from `self-release.yml`; proof: `rg -i 'auto.?merge|RELEASE_HOLD' .github actions` prints nothing, and the workflow policy step passes in CI

  2026-11-09 UTC: on main at [caea7d3][caea7d3], `git grep -i -E 'auto.?merge|RELEASE_HOLD' -- .github actions` prints nothing.
  The workflow policy step is part of the [verify run][run-41494127104] of [PR 230][verctl#230], which passed.

## 3. Docs

- [x] 3.1 Update the instructions, the operator skill and their templates, `README.md`, `AGENTS.md`, and the `reusable-release-workflow` change; proof: `operator_docs` passes in CI.
  Also `rg -i 'auto.?merge'` finds the feature only in the changelog, the archive, this change, and finished queue rows

  2026-11-09 UTC: [PR 230][verctl#230] changed each of those files, and `operator_docs` ran in its passing [verify run][run-41494127104].

- [x] 3.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it

  2026-11-09 UTC: [PR 230][verctl#230] added `.changeset/remove-auto-merge.md`, and its [verify run][run-41494127104] passed.
  The [Version PR 234][verctl#234] consumed the fragment.

[verctl#230]: https://github.com/victor-software-house/verctl/pull/230
[verctl#234]: https://github.com/victor-software-house/verctl/pull/234
[39a26ee]: https://github.com/victor-software-house/verctl/commit/39a26eecef7a146c83b9b3b1f8f7429d6d3f56bd
[caea7d3]: https://github.com/victor-software-house/verctl/commit/caea7d3
[run-41494127104]: https://github.com/victor-software-house/verctl/actions/runs/41494127104
[status-tests]: ../../../../tests/status.rs
