# Design

## Context

`assets` builds one binary per target from the first package, at that package's version, and uploads it to the Release `publish` made.
A vendored library has none of those.
Its version is upstream's.
Its archive holds a library, headers, and a license.
Nothing else creates its Release.

## Goals and non-goals

Goals:

1. Publish a vendored build as Release assets with one verb, from a matrix the plan writes.
2. Leave every existing `assets` behaviour unchanged.

Non-goals:

1. Bumping a vendored version through fragments or the Version PR. The version is written by hand in `.ctl/ver.yaml`.
2. Building upstream sources. The repository's own build command does that; verctl runs it and packs what it names.
3. Archive formats other than `.tar.gz`.

## Decisions

### 1. A separate `vendored` section holds the sets

Each set is a named entry under `vendored`, with its own version and tag.

Alternative: optional `version`, `tag`, and `files` keys on `assets`.
It lost for three reasons:

- `assets` is one set tied to the first package
- its target keys share a mapping with its recipe keys
- a repository may need both its own binary and a vendored library

A separate section also keeps the existing path untouched, which is what makes it byte-identical.

### 2. Templates with named placeholders and defaults that fit the common case

`tag` defaults to `{name}-{version}`, and `archive` to `{name}-{version}-{id}.tar.gz`, where `{name}` is the set's key and `{id}` the target's.
`archive` must contain `{id}`, since two targets with one archive name would overwrite each other, and must end in `.tar.gz`, the only format verctl writes.

Alternative: the `assets` naming, `{bin}_{version}_{os}_{arch}`.
It lost because a vendored set has no binary and its consumer names platforms itself (`macos-arm64`), so the target key is the platform.

### 3. `dir` and `files` instead of one binary

The archive is `tar czf <archive> -C <dir> <files…>`, so each listed path keeps its place under `dir`, and a directory is packed whole.
Every listed path must exist after the build, or the build fails naming it.

Alternative: a list of source and destination pairs.
It lost because the build command already stages its output, so one base directory covers the case with less to declare.
The disable path for the build is to omit `build`, and an earlier workflow step builds instead.

### 4. The checksum file comes from GitHub's digest

After the archive upload, verctl reads the asset's `digest` (`sha256:<hex>`) from the Release and uploads `<archive>.sha256` holding `<hex>  <archive>`, the `sha256sum` format.

Alternative: hash the local archive.
It lost on reruns.
Two builds of one source tree are not byte-identical.
So a rerun whose archive upload reports `already` would publish a checksum of a file GitHub does not hold.

Deriving it from GitHub's digest makes the checksum agree with the stored archive whichever run uploaded it, and it needs no new dependency.

GitHub reports `digest` on every release asset ([REST API][assets-api]).

### 5. The upload creates a missing Release

`--upload` reads the Release for the tag, and creates it at `HEAD` when none exists, named after the tag, with the `notes` file as its body and `make_latest: false`.

A creation refused because another matrix job created it first reads the Release again.

An existing Release is used as it is, wherever its tag points.
The tag records which repository commit built the first asset and does not mark a version commit.

`make_latest: false` keeps a vendored Release from becoming the repository's latest, which GitHub otherwise does for every new published release ([REST API][releases-api]).

Alternative: a separate step that creates the Release before the matrix.
It lost because every matrix job would still need to handle the Release missing on a rerun, and one verb is simpler for a workflow.

### 6. An existing asset is not uploaded again

The archive and the checksum each reuse the existing duplicate handling: an asset of that name that is complete is reported as `already`.

### 7. A config needs a package or a vendored set

The load fails only when both `packages` and `vendored` are empty.
Alternative: keep `packages` required, so a repository that only vendors declares a placeholder package.
It lost because a placeholder package would pull the repository into the release loop it does not use.

## Risks

1. A hand-deleted archive with its checksum left behind leaves a stale checksum, reported as `already`. The instructions require deleting both.
2. The build runs under the same 15-minute limit as `assets.build`.

[assets-api]: https://docs.github.com/en/rest/releases/assets
[releases-api]: https://docs.github.com/en/rest/releases/releases#create-a-release
