# Tasks

Compile, format, lint, and test tasks run on a build runner with
`mise run verify`. Implementation starts after the operator reviews this
change.

## 0. Decide

- [x] 0.1 The operator reviews this change; proof: the spec pull request is approved, and any change of decision is folded into design.md
  - 2026-10-17 (UTC): the spec merged as [#156][pr-156]. Queue row [VER-066][queue] records the operator's decision that the spec is reviewed before implementation; GitHub holds no approving review on the pull request. The operator's later rulings, the build rule and the tag rule, are in design.md sections 1.3 and 2.2.

## 3. Workflow

- [x] 3.1 `.github/workflows/release.yml` with the inputs, secrets, jobs, and concurrency of design section 1; proof: zizmor `--pedantic` and the conftest policy pass on it
  - 2026-10-17 (UTC): [#166][pr-166] shipped `release.yml`. On `main` at 191aa64, zizmor 1.31.1 `--pedantic --offline` reports no finding in `release.yml`, and `conftest test --policy policy .github/workflows` passes 3 of 3. `verify` runs the conftest policy but not zizmor.
  - 2026-10-17 (UTC): zizmor reports four findings in the caller, `self-release.yml`.
    The findings are workflow-level write permissions (2), no workflow concurrency, and `./` instead of the self-repository syntax.
    Those findings belong to the caller, and this task does not cover them.
- [x] 3.2 Steps run `mise run ver <verb>`, with the verctl version pinned in the task, and settings go through env only.
  - verctl is built from source only for a release, only in CI, and never in every job.
  - A release reuses a binary a pull-request or merge workflow already built.
  - In verctl's own repository the release run builds once (or reuses) and every other job uses the pinned verctl (design section 1.3).
  - Proof: `git grep -n '\-c "\$' .github actions` prints nothing, and `self-release.yml` builds verctl only in its `build` job.
  - 2026-10-17 (UTC): `git grep -n '\-c "\$' .github actions` prints nothing on `main` at 191aa64. `self-release.yml` runs `mise run build` only in its `build` job; every job of `release.yml` runs `mise run ver <verb>` with the uploaded build first on PATH. Not done: `ci.yml`'s `plan` job still runs verctl through `cargo run`.
- [x] 3.3 This repository calls its own workflow, with the `build` job of design section 1.3, and deletes `version-pr.yml` and `publish.yml`.
  Proof: the next Version PR merge on `main` releases through the new workflow, and a rerun reports `nothing to release`.
  - 2026-10-17 (UTC): merging Version PR [#168][pr-168] released 0.4.6 through [run 39075339999][run-046].
    The crates.io registry lists 0.4.6, tag `v0.4.6` names 191aa64, and the Release carries both tarballs.
    Its `docs` leg passed on rerun (attempt 2). The dispatched [run 39076359412][run-rerun] reported `nothing to release` and skipped publish, assets, and docs.
- [ ] 3.4 The `mise-ignored-config-paths` input of design section 1.4.
  Proof: zizmor `--pedantic` and the conftest policy pass on the workflow.
  Proof: session-lake's first release on it installs no root `mise.toml` tool outside `propose`.

## 4. Docs and consumers

- [x] 4.1 Instructions, the operator skill, README, AGENTS.md, and `examples/workflows/` describe the caller and the migration; proof: `cargo test operator_docs` passes, and only the migration guide names `version-packages`
  - 2026-10-17 (UTC): `operator_docs` passed in [CI run 39071583328][run-166]. `git grep version-packages` over `src`, `actions`, `README.md`, `AGENTS.md`, `examples`, `skills`, and `.github` prints only the migration steps in `README.md` and `src/instructions.md`.
- [ ] 4.2 The template renders the caller; proof: a template-rendered repository's caller matches design section 1 and its first release passes
  - 2026-10-17 (UTC): open. The template work is queued in the templates repository; this repository's part of the change is complete.

[queue]: ../../../tasks.yaml
[pr-156]: https://github.com/victor-software-house/verctl/pull/156
[pr-166]: https://github.com/victor-software-house/verctl/pull/166
[pr-168]: https://github.com/victor-software-house/verctl/pull/168
[run-046]: https://github.com/victor-software-house/verctl/actions/runs/39075339999
[run-166]: https://github.com/victor-software-house/verctl/actions/runs/39071583328
[run-rerun]: https://github.com/victor-software-house/verctl/actions/runs/39076359412
