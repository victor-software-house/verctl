# Atomic, idempotent releases: analysis

Status: analysis, 2026-11-13.  
Section 4 holds what the operator decided; section 9 holds what is still open.  
Four OpenSpec changes carry the decisions: [`release-from-head`][ch-head], [`release-order`][ch-order], [`release-checks`][ch-checks] and [`first-publish`][ch-first].

## Contents

- [1. What failed in shrub 0.7.0](#1-what-failed-in-shrub-070)
- [2. What verctl does today](#2-what-verctl-does-today)
- [3. What the registries make impossible](#3-what-the-registries-make-impossible)
- [4. Decided](#4-decided)
- [5. Releases that land whole](#5-releases-that-land-whole)
  - [5.1 Enforcement by plan](#51-enforcement-by-plan)
  - [5.2 No commit hooks](#52-no-commit-hooks)
- [6. Rate limits and resuming a partial release](#6-rate-limits-and-resuming-a-partial-release)
- [7. What other release tools do](#7-what-other-release-tools-do)
- [8. Checks on pull requests](#8-checks-on-pull-requests)
- [9. Open decisions](#9-open-decisions)
- [10. Sources](#10-sources)

## 1. What failed in shrub 0.7.0

1. shrub is a workspace that publishes 9 crates together, at one version.
2. On 2026-10-26, [run 40251818311][shrub-070] published `shrub-parser` 0.7.0 to crates.io.
3. The next crate, `shrub-process`, was new in 0.7.0.
   crates.io refused it with `403 Forbidden: Trusted Publishing tokens do not support creating new crates`.
4. The run stopped there.
   One crate of 0.7.0 is on crates.io, `shrub-process` is not on crates.io at all, and the rest stay at 0.6.2.
5. Each later push retried the release at the version commit `1ecc0b5`.
   From 2026-11-11, each retry failed earlier, at `mise install --locked`, because mise 2026.11.6 refuses a locked download without a checksum, and that commit's lock has none for taplo.
6. verctl refuses to release a newer version while an older one is unreleased, so shrub can release nothing until 0.7.0 completes.
7. On 2026-11-13, shrub's `main` still holds 0.7.0, 24 commits past the version commit, with 3 fragments pending.

The first failure was knowable before any upload: the crate did not exist, and the run used trusted publishing.

## 2. What verctl does today

1. A repository's caller runs the reusable `release.yml` on every push to the default branch and on `workflow_dispatch`.
   One run plans, publishes, builds assets, publishes the draft Releases, and then opens or updates the Version PR.
   So merging the Version PR starts its release with no further step; a dispatch only retries.
2. The plan job runs at HEAD and lists every version commit whose release is incomplete, oldest first.
   A release is complete when the registry lists each package's version, and its tag and its published Release exist with every declared asset (`src/pending.rs`).
3. A publish job checks out each listed version commit and runs `verctl release` there.
4. `release` uploads each package in order.
   A version the registry already lists is skipped, so a rerun resumes where the last run stopped.
   A 429 stops the run before any tag, naming the crate and the time the registry gives.
5. Tags and Releases follow the uploads.
   A Release with declared assets stays a draft, and the `finalize` job publishes it once every asset job has uploaded.
6. `refuse_skipped_versions` stops a newer version of a package while an older version of it is unreleased (`src/publish.rs`).
7. `release --dry-run` prints the tags and packages it would create (`dry_run_plan` in `src/publish.rs`).
   It runs no `cargo publish --dry-run`, no registry check and no ownership check.

So a release is already idempotent: every step checks a fact first, and a rerun does only what is missing.
It is not all-or-nothing: nothing checks, before the first upload, that every upload can succeed.

## 3. What the registries make impossible

1. crates.io has no atomic or staged multi-crate publish.
   `cargo publish --workspace`, stable since Rust 1.90, verifies every crate and then uploads in dependency order, but stops at the first failure with no rollback.
   A rerun aborts on "already exists", and `--idempotent` is still an open request ([cargo#13397][cargo-13397]).
2. A published crates.io version cannot be removed.
   A yank only stops new resolution.
   A whole crate can be deleted only within 72 hours of publishing, or when it has one owner, few downloads and no reverse dependencies ([RFC 3660][rfc-3660]).
3. crates.io has no server-side dry run ([crates.io#1515][cratesio-1515]).
   `cargo publish --dry-run` checks the package and the build, never ownership, rate limits or server policy.
4. Trusted publishing cannot create a crate ([publish.rs][cio-publish-new]).
   The first version of every crate needs an API token.
5. npm refuses to republish a version, and unpublishing is limited to 72 hours with no dependents.
   Its staged publishing needs a human 2FA approval, and its batch approval is not atomic either.

So a release across several packages cannot be atomic on any registry we use.
Once one package is live, the only ways out are to finish the release, or to yank and ship a new version.

GitHub is not part of that problem.
verctl only creates a tag and a Release with assets, and a draft Release can be changed or deleted until it is published.

## 4. Decided

The operator decided these on 2026-11-13.

1. **Check everything first, and fail closed.**
   Before any upload, the run proves:
   1. every crate already exists on the registry and lists the repository's workflow as a trusted publisher, or the run holds a token that may create it;
   2. every crate has the owners the `owners` key lists, through the crates.io owners API;
   3. every package packages and builds, through `cargo publish --workspace --dry-run`;
   4. every declared asset builds.
2. **Order.**
   Build the assets and upload them to a draft Release, then publish to the registries in dependency order, then publish the tag and the Release last.
3. **A partial release is its own state.**
   `status` reports, for example, "partially published: 1 of 9 crates live", with the cause and the resume time when there is one.
   A rerun resumes it once the cause is gone.
4. **HEAD is the release reference.**
   A run releases the versions HEAD's manifests hold, from HEAD.
   It never releases an older version, before a new one or in the same run.
   Releasing a skipped version later is opt-in and separate, and Releases are immutable by default.
5. **The Version PR's merge is the signal.**
   The release starts on the push that lands it, with no manual step.
6. **A bump lands only with its release.**
   If a release cannot happen, nothing is bumped; section 5 is the mechanism.
7. **Checks are named, and add up.**
   1. `verctl check [CHECK]...` runs `fragments`, `served`, `versions`, `registry` and `trust`, as `cargo deny check` runs its named checks; bare `check` runs `fragments` and `served`, offline, and `--all` runs every check.
   2. `check registry` runs on every pull request with no token: each crate exists, has the owners the `owners` key lists, and shows whether its latest version came through trusted publishing.
      It needs the network, so it is a CI step of its own, like `deny:advisories`, and a rerun repeats it whatever the pull request's state.
   3. `check trust` proves the trusted publisher entry with an owner's token.
   4. `verctl release --dry-run` adds the package build, the replay of `version` and the asset builds, uploads nothing, and runs where the Version PR is proven (section 5).
   5. `check --versions` goes, with no hidden flag; `check versions` replaces it.
      The hidden renamed verbs `prepare`, `pin`, `publish` and `assets`, and the hidden `release --plan`, go too, reversing that part of the 2026-11-09 renames.
   6. A person types a short form for each flag, such as `-a` for `--all` and `-n` for `--dry-run`; a flag only CI sets through `VERCTL_*` stays long-only.
8. **First publish: `verctl release bootstrap`, then `verctl release trust`.**
   1. Run locally with the operator's crates.io token, `release bootstrap [PACKAGE]...` publishes the real code of each package the registry lacks, tags each standalone one, and runs `release trust`.
      It is a step under `release`, as `release asset <target>` is, and the name the state model of `derived-release-state` already plans, so no two names cover one step.
   2. `verctl release trust` lists each crate's trusted publisher entries and adds the repository's entry only when it is missing, because crates.io accepts duplicates, up to 5 per crate ([create.rs][cio-trustpub-create]).
      `release trust --dry-run` shows what it would add.
      The step reads the repository, workflow file and environment from the repository, so it takes no provider argument.
   3. `check trust` proves the entry and `release trust` adds it: `check` proves, and a `release` step acts.
   4. This follows npm, where the first publish is by hand and `npm trust` follows.
   5. Nothing is published only to hold a name: crates.io and npm both forbid it (section 10).
9. **The first version is the one main holds.**
   A standalone package first publishes at `0.0.0`, where verctl already starts it, and `check` fails a never-published package at any other version.
   A member of a shared-version group, such as a crate whose manifest sets `version.workspace = true`, first publishes at the group's current version.
10. **Tokens in CI are opt-in.**
    Trusted publishing stays the default.
    A repository may opt in with the boolean `bootstrap` input, beside `publish`, `assets` and `docs`, and the `registry-token` secret; `check registry` then counts a missing crate as one the run can create.
11. **Every break ships in a 0.8.x patch.**
    A `patch!` fragment carries it, and the CHANGELOG lists it under `### Breaking`, as 0.8.4 did.

The decision on HEAD drops `refuse_skipped_versions` and the oldest-first plan of section 2.
It also replaces the first draft of `VER-129`, about tools at old version commits: no run checks out an old commit, and `VER-129` now carries `release-from-head`.

## 5. Releases that land whole

A version bump reaches the default branch only with its release, and nothing lands between the two.

1. **The Version PR is proven before it lands.**
   1. `verctl release --dry-run` runs on the commit that will land and holds unreleased versions.
   2. A failure keeps the Version PR out, so nothing is bumped.
      Every failure shrub met in 0.7.0 is caught here.
   3. It also proves that the replay of `version` matches, so no pull request merged since the Version PR was prepared changes what it releases.
2. **The push that lands it publishes at once.**
   1. The release runs from HEAD, the commit the check proved.
   2. It builds the assets and uploads them to a draft Release, which it can still delete.
   3. It uploads to the registries by trusted publishing, which meets no rate limit (section 6).
   4. It then publishes the draft Release, and that creates the tag.
3. **The default branch freezes until the release completes.**
   1. While the default branch holds a version whose release is incomplete, `check registry` fails every other pull request and names the pending release.
   2. So HEAD never moves past an unreleased version commit, and a retry always runs at the version commit.
   3. The check first waits for the running release to finish, within its timeout.
      A pull request checked during a normal release then passes.
4. **A failure after the landing.**
   1. Only an external failure is left, such as a registry outage.
   2. The default branch stays frozen, `status` reports the partial release, and a rerun resumes it.
   3. A release that can never finish, such as one whose crate name someone else took, ends only by an operator act.
      The operator labels a pull request `verctl: unfreeze` that renames or removes the package, and the check lets that one through.

### 5.1 Enforcement by plan

GitHub enforces a required check on its own servers: the merge button and the merge API refuse a pull request until the named check passes on its latest commit.
Only an actor on the ruleset's bypass list can merge past it.
What it can enforce depends on the plan (section 10):

1. **Tier 1, a merge queue.**
   Organization public repositories have it, and private ones on Enterprise Cloud.
   1. The `release --dry-run` job is a required check on the merge-group commit, which is the commit that lands with the `MERGE` method.
   2. `check registry` is required on every merge group, which enforces the freeze.
2. **Tier 2, rulesets without a queue.**
   Public repositories on every plan have them, and private ones on Pro or Team.
   1. The `release --dry-run` job is required on the Version PR, with "require branches to be up to date".
      The tested head then holds the default branch's tip, and the merge commit has the same tree.
      The release on the landing push runs at the merge commit (`github.sha`), so its draft and its tag name that commit, never the Version PR's head.
   2. `check registry` is required on every pull request.
      The up-to-date rule makes each one run again after the Version PR lands, which enforces the freeze.
3. **Tier 3, neither.**
   Private repositories on Free.
   1. The operator runs `release --dry-run` by dispatch before merging the Version PR (section 5.2).
   2. The freeze is not enforced; `status` and the job summary report the pending release.

`verctl status` names the tier it finds, from the default branch's rules.
In every tier the release still runs the same proof before its first upload, so a failure there stops with nothing published.

Publishing inside the queue would remove the freeze, but GitHub can still drop a group after its last check passed, as when an entry is removed by hand.
A registry upload made in that group could not be undone ([pulp 089d712][pulp]).
The freeze gives the same result, a bump and its release with nothing between them, without that window.

### 5.2 No commit hooks

The design adds no git hook (operator, 2026-11-13).
A hook runs only in the clone that pushes, `--no-verify` skips it, and a merge through GitHub's button or API runs none.
A repository may also use another hook manager, or none, and may never push to its default branch.

So in tier 3 the operator dispatches `release --dry-run` before merging the Version PR.

## 6. Rate limits and resuming a partial release

crates.io limits each user, keyed by user id and action, not by token or crate ([rate_limiter.rs][cio-rate-limiter]):

| Upload | Burst | Then |
|:--|:--|:--|
| A new crate | 5 | 1 every 10 minutes |
| A new version of an existing crate | 30 | 1 a minute |

1. A trusted-publishing upload counts against neither bucket.
   Its token has no user, and the limit runs only for a user ([publish.rs][cio-publish-limit]).
   A crates.io maintainer confirmed it has no hard limit ([crates.io#11685][cio-11685]).
   So with trusted publishing, a release of new versions is never rate limited.
2. Only a new crate meets a limit, because its first version needs a user's token.
3. The 429 carries a `Retry-After` header with an HTTP date, and the same time in its message ([rate_limiter.rs][cio-retry-after]).
4. cargo does not retry a publish upload on a 429, with or without `--workspace`.
   It stops, and the crates already uploaded stay live.

What follows for the design:

1. A release by trusted publishing creates no crate, so it meets no rate limit.
2. `release bootstrap`, and a release with the `bootstrap` opt-in, can meet one.
   On a 429 each waits until `Retry-After`, then uploads again, so one run creates any number of crates.
3. A release that still stops on an external failure, such as an outage, is a partial release.
   `status` and the job summary name it, and a rerun, by dispatch or by "Re-run failed jobs", skips every crate the index lists.
4. Where the Version PR sits in its merge group does not matter.
   Under `HEADGREEN` the top entry's run decides the merge, and its commit becomes HEAD, so `release --dry-run` there covers the version bump.

## 7. What other release tools do

| Tool | Publishes from | Decides what is pending by | An older unpublished version | Commits on top of the release PR |
|:--|:--|:--|:--|:--|
| changesets/action | HEAD, on a push with no pending changesets | an `npm info` per package | ignored | with changesets pending, it updates the version PR and publishes nothing |
| release-plz | the release PR's last commit (`release_always = false`), or HEAD | a git tag, then `cargo info` per package | ignored | released with HEAD when `release_always = true` |
| release-please | does not publish; tags the release PR's merge commit | merged PRs labelled `autorelease: pending` | each merged release PR gets its own release | left for the next release PR |
| cargo-dist | the pushed tag | the tag alone | not applicable | not applicable |

1. No tool is atomic, and none checks every upload before the first.
2. release-plz publishes one crate at a time, stops at the first failure, and has no handling for a 429.
   A crate that went live without its tag is skipped on rerun and never gets the tag.
3. Each tool checks only the current versions on every push, which costs one registry lookup per package.

## 8. Checks on pull requests

1. `check:versions` runs in `verify` on every pull request.
   It fails any version change, up or down, outside the Version PR, and on the Version PR it replays `version` and compares.
   So a version goes down only through a change that skips the pull request checks.
2. The plan can still guard that case cheaply: HEAD's version below the highest the registry lists fails the plan.
3. Nothing runs a release dry run on a pull request today (section 2, item 7).
4. A mise task cannot make itself a dependency of another task.
   `depends` takes patterns, and an `optional = true` entry matches nothing without failing ([mise docs][mise-depends]), so `verify` could depend on `check:*` and every catalog task with that prefix would join it.
   Not checked against the pinned mise 2026.11.6.

## 9. Open decisions

1. **shrub, once it ships.**
   shrub's `main` already moved 24 commits past 0.7.0, which the freeze would have prevented.
   The plan for it: run `verctl release bootstrap` for `shrub-process` at 0.7.0, let the next Version PR consume the 3 pending fragments into a new version, and leave 0.7.0 of the other crates skipped.

`check:*` in `verify` is queued as templates `TPL-052`.
One-command setup of a ctl CLI is ctl-core `CTC-058`, and the checks run from a GitHub App are `CTC-059`; verctl pilots both.

## 10. Sources

- shrub's failed 0.7.0 release: [run 40251818311][shrub-070]
- `cargo publish --workspace` stabilization: [cargo#15636][cargo-15636]
- idempotent publish: [cargo#13397][cargo-13397]
- crates.io deletion: [RFC 3660][rfc-3660]
- no server dry run: [crates.io#1515][cratesio-1515]
- new crates refused to trusted publishing: [publish.rs][cio-publish-new]
- trusted publisher configs by API: [create.rs][cio-trustpub-create]
- name squatting: [crates.io policies][cio-policies], [npm disputes][npm-disputes]
- GitHub plans: [merge queue][gh-queue], [protected branches][gh-protected], [rulesets][gh-rulesets]
- rate limits: [crates.io docs][cratesio-rates], [rate_limiter.rs][cio-rate-limiter], [the limit only for a user][cio-publish-limit], [crates.io#11685][cio-11685]
- npm unpublish: [npm policy][npm-unpublish]
- npm staged publishing: [npm docs][npm-staged]
- GitHub immutable releases: [GitHub docs][gh-immutable]
- merge queue: [GitHub docs][gh-queue]
- a deploy from an evicted merge group: [pulp 089d712][pulp]
- release-plz: [release.rs][rplz-release], [`release_always`][rplz-config]
- changesets/action: [run.ts][cs-action]
- release-please: [design.md][rp-design]
- mise `depends`: [task configuration][mise-depends]

[shrub-070]: https://github.com/victor-software-house/shrub/actions/runs/40251818311
[ch-head]: ../../openspec/changes/release-from-head/proposal.md
[ch-order]: ../../openspec/changes/release-order/proposal.md
[ch-checks]: ../../openspec/changes/release-checks/proposal.md
[ch-first]: ../../openspec/changes/first-publish/proposal.md
[cargo-13397]: https://github.com/rust-lang/cargo/issues/13397
[cargo-15636]: https://github.com/rust-lang/cargo/pull/15636
[rfc-3660]: https://rust-lang.github.io/rfcs/3660-crates-io-crate-deletions.html
[cratesio-1515]: https://github.com/rust-lang/crates.io/issues/1515
[cratesio-rates]: https://crates.io/docs/rate-limits
[cio-publish-new]: https://github.com/rust-lang/crates.io/blob/6e5f2e47f6c3cf34fd7ccad959975c52e15fe4d2/src/controllers/krate/publish.rs#L205-L211
[cio-publish-limit]: https://github.com/rust-lang/crates.io/blob/6e5f2e47f6c3cf34fd7ccad959975c52e15fe4d2/src/controllers/krate/publish.rs#L262-L273
[cio-rate-limiter]: https://github.com/rust-lang/crates.io/blob/6e5f2e47f6c3cf34fd7ccad959975c52e15fe4d2/src/rate_limiter.rs#L130-L160
[cio-retry-after]: https://github.com/rust-lang/crates.io/blob/6e5f2e47f6c3cf34fd7ccad959975c52e15fe4d2/src/rate_limiter.rs#L75-L95
[cio-11685]: https://github.com/rust-lang/crates.io/issues/11685
[npm-unpublish]: https://docs.npmjs.com/policies/unpublish
[npm-staged]: https://docs.npmjs.com/staged-publishing
[gh-immutable]: https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
[gh-queue]: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
[pulp]: https://github.com/Generous-Corp/pulp/commit/089d7121ecbd49af80991c3d88062fc4a3f690bf
[cio-policies]: https://github.com/rust-lang/crates.io/blob/6e5f2e47f6c3cf34fd7ccad959975c52e15fe4d2/svelte/src/routes/policies/+page.svelte#L68
[npm-disputes]: https://docs.npmjs.com/policies/disputes
[gh-protected]: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches
[gh-rulesets]: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets
[cio-trustpub-create]: https://github.com/rust-lang/crates.io/blob/6e5f2e47f6c3cf34fd7ccad959975c52e15fe4d2/src/controllers/trustpub/github_configs/create.rs#L24-L36
[rplz-release]: https://github.com/release-plz/release-plz/blob/0f401ecb1f8b629b335011f9abbbf14acc5dbc3a/crates/release_plz_core/src/command/release.rs#L691-L705
[rplz-config]: https://github.com/release-plz/release-plz/blob/0f401ecb1f8b629b335011f9abbbf14acc5dbc3a/website/docs/config.md#L700-L740
[cs-action]: https://github.com/changesets/action/blob/615034bb14e5d240e559f941ce9769beb5e14fb0/src/run.ts
[rp-design]: https://github.com/googleapis/release-please/blob/ea20bc261acd75149f0be90890bb9c90e681eea7/docs/design.md#L99-L106
[mise-depends]: https://github.com/jdx/mise/blob/a4a1996f49779db556570678c81f658e5a28fe19/docs/tasks/task-configuration.md#depends
