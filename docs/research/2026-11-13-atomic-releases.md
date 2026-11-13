# Atomic, idempotent releases: analysis

Status: analysis for the operator to decide on, 2026-11-13.  
Nothing here is decided, and no spec changes until the operator chooses.

## Contents

- [1. What failed in shrub 0.7.0](#1-what-failed-in-shrub-070)
- [2. What verctl does today](#2-what-verctl-does-today)
- [3. What the registries make impossible](#3-what-the-registries-make-impossible)
- [4. The closest sound release](#4-the-closest-sound-release)
- [5. Releasing from the merge queue](#5-releasing-from-the-merge-queue)
- [6. Drift between the merge and the release](#6-drift-between-the-merge-and-the-release)
- [7. Decisions for the operator](#7-decisions-for-the-operator)
- [8. Sources](#8-sources)

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

The first failure was knowable before any upload: the crate did not exist, and the run used trusted publishing.

## 2. What verctl does today

1. The plan job runs at HEAD and lists every version commit whose release is incomplete, oldest first.
   A release is complete when the registry lists each package's version, and its tag and its published Release exist with every declared asset (`src/pending.rs`).
2. A publish job checks out each listed version commit and runs `verctl release` there.
3. `release` uploads each package in order.
   A version the registry already lists is skipped, so a rerun resumes where the last run stopped.
4. Tags and Releases follow the uploads.
   A Release with declared assets stays a draft, and the `finalize` job publishes it once every asset job has uploaded.
5. `refuse_skipped_versions` stops a newer version of a package while an older version of it is unreleased, because publishing past it would leave a gap no later run fills (`src/publish.rs`).

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
4. crates.io limits each user: 5 new crates in a burst, then 1 every 10 minutes, and 30 new versions in a burst, then 1 a minute.
   A larger release fails mid-way with HTTP 429.
5. npm refuses to republish a version, and unpublishing is limited to 72 hours with no dependents.
   Its staged publishing needs a human 2FA approval, and its batch approval is not atomic either.

So a release across several packages cannot be atomic on any registry we use.
Once one package is live, the only ways out are to finish the release, or to yank and ship a new version.

## 4. The closest sound release

A release can still get close to all-or-nothing: prove every upload will pass before the first irreversible step, and make every irreversible step last and resumable.

1. **Check everything first, and fail closed.**
   Before any upload, the run proves:
   1. every crate already exists on the registry, or the run holds a credential that may create a crate, since trusted publishing cannot;
   2. the publishing identity owns every crate, through the crates.io owners API;
   3. the release fits the rate limits, or the plan spaces the uploads to fit them;
   4. every package packages and builds, through `cargo publish --workspace --dry-run` or `cargo package --workspace`;
   5. every declared asset builds.
2. **Do the reversible work first.**
   Build the assets, create the Release as a draft, and upload every asset to the draft.
   A draft can still be changed or deleted, and its tag does not exist until it is published.
3. **Publish to the registries next, in dependency order.**
   Each upload skips a version the registry already lists, as today, so a rerun resumes.
4. **Publish the Release and its tag last.**
   An immutable Release then locks a tag whose packages are all live.
5. **Name a partial release as its own state.**
   `status` reports "partially published: 1 of 9 crates live" with the cause, in place of a plain incomplete release.
   The two ways forward are a resumed run once the cause is fixed, or a yank and a new version.

Limits that stay:

1. A registry can still fail after the checks pass, such as an outage or a policy change.
   The release then waits half-published until a rerun, and `status` reports that state.
2. A skipped version must be identical to the one the registry holds.
   Cargo cannot compare them, so verctl trusts the version number, as today.

## 5. Releasing from the merge queue

The Version PR's merge is the signal that its commit is ready to release.
Releasing from the queue would mean the published commit is exactly the one the queue tested.

1. A `merge_group` job can publish, and `GITHUB_SHA` is the group's commit.
2. GitHub merges the group only after its required checks pass.
   With the `MERGE` method, the commit that lands is the one the queue tested.
   Every queue in the organization uses `MERGE`.
3. A group can still be evicted or rebuilt after a job in it ran.
   Three things rebuild the later groups: a failing check of an earlier entry, a timeout, or a manual removal.
   A job that already published has then shipped a commit that never lands.
4. One project shipped production from such a commit, and fixed it by naming the events allowed to deploy ([pulp 089d712][pulp]).
5. A registry upload cannot be undone, so a publish inside the queue cannot be retracted when the group fails.

The closest sound use:

1. **In the queue, run the whole release in dry-run mode, as a required check of the Version PR.**
   That covers every check of section 4, item 1, against the commit the queue will land.
2. **After the merge, release at once from the landed commit.**
   A `push` to the default branch whose commit is the Version PR's merge starts the release, which runs the same checks again and then publishes.
3. **Queue the Version PR at the top of its group, or alone.**
   With `HEADGREEN`, only the top entry's run must pass, so a Version PR inside a group could land with only part of its checks proven.

## 6. Drift between the merge and the release

The checks the queue ran can go stale between the merge and the release.
That happens when the release starts long after the merge, as shrub's retries did, or when something outside the repository changes in between.
Examples are a registry rule, a mise rule, or a crate that someone else claims.

1. Running the checks again at release time detects any drift, since they are cheap.
2. A failed check names what changed, and the run stops before the first upload.
3. A run long after the merge is a retry by definition.
   A retry that fails only because the version commit's environment broke, as with shrub's lock, needs an explicit way to move that environment.
   One option is an input naming a commit whose mise files the release jobs install from.
   The default stays the version commit's own environment.

## 7. Decisions for the operator

1. **Checks before the first upload:** add the checks of section 4, item 1, and fail closed?
2. **Order:** assets and draft first, then registries, then tag and Release last?
3. **Partial release:** report it as its own state with its two ways forward?
4. **Merge queue:** run the release dry run as a required check on the Version PR, and release right after the merge from the landed commit?
   Or publish inside the queue, and accept that an evicted group can leave a published version that never lands?
5. **Version PR in the queue:** require it at the top of its group, or alone?
6. **A broken old environment:** add an opt-in way to take a version commit's tools from another commit, and what to call it?
7. **Which pending versions a run releases:** keep every incomplete version, oldest first, as `refuse_skipped_versions` requires today, or change that rule?

Two rows wait on these answers: `VER-129`, the draft about installing only the release tools, and `VER-130`, the research row on which commit and which versions a run releases.

## 8. Sources

- shrub's failed 0.7.0 release: [run 40251818311][shrub-070]
- `cargo publish --workspace` stabilization: [cargo#15636][cargo-15636]
- idempotent publish: [cargo#13397][cargo-13397]
- ownership not checked first: [cargo#16181][cargo-16181]
- crates.io deletion: [RFC 3660][rfc-3660]
- no server dry run: [crates.io#1515][cratesio-1515]
- rate limits: [crates.io docs][cratesio-rates]
- npm unpublish: [npm policy][npm-unpublish]
- npm staged publishing: [npm docs][npm-staged]
- GitHub immutable releases: [GitHub docs][gh-immutable]
- merge queue: [GitHub docs][gh-queue]
- A deploy from an evicted merge group: [pulp 089d712][pulp]

[shrub-070]: https://github.com/victor-software-house/shrub/actions/runs/40251818311
[cargo-13397]: https://github.com/rust-lang/cargo/issues/13397
[cargo-15636]: https://github.com/rust-lang/cargo/pull/15636
[cargo-16181]: https://github.com/rust-lang/cargo/issues/16181
[rfc-3660]: https://rust-lang.github.io/rfcs/3660-crates-io-crate-deletions.html
[cratesio-1515]: https://github.com/rust-lang/crates.io/issues/1515
[cratesio-rates]: https://crates.io/docs/rate-limits
[npm-unpublish]: https://docs.npmjs.com/policies/unpublish
[npm-staged]: https://docs.npmjs.com/staged-publishing
[gh-immutable]: https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
[gh-queue]: https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-a-merge-queue
[pulp]: https://github.com/Generous-Corp/pulp/commit/089d7121ecbd49af80991c3d88062fc4a3f690bf
