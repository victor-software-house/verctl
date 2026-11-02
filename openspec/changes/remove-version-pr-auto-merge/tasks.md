# Tasks

## 1. Code

- [ ] 1.1 Remove `--auto-merge`, `require_auto_merge`, `auto_merge`, `requires_status_check`, and `enable_auto_merge`, with their tests; proof: `rg -i 'auto.?merge' src tests` prints nothing, and the PR's CI passes
- [ ] 1.2 Add a test that `prepare --pr --auto-merge` is refused as an unexpected argument; proof: the test passes in the PR's CI

## 2. Workflows and actions

- [ ] 2.1 Remove the `auto-merge` input from `release.yml` and `actions/version-pr/action.yml`, and the `hold` job from `self-release.yml`; proof: `rg -i 'auto.?merge|RELEASE_HOLD' .github actions` prints nothing, and the workflow policy step passes in CI

## 3. Docs

- [ ] 3.1 Update the instructions, the operator skill and their templates, `README.md`, `AGENTS.md`, and the `reusable-release-workflow` change; proof: `operator_docs` passes in CI, and `rg -i 'auto.?merge'` finds the feature only in the changelog, the archive, this change, and finished queue rows
- [ ] 3.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it
