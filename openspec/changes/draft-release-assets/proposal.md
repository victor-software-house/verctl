# Keep a Release with assets a draft until every asset is uploaded

## Motivation

GitHub can make a repository's Releases immutable: once a Release is published, its assets and its tag cannot change ([immutable releases][immutable]).
GitHub's advice is to create the Release as a draft, attach every asset, then publish it.

`verctl release` creates every Release published, and `release asset` uploads afterwards.
With immutable Releases on, the upload is refused and a Release ships without its tarballs.
A Release that holds some declared assets and not others also counts complete once published, so nothing finishes it.

Queue row: [VER-011][ver-011], the drafted and finalize slice.
It reuses the design of the closed pull request 228, adapted to the 0.8 verbs and code.
Its `assets.files` key stays out, because [VER-081][ver-011] gives that key another meaning.

## Changes

1. When a tag's Release declares an asset target and `--skip-assets` is not set, `release` creates the tag through the git refs API at the version commit, then the Release as a draft.
   With no declared asset, the Release is published at once, as before.
2. Every Release lookup sees drafts.
   `GET /releases/tags/{tag}` hides them, so on a 404 the lookup reads `GET /releases`.
   Only an asset whose state is `uploaded` counts.
3. `release asset` uploads to the draft.
4. `release` run again at a commit whose Release is a draft publishes it once every declared asset is uploaded.
   Otherwise it reports `draft <url> lacks <names>` and succeeds.
5. A draft is incomplete, even with every asset.
   A published Release that lacks a declared asset is incomplete when GitHub holds it mutable, with only the missing targets planned.
   When GitHub holds it immutable it counts complete, plans no asset job, and `status` and `release` warn that a later version must ship the asset.
6. The reusable workflow gains a `finalize` job after `assets`, and its `plan` job needs `contents: write`, because GitHub lists drafts only to a token with push access.

## Capabilities

### New Capabilities

- `release-assets`: a Release with declared assets stays a draft until every asset is uploaded.

### Modified Capabilities

None.

## Impact

1. Code: `src/github.rs`, `src/publish.rs`, `src/pending.rs`, `src/main.rs`, `src/presentation.rs`, tests.
2. Workflow: `.github/workflows/release.yml`, a new `finalize` job and `contents: write` on `plan`.
3. Docs: `src/instructions.md` with its template, the operator skill, and the README.
4. Release: a patch with a Breaking entry.
   A caller must grant `contents: write` to the whole called workflow, and callers already grant it for `publish`.
   Every library field it adds is on a `#[non_exhaustive]` type.

[immutable]: https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
[ver-011]: ../../../tasks.yaml
