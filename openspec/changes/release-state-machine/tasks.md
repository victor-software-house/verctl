# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
On 2026-11-09 the operator limited 0.8.0 to the breaks that can be built soon; the larger rewrites ship later in increments (operator, 2026-11-09).
The 0.8.0 slices are the changes [version-pr-proposal](../version-pr-proposal/proposal.md), [command-renames](../command-renames/proposal.md), [served-file-check](../served-file-check/proposal.md), and [release-commit-proofs](../release-commit-proofs/proposal.md), plus VER-036.
Section 5 releases 0.8.0, and section 6 is patches after it.
0.4.0 to 0.7.4 shipped from `main` before this change; the parts of it they carried are checked below with their evidence, and [audit.md](audit.md) lists the state of every breaking item.
State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 0. Decide

- [x] 0.1 The operator answers the open decisions in proposal.md; proof: each answer is recorded on VER-011 with `mise run q edit VER-011 -n …`, and this change is updated where an answer differs from the recommendation
  - 2026-10-12 (UTC), all eight approved as recommended, recorded in a VER-011 note. Decision 8 (0.4.0 from `next`) is contradicted by what shipped, and the operator confirms the 0.8.0 scope after the audit.
- [x] 0.2 The held branch `feat/011-refresh-lockfile` is deleted unpushed once 0.1 is recorded; proof: `git branch --list feat/011-refresh-lockfile` prints nothing
  - 2026-11-09, `git branch --list feat/011-refresh-lockfile` and `git ls-remote origin refs/heads/feat/011-refresh-lockfile` print nothing.
