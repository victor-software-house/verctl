# Design

## Context

See [proposal.md](proposal.md) for why, and [`release-assets`](specs/release-assets/spec.md) for what.

`release` creates each Release through the Releases API with `target_commitish` set to the version commit, and GitHub creates the tag with it.
`pending` decides completeness from the registry, the tag, the Release, and the declared target tarballs.
It reads the Release from `GET /repos/{owner}/{repo}/releases/tags/{tag}`, which returns only a published Release ([get a release by tag name][get-by-tag]), so a draft is invisible to every lookup.

## Decisions

### 1. States

For a tag that carries declared assets:

```text
tagged ──release──▶ drafted ──release asset──▶ … ──release──▶ released
```

A tag with no declared asset keeps today's single step to `released`.

1. **Declared assets** are the tarball of each `assets` target.
   They belong to the first package's tag, as target tarballs already do.
   `--skip-assets` declares none.
2. **Drafted.** A draft does not create its tag until it is published, and the tag is what `pending`, the docs check, and an unchecked upload's record read.
   So `release` creates the tag first through the git refs API ([create a reference][ref]), then the Release with `draft: true`.
   An existing tag must name the version commit.
3. **Finalize.** `release` on a drafted tag publishes the draft with `draft: false` when every declared asset is `uploaded`.
   A draft found with no tag, such as one made by hand, gets its tag at the version commit first, and a tag that names another commit fails.
   The update sends this run's title and notes, and the settings of its stage.
   A candidate repeats `prerelease` and `make_latest: false`.
   A stable Release sends `make_latest: legacy`, so an older version published late does not take the latest from a newer one.
   While an asset is missing it reports the draft as a warning, `draft <url> lacks <names>`, and succeeds, so a run before the uploads is not a failure.
   The report's `releases` stay URLs.
4. **Completeness.** A draft is incomplete, even with every asset, so the next run publishes it.
   A published Release with every declared asset is complete.

A Release with no declared asset is created as before: published at once, at `target_commitish`.
Only a Release with assets takes the tag-first path.

### 2. Published Releases that lack an asset

A published Release can lack a declared asset when an earlier verctl published it, or when a target was declared in the version commit and its job failed.
What happens next depends on `immutable`, which GitHub returns on every Release:

1. **Mutable.** The Release still takes assets, so it stays pending and only the missing targets are planned.
   This is the rule of [release-asset-targets](../release-asset-targets/proposal.md).
2. **Immutable.** GitHub refuses the asset and never unpublishes, so the Release counts complete and no asset job is planned.
   `status` and `release` print one warning naming the tag and the missing assets, and say that a later version must ship them.

The alternative, to keep an immutable Release pending, loses: no run could finish it, so every plan would list the commit and rerun its jobs forever.

### 3. Finding a draft

Drafts are listed only by `GET /repos/{owner}/{repo}/releases`, and only to a token with push access ([list releases][list]).
The lookup reads the tag endpoint first, and on a 404 pages the list, 100 per page, newest first, for the Release whose `tag_name` is the tag.
A new draft is on the first page, and only a tag with no Release at all reads every page, up to ten.
After ten pages the lookup finds no Release, which is right for a new draft, because it is the newest.
`release_state`, `ensure_release_as`, and the upload and checksum lookups all use it.

A draft's assets carry a `state`, and an upload that broke off is in another state than `uploaded`.
Only `uploaded` counts toward the declared assets.

`release asset` reads the Release first and sends nothing for an asset already `uploaded`.

A rerun at a version commit does not repeat an upload for a package the registry already lists.
`pending` marks it listed, and `release` reports it as `already`.
Without this, a finalize run would upload a package with an unchecked upload a second time.

### 4. A finished version commit

A `finalize` leg can run at a version commit whose Release another leg already finished, while an older draft still waits.
`release` there reports `nothing to release` and exits zero, instead of failing because HEAD is not an incomplete commit.
A commit that is not a version commit still fails before any side effect.

### 5. Earlier versions

The walk below HEAD for versions nobody released treated a version as released once its registry listed it or its tag existed.
A draft at an older version was therefore never planned again after a newer version commit landed.
An earlier version now counts as released only when its Release is not a draft, and a repository that declares assets walks even when HEAD is complete.
A draft at an earlier version becomes a pending commit with its missing targets, named by that version, and gets publish, assets, and finalize legs.
A listed older version is not a skipped one, so `release` at a newer commit does not refuse because of it.

### 6. One set of assets for the plan and the legs

The plan schedules only the targets that HEAD and the version commit both declare, while a leg at the commit used every target its config names.
`status --github-output` now writes `declared`, a map from each pending commit to its expected asset names.
The `publish` and `finalize` legs pass the entry for their commit as `VERCTL_DECLARED_ASSETS`, and `release` declares exactly those names.
A commit the plan expects no asset at runs with `VERCTL_SKIP_ASSETS`.
A hand-run `release` has no such variable and declares the commit's own targets.

### 7. A job without registry credentials

`finalize` holds no registry token, and a run there must never upload.
`release --releases-only` (`VERCTL_RELEASES_ONLY`) creates and publishes tags and Releases and starts no publisher.
It refuses a package whose version the registry does not list, so a publish that failed stops the leg.

### 8. Hand-written workflows

A workflow that runs `actions/publish` and then `actions/asset` has no `finalize` job.
It must run `verctl release` again at the version commit after its uploads, which publishes the draft.
The Breaking text and the operator instructions state it.
Draft publishing covers asset targets only: a vendored set's Release is still created published, so immutable Releases are not yet safe for a repository with vendored sets.

### 9. The reusable workflow

`finalize` runs after `assets`, one leg per pending commit in order, under the group `publish` uses.
Each leg installs the plan's verctl from HEAD's lock, as the other jobs do, and runs `verctl release` at its commit.
It needs `contents: write` and runs only when `.ctl/ver.yaml` declares an asset target, which the plan job now outputs as `has-assets`.
A leg whose assets failed leaves its draft, and the next run's `plan` lists the commit again.

The `plan` job needs `contents: write`, because with `contents: read` it would not see a draft and would rebuild assets the draft already holds.

Alternatives considered:

1. **A `--draft` flag and a separate `finalize` verb.** It lost because whether a Release is a draft follows from the declaration, and a flag could conflict with it.
   Running `release` again is the finalize, so a rerun at any stage is safe.
2. **The last asset upload publishes the draft.** It lost because parallel asset jobs would race to publish, and each would need every other target's state.
3. **Upload with `gh release upload` outside verctl.** It lost because `release` could not tell when the draft is complete.

Public override and disable path: declare no asset target, or pass `--skip-assets` (`VERCTL_SKIP_ASSETS=true`), and the Release is published at once.

[get-by-tag]: https://docs.github.com/en/rest/releases/releases#get-a-release-by-tag-name
[ref]: https://docs.github.com/en/rest/git/refs#create-a-reference
[list]: https://docs.github.com/en/rest/releases/releases#list-releases
