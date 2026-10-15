# Tasks

Compile, format, lint, and test tasks run on a build runner with
`mise run verify`. Implementation starts after the operator reviews this
change.

## 0. Decide

- [ ] 0.1 The operator reviews this change; proof: the spec pull request is approved, and any change of decision is folded into design.md

## 1. Idempotent publish

- [ ] 1.1 `query` on publisher recipes, stock `cargo` and `bun` queries, and the pre- and post-publish checks; proof: the idempotent-publish registry scenarios pass as tests with a fake publisher and query, including a fake registry that lacks the version of a workspace member
- [ ] 1.2 The version commit from first-parent history, and `publish --plan` with `$GITHUB_OUTPUT`; proof: the version-commit scenarios pass against a local repository
- [ ] 1.3 `nothing to release` and the `outcome` field; proof: the nothing-to-release scenarios pass with mocked GitHub responses

## 2. Version PR branch and auto-merge

- [ ] 2.1 `verctl/release/<base>` replaces `version-packages` in `release.rs`, `versions.rs`, and the preview; proof: the version-pr-branch scenarios pass, and `git grep version-packages -- src actions` prints nothing
- [ ] 2.2 `prepare --pr --auto-merge` in Rust, and `auto-merge.sh` deleted; proof: the version-pr-auto-merge scenarios pass against mocked GitHub responses

## 3. Workflow

- [ ] 3.1 `.github/workflows/release.yml` with the inputs, secrets, jobs, and concurrency of design section 1; proof: zizmor `--pedantic` and the conftest policy pass on it
- [ ] 3.2 Every step runs `verctl <verb>` with `env:` only, on a verctl that adopts ctl-core's environment-bound flags; proof: `git grep -n '\-c "\$' .github actions` prints nothing
- [ ] 3.3 This repository calls its own workflow, with `mise run ver` building verctl, and deletes `version-pr.yml` and `publish.yml`; proof: the next Version PR merge on `main` releases through the new workflow and a rerun reports `nothing to release`

## 4. Docs and consumers

- [ ] 4.1 Instructions, the operator skill, README, AGENTS.md, and `examples/workflows/` describe the caller and the migration; proof: `cargo test operator_docs` passes, and only the migration guide names `version-packages`
- [ ] 4.2 The template renders the caller; proof: a template-rendered repository's caller matches design section 1 and its first release passes
