# Tasks

Compile, format, lint, and test tasks run on the build host with `mise run verify`.
Every part in sections 1 to 4 is a reviewed pull request into the integration
branch `next`. Section 5 merges `next` into `main` as one release, 0.4.0.
Section 6 is patches after it.

## 0. Decide

- [ ] 0.1 The operator answers the open decisions in proposal.md; proof: each answer is recorded on VER-011 with `mise run q edit VER-011 -n …`, and this change is updated where an answer differs from the recommendation
- [ ] 0.2 The held branch `feat/011-refresh-lockfile` is deleted unpushed once 0.1 is recorded; proof: `git branch --list feat/011-refresh-lockfile` prints nothing
- [ ] 0.3 Branch `next` is created from `main`; proof: `git ls-remote origin refs/heads/next` prints a commit

## 1. Blocks and built-ins

- [ ] 1.1 The Manifest, Lockfile, Artifact, and Registry blocks exist, with cargo and bun providers for version, dependents, lockfile refresh through the owner, lockfile check by parsing, publish order, and registry query; proof: the building-blocks cargo and bun scenarios pass as tests
- [ ] 1.2 Steps run as `mise run --raw` or argv with the `VERCTL_*` environment, `writes` enforced and `check` honoured; declared ecosystems and per-package block overrides work; proof: the building-blocks step scenarios pass
- [ ] 1.3 verctl writes no lockfile; proof: `git grep` finds no write to a path ending in `.lock` in `src/`, and the lockfile ownership scenario passes
- [ ] 1.4 `version` replaces `prepare`, and the report field `next` becomes `after`; proof: the version-proposal version-step scenarios pass

## 2. The release state machine

- [ ] 2.1 The release commit is derived from first-parent history, and `status` reports both axes and the versioned plan; proof: the release-state scenarios for the release commit and for no labels pass
- [ ] 2.2 `release` and `release asset` run verify, publish, tag, release, asset, and finalize with their guards, and `released` steps after finalize; proof: the release-state transition scenarios pass against a local bare remote and mocked registry and GitHub APIs
- [ ] 2.3 bun publishes through `bun pm pack`, the packed-manifest inspection, and verctl's registry client with `auth: token`; proof: the building-blocks bun scenarios pass against a local npm-compatible registry

## 3. The Version PR

- [ ] 3.1 `propose` finds its pull request by head repository, `verctl/release/<base>`, and base; opens, updates, and closes it; refuses while a release is incomplete; the `proposal` keys replace `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`; proof: the version-proposal identity and lifecycle scenarios pass
- [ ] 3.2 `check` enforces identity and content; the label, the branch exemption, and the `labeled` and `unlabeled` CI types are removed; proof: the version-proposal `check` scenarios pass

## 4. Config, served files, docs

- [ ] 4.1 The final schema: every removed key refused with its replacement, every struct refusing unknown keys, every name resolved at load; proof: the release-config scenarios pass
- [ ] 4.2 `templates.rs` renders through `ctl_core::served`, with the `tags` map and the mount line in the context, after telling the session that owns CTC-008; `render` replaces `pin`; operator docs and example workflows are served templates; proof: served bytes are identical before and after for every template in this repository and qctl, and `check` fails on a hand-edited served file
- [ ] 4.3 AGENTS.md, README.md, `src/instructions.md`, the operator skill, and `docs/served-files.md` describe only 0.4.0; proof: `cargo test operator_docs` passes and no document names a removed key or command

## 5. Release 0.4.0

- [ ] 5.1 `release.yml` replaces `version-pr.yml` and `publish.yml` in this repository, and the asset jobs attest each archive; proof: the merge of `next` into `main` passes CI, and `gh attestation verify` passes for each 0.4.0 asset
- [ ] 5.2 One pull request merges `next` into `main` with one `minor` fragment; proof: the 0.4.0 Version PR passes `check` on its first run, and the release goes from merge to a published Release with both assets; a rerun reports every step `already`
- [ ] 5.3 qctl, forkctl, ctl-core, unixd, and yamled move to 0.4.0: delete `prepare`, adopt the two workflows; qctl and forkctl move four pinned files to templates; proof: each repository's next Version PR commits its lockfile and passes `check`

## 6. Patches after 0.4.0

- [ ] 6.1 `auth: oidc` for crates.io and npm; proof: the building-blocks OIDC scenarios pass, and one real crates.io release runs with no `CARGO_REGISTRY_TOKEN` secret
- [ ] 6.2 `provenance: true`, staged publish, and `release bootstrap` in the bun registry client; proof: one real bun-release version publishes with provenance through `verctl release`, and bun-release is archived
- [ ] 6.3 The `bootstrap` stage stops a first publication under OIDC; proof: the release-state bootstrap scenario passes
- [ ] 6.4 The built-in `mise-pin` step, and `MISE_LOCKED_VERIFY_PROVENANCE=1` on `mise install --locked` in the served CI workflow; proof: the building-blocks pin scenarios pass, a consumer's lock regenerated after 0.4.x records `provenance = "github-attestations"`, and that consumer's CI install passes with the variable set

## 7. Close

- [ ] 7.1 Archive VER-013, VER-016, VER-020, VER-028, VER-029, VER-030, VER-031, VER-033, VER-034, VER-035, VER-036, VER-038, and VER-010 when 0.4.0 or the patch that removes their cause releases, with that pull request as evidence; proof: `mise run q check` passes
- [ ] 7.2 Archive this change and VER-011 after section 6; proof: `openspec validate --specs` passes
