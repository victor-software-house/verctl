# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.
Each phase is one pull request with a `minor` fragment, and lists the exact
config and workflow edits for qctl, forkctl, ctl-core, unixd, and yamled.

## 0. Decide

- [ ] 0.1 The operator answers the ten decisions in proposal.md; proof: each answer is recorded on VER-011 with `mise run q edit VER-011 -n …`, and this change is updated where an answer differs from the recommendation
- [ ] 0.2 The held branch `feat/011-refresh-lockfile` is deleted unpushed once 0.1 is recorded; proof: `git branch --list feat/011-refresh-lockfile` prints nothing

## 1. Ecosystems (0.4.0)

- [ ] 1.1 cargo and bun modules own version, dependents, lockfile refresh and check, publish order, and registry query; proof: the ecosystems spec scenarios pass as tests
- [ ] 1.2 `version` and `propose` replace `prepare`; the report field `next` becomes `after`; proof: the version-proposal "version step" scenarios pass
- [ ] 1.3 `drivers`, `publishers`, the per-package driver keys, and `prepare` are refused with their replacements, and every struct refuses unknown keys; proof: the release-config scenarios pass
- [ ] 1.4 `drivers.toml` is deleted; proof: `git grep -n drivers.toml` prints nothing outside CHANGELOG.md
- [ ] 1.5 The five consumers delete their `prepare` section and their Version PR jobs stop installing a toolchain for it; proof: each consumer's next Version PR commits its lockfile

## 2. Release reconciliation (0.5.0)

- [ ] 2.1 The release commit is derived from first-parent history; proof: the release-state "release commit" scenarios pass
- [ ] 2.2 `release` and `release asset` run publish, tag, release, asset, and finalize with their guards; proof: the release-state transition scenarios pass against a local bare remote and a mocked registry and GitHub API
- [ ] 2.3 `status` reports both axes and the versioned plan; `branch`, `tags`, and `packages[].assets` replace their predecessors; proof: snapshot tests with `--color never` and `--format json`
- [ ] 2.4 `release.yml` on push to the default branch replaces `version-pr.yml` and `publish.yml` here and in the five consumers; proof: one real release of this repository goes from Version PR merge to a published Release with both assets, and a rerun of the workflow reports every step `already`

## 3. Content-verified check and the proposal lifecycle (0.6.0)

- [ ] 3.1 `check` enforces the five rules; proof: the version-proposal `check` scenarios pass
- [ ] 3.2 `propose` closes an empty Version PR and refuses while a release is incomplete; the `proposal` keys replace `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`; proof: the version-proposal lifecycle scenarios pass
- [ ] 3.3 The label, the branch exemption, and the `labeled` and `unlabeled` CI types are removed; proof: a real Version PR passes `check` on its first CI run

## 4. Served files (0.7.0)

- [ ] 4.1 `templates.rs` renders through `ctl_core::served`, with the `tags` map and the mount line in the context, after telling the session that owns CTC-008; proof: served bytes are identical before and after for every template in this repository and qctl
- [ ] 4.2 `render` replaces `pin`; operator docs and example workflows are served templates; proof: `check` fails on a hand-edited served file and passes after `render`
- [ ] 4.3 `pins` and `patterns` are refused; qctl and forkctl move their four pinned files to templates; proof: each repository's `check` passes

## 5. Auth (0.8.0)

- [ ] 5.1 verctl performs the crates.io and npm OIDC exchanges; `registries` declares URL and auth; proof: the ecosystems OIDC scenarios pass, and one real crates.io release runs with no `CARGO_REGISTRY_TOKEN` secret
- [ ] 5.2 The `bootstrap` stage stops a first publication under OIDC; proof: the release-state bootstrap scenario passes
- [ ] 5.3 bun-release's publish path runs through the bun ecosystem; proof: one real bun-release version publishes through `verctl release`

## 6. Close

- [ ] 6.1 Archive VER-013, VER-016, VER-020, VER-028, VER-029, VER-030, VER-031, VER-033, VER-034, VER-035, VER-036, VER-038, and VER-010, each when its phase lands, with the phase's pull request as evidence; proof: `mise run q check` passes
- [ ] 6.2 Update AGENTS.md, README.md, `src/instructions.md`, the operator skill, and `docs/served-files.md` in each phase; proof: `cargo test operator_docs` passes and no document names a removed key or command
- [ ] 6.3 Archive this change and VER-011 after phase 5; proof: `openspec validate --specs` passes