- [ ] 0.3 Branch `next` is created from `main`; proof: `git ls-remote origin refs/heads/next` prints a commit
  - 2026-11-09, not started: no `next` exists. The 0.8.0 work is held on [stack #258][stack] instead, which waits on decision 8.

## 1. Blocks and built-ins

- [ ] 1.1 The Manifest, Lockfile, Artifact, and Registry blocks exist, with cargo and bun providers for version, dependents, lockfile refresh through the owner, lockfile check by parsing, publish order, and registry query. Proof: the building-blocks cargo and bun scenarios pass as tests
  - 2026-11-09, partly shipped: version read and write through drivers, the registry query before and after each publish ([#162][pr162]), and cargo workspace members in dependency order (`src/members.rs`). Remaining: the block model, lockfile refresh through the owner (verctl prints the follow-up command), lockfile check by parsing, and publish order across packages (VER-033).
- [ ] 1.2 Steps run as `mise run --raw` or argv with the `VERCTL_*` environment, `writes` enforced and `check` honoured; declared ecosystems and per-package block overrides work; proof: the building-blocks step scenarios pass
  - 2026-11-09, not started. `prepare.after` with `stage` globs is the precursor (`src/config.rs:1038`).
- [ ] 1.3 verctl writes no lockfile; proof: `git grep` finds no write to a path ending in `.lock` in `src/`, and the lockfile ownership scenario passes
  - 2026-11-09, partly shipped: `src/` writes no lockfile outside tests. The lockfile ownership scenario needs steps (1.2).
- [ ] 1.4 `version` replaces `prepare`, and the report field `next` becomes `after`; proof: the version-proposal version-step scenarios pass
  - 2026-11-09, not started: the verb is `prepare` (`src/cli.rs:40`).
  - 2026-11-09, in 0.8.0 through [command-renames](../command-renames/proposal.md) (VER-091), with `pin` renamed `render` and no change to what the version step does.

## 2. The release state machine

- [ ] 2.1 The release commit is derived from first-parent history, and `status` reports both axes and the versioned plan; proof: the release-state scenarios for the release commit and for no labels pass
  - 2026-11-09, partly shipped: `publish` derives each version commit from first-parent history (3452801, 2026-08-17), and no label is read ([#188][pr188]). Remaining: `status` reporting both axes.
- [ ] 2.2 `release` and `release asset` run verify, publish, tag, release, asset, and finalize with their guards, and `released` steps after finalize; proof: the release-state transition scenarios pass against a local bare remote and mocked registry and GitHub APIs
  - 2026-11-09, partly shipped under `publish` and `assets`: registry, tag, Release, and asset steps that report `already` on a rerun, and a registry-checked package tagged before its uploads (`src/publish.rs:350`). Remaining: the `release` verb, one atomic tag push, a draft Release until every asset is uploaded (PR 228, closed, is the reusable start), finalize, and `released` steps.
- [ ] 2.3 bun publishes through `bun pm pack`, the packed-manifest inspection, and verctl's registry client with `auth: token`; proof: the building-blocks bun scenarios pass against a local npm-compatible registry
  - 2026-11-09, not started.

## 3. The Version PR

- [ ] 3.1 `propose` finds its pull request by head repository, `verctl/release/<base>`, and base; opens, updates, and closes it; refuses while a release is incomplete; the `proposal` keys replace `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`; proof: the version-proposal identity and lifecycle scenarios pass
  - 2026-11-09, partly shipped: `prepare --pr` uses the derived branch `verctl/release/<base>` ([#161][pr161]). Remaining: the `propose` verb, the refusal while a release is incomplete, and `proposal` keys in place of `VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`, which shipped as the interface in [#155][pr155].
  - 2026-11-09, the `proposal` keys are in 0.8.0 through [version-pr-proposal](../version-pr-proposal/proposal.md) (VER-090); the `propose` verb and the refusal stay for later.
- [ ] 3.2 `check` enforces identity and content; the label, the branch exemption, and the `labeled` and `unlabeled` CI types are removed; proof: the version-proposal `check` scenarios pass
  - 2026-11-09, partly shipped: `check` knows the Version PR by identity, reads no label, and CI dropped the `labeled` and `unlabeled` types ([#188][pr188]). Remaining: the content check, and removing `prepare.version_label`, which still labels the pull request.
  - 2026-11-09, the label is removed in 0.8.0 through [version-pr-proposal](../version-pr-proposal/proposal.md) (VER-090); the content check stays for later.

## 4. Config, served files, docs

- [ ] 4.1 The final schema: every removed key refused with its replacement, every struct refusing unknown keys, every name resolved at load; proof: the release-config scenarios pass
  - 2026-11-09, partly shipped: the config is read strictly through ctl-core `Input`, and 9 structs refuse unknown keys. Remaining: every key this change removes is still read, and none of its new keys exists.
- [ ] 4.2 `templates.rs` renders through `ctl_core::served`, with the `tags` map and the mount line in the context, after telling the session that owns CTC-008. `render` replaces `pin`. Operator docs and example workflows are served templates. Proof: served bytes are identical before and after for every template in this repository and qctl, and `check` fails on a hand-edited served file
  - 2026-11-09, not started.
- [ ] 4.3 AGENTS.md, README.md, `src/instructions.md`, the operator skill, and `docs/served-files.md` describe only 0.8.0. The instructions and the operator skill state that whether a release publishes is read at its release commit, so a later `publish = false` does not cancel a merged version. Proof: `cargo test operator_docs` passes and no document names a removed key or command
  - 2026-11-09, not started.

## 5. Release 0.8.0

- [ ] 5.1 `release.yml` replaces `version-pr.yml` and `publish.yml` in this repository, and the asset jobs attest each archive; proof: the 0.8.0 release passes CI, and `gh attestation verify` passes for each 0.8.0 asset
  - 2026-11-09, partly shipped: the reusable `release.yml` replaced `version-pr.yml` and `publish.yml` ([#166][pr166]). Remaining: attestation, which no workflow runs yet.
- [ ] 5.2 The 0.8.0 minor merges with one `minor` fragment per breaking change; proof: the 0.8.0 Version PR passes `check` on its first run, and the release goes from merge to a published Release with both assets; a rerun reports every step `already`
  - 2026-11-09, not started.
- [ ] 5.3 qctl, forkctl, ctl-core, unixd, and yamled move to 0.8.0: delete `prepare`, adopt the two workflows; qctl and forkctl move four pinned files to templates; proof: each repository's next Version PR commits its lockfile and passes `check`
  - 2026-11-09, not started.

## 6. Patches after 0.8.0

- [ ] 6.1 `auth: oidc` for crates.io and npm; proof: the building-blocks OIDC scenarios pass, and one real crates.io release runs with no `CARGO_REGISTRY_TOKEN` secret
  - 2026-11-09, partly shipped: `release.yml` publishes to crates.io through trusted publishing with `crates-io-trusted-publishing` ([#218][pr218]). Remaining: npm OIDC, and `auth: oidc` as a config key.
- [ ] 6.2 `provenance: true`, staged publish, and `release bootstrap` in the bun registry client; proof: one real bun-release version publishes with provenance through `verctl release`, and bun-release is archived
  - 2026-11-09, not started.
- [ ] 6.3 The `bootstrap` stage stops a first publication under OIDC; proof: the release-state bootstrap scenario passes
  - 2026-11-09, not started. 6.5 covers the detection this needs, and replaces the stop with a token publish when the caller passes a token.
- [ ] 6.4 The built-in `mise-pin` step, and `MISE_LOCKED_VERIFY_PROVENANCE=1` on `mise install --locked` in the served CI workflow. Proof: the building-blocks pin scenarios pass, a consumer's lock regenerated after 0.8.x records `provenance = "github-attestations"`, and that consumer's CI install passes with the variable set
  - 2026-11-09, not started.
- [ ] 6.5 A first publish works beside trusted publishing. The publish plan reports, for each crates.io package, whether the crate exists. The reusable `release.yml` accepts `registry-token` together with `crates-io-trusted-publishing`. It publishes a crate that does not exist yet with the token, and every other crate through trusted publishing. Proof: a plan test with a mocked sparse index reports a missing crate, and one real first publish of a new crate passes under a caller that sets both
  - 2026-11-09, not started. crates.io refused a new crate under trusted publishing alone, because trusted publishing is configured on a crate that already exists. Today `release.yml` refuses the two together ([`release.yml:314`][release-yml]). Patch-safe: the plan gains a field, and a combination that fails today starts to work.

## 7. Close

- [ ] 7.1 Archive these rows when 0.8.0 or the patch that removes their cause releases. The rows are VER-013, VER-016, VER-020, VER-028, VER-029, VER-030, VER-031, VER-033, VER-034, VER-035, VER-036, VER-038, and VER-010. The evidence is that pull request. Proof: `mise run q check` passes
- [ ] 7.2 Archive this change and VER-011 after section 6; proof: `openspec validate --specs` passes

[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
[stack]: https://github.com/victor-software-house/verctl/pull/256
[release-yml]: https://github.com/victor-software-house/verctl/blob/cb1be28/.github/workflows/release.yml#L314
[pr155]: https://github.com/victor-software-house/verctl/pull/155
[pr161]: https://github.com/victor-software-house/verctl/pull/161
[pr162]: https://github.com/victor-software-house/verctl/pull/162
[pr166]: https://github.com/victor-software-house/verctl/pull/166
[pr188]: https://github.com/victor-software-house/verctl/pull/188
[pr218]: https://github.com/victor-software-house/verctl/pull/218
