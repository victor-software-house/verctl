# Proposal

## Why

A consumer of the actions repository pins a commit SHA, because GitHub names a full SHA the only immutable reference to a reusable workflow.
A release tag is worth pinning against only when it cannot move.
GitHub [immutable releases][gh-immutable] lock a Release's tag and assets once it is published, but a tag with no Release is not covered, and a published Release refuses assets uploaded after it.
Consumers today pin `@<sha> # main, <date>`: nothing tells them which release a commit is, and nothing moves the pin when a release lands.

Queue row: VER-083.

## What Changes

1. `publish` creates annotated tags only, and every release gets a GitHub Release, including `publisher: none` packages, so immutable Releases can cover every tag.
2. A Release with declared assets stays a draft until they are uploaded, as VER-011 plans; the work closed in PR 228 is reused there.
3. `check --repository` reports whether the repository has immutable Releases on and a tag ruleset that blocks updates and deletions of release tags, and fails when `.ctl/ver.yaml` sets `tags.immutable: true` and either is missing.
4. Consumers pin `@<sha> # v<version>`, the form Renovate's GitHub Actions manager and the actions repository's `workflow_pins` policy already accept.
5. How a consumer's pin moves to a new release is the open decision of this change: Renovate with the version comment, a verctl verb run in the consumer, or a template pin bump followed by `copier update`.

## Capabilities

### New Capabilities

- `release-immutability`: annotated tags, a Release for every tag, and the repository settings that keep them fixed.

### Modified Capabilities

- `release-tag-order`: every tag is annotated and has a Release.

## Impact

1. Code: `src/git.rs` (annotated tags), `src/github.rs` (repository settings reads), `src/publish.rs`, tests.
2. Depends on the draft Release slice of VER-011.
3. The actions and templates repositories turn on immutable Releases and the tag ruleset; their maintainers do that.

[gh-immutable]: https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
