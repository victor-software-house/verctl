# Proposal

## Motivation

A consumer of the actions repository pins a commit SHA, because GitHub names a full SHA the only immutable reference to a reusable workflow.
A release tag is worth pinning against only when it cannot move.

GitHub [immutable releases][gh-immutable] lock a Release's tag and assets once it is published.
A tag with no Release is not covered, and a published Release refuses assets uploaded after it.

Consumers today pin `@<sha> # main, <date>`.
The pin shows no release for the commit, and nothing moves it when a release lands.

Queue row: [VER-083][ver-083].

## Changes

1. `publish` creates annotated tags only, and every release gets a GitHub Release, including `publisher: none` packages, so immutable Releases can cover every tag.
2. A Release with declared assets stays a draft until they are uploaded, as queue row [VER-011][ver-011] plans.
   The work closed in PR 228 is reused there.
3. `check --repository` reports whether the repository has immutable Releases on and a tag ruleset that blocks updates and deletions of release tags.
   It fails when `.ctl/ver.yaml` sets `tags.immutable: true` and either is missing.
4. Consumers pin `@<sha> # v<version>`, the form Renovate's GitHub Actions manager and the actions repository's `workflow_pins` policy already accept.
5. How a consumer's pin moves to a new release is the open decision of this change.
   The options are:
   - Renovate with the version comment;
   - a verctl verb run in the consumer;
   - a template pin bump followed by `copier update`.

## Capabilities

### New Capabilities

- `release-immutability`: annotated tags, a Release for every tag, and the repository settings that keep them fixed.

### Modified Capabilities

- `release-tag-order`: every tag is annotated and has a Release.

## Impact

1. Code: `src/git.rs` (annotated tags), `src/github.rs` (repository settings reads), `src/publish.rs`, tests.
2. Depends on the draft Release slice of queue row [VER-011][ver-011].
3. The actions and templates repositories turn on immutable Releases and the tag ruleset; their maintainers do that.

[gh-immutable]: https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
[ver-011]: ../../../tasks.yaml
[ver-083]: ../../../tasks.yaml
