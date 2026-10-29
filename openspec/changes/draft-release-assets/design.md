# Design

## Context

`publish` creates each Release through the Releases API with
`target_commitish` set to the version commit, and GitHub creates the tag with
it. `pending` decides completeness from the registry, the tag, the Release,
and the declared target tarballs, reading the Release from
`GET /repos/{owner}/{repo}/releases/tags/{tag}`. That endpoint returns only a
published Release ([GitHub REST][get-by-tag]), so a draft is invisible to
every lookup verctl makes today.

## 1. States

For a tag that carries declared assets:

```text
tagged ──publish──▶ drafted ──assets --upload──▶ … ──publish──▶ released
```

A tag with no declared asset keeps today's single step to `released`.

1. **Declared assets** are the tarball of each `assets` target and the file
   name of each `assets.files` entry. They belong to the first package's tag,
   as target tarballs already do. `--skip-assets` declares none.
2. **Drafted.** The tag exists and its Release is a draft. `publish` creates
   the tag first through the git refs API, then the draft. A draft does not
   create its tag until it is published, and the tag is what `pending`, the
   docs check, and an unchecked upload's record read, so the tag comes
   first. An existing tag must name the version commit.
3. **Finalize.** `publish` on a drafted tag publishes the draft when every
   declared asset is `uploaded`, and otherwise reports
   `draft <url> lacks <names>` and succeeds, so a run that has not uploaded
   yet is not a failure.
4. **Released.** `pending` counts a release complete only when its Release
   is published. A draft is incomplete even when every asset is there, so
   the next run finalizes it.

A published Release found by `publish` is reported as today, whatever its
assets: verctl does not unpublish, and an immutable Release cannot change.

## 2. Finding a draft

Drafts are listed only by `GET /repos/{owner}/{repo}/releases`, and only to a
token with push access ([GitHub REST][list]). verctl reads that list, newest
first, 100 per page, and takes the Release whose `tag_name` is the tag. A new
draft is on the first page. The reusable workflow's `plan` job therefore gets
`contents: write`; with `contents: read` it would not see a draft and would
rebuild assets the draft already has.

Each asset carries a `state`; an upload that broke off is listed in another
state than `uploaded`. Only `uploaded` counts toward declared assets.

## 3. A repository's own files

`assets.files` names files that the repository builds in its own job, such
as a notarized installer. verctl does not build them.
`verctl assets --upload --file <path>` uploads one, and refuses a path that
is not declared, because an undeclared file would not hold the draft back
and could be missed. The tag defaults to the plan's tag, as for a target.

Alternatives considered:

1. **Upload with `gh release upload` and keep verctl out of it.** Lost:
   `publish` could not tell when the draft is complete, so it would either
   publish too early or need the asset names some other way.
2. **A `--draft` flag on `publish` and a separate `finalize` verb.** Lost:
   whether a Release is a draft follows from the declaration, and a flag
   could disagree with it. Running `publish` again is the finalize, as in
   VER-011's design, so a rerun at any stage is safe.
3. **The asset upload publishes the draft when it uploads the last asset.**
   Lost: parallel asset jobs race to publish, and an upload job would need
   to know every other target's state.

Public override and disable path: declare no assets, or pass
`--skip-assets` (`VERCTL_SKIP_ASSETS=true`), and the Release is published at
once.

## 4. The reusable workflow

`finalize` runs after `assets`, one leg per pending commit in order, and runs
`mise run ver publish` at that commit. It needs `contents: write` only. A leg
whose assets failed leaves its draft, and the next run's `plan` lists the
commit again, because a draft is incomplete.

[get-by-tag]: https://docs.github.com/en/rest/releases/releases#get-a-release-by-tag-name
[list]: https://docs.github.com/en/rest/releases/releases#list-releases
