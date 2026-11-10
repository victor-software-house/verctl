# Proposal

## Motivation

`verctl publish --plan` fails for good once a release tag's commit leaves HEAD's first-parent history, even though HEAD still contains it.
A forkctl append publish leaves exactly that shape.

It rewrites the patch stack from its base, then merges the old remote tip back as the second parent, keeping the rewritten tree.
The tagged commit stays in HEAD's history, but only on that second parent.

The public fork `victor-software-house/serde-saphyr` hit this right after its first release.
Tag `v0.0.1` named `d5a586c`.
The next append publish made `293cc62` the first-parent commit at `0.0.1`, and every plan from then on, on push and on dispatch, failed with:

> tag v0.0.1 names d5a586c6365de2c75dd1831f1407d9066e18f4df: not the version commit 293cc62a1fb41cc172a75ebdc899df92549e2570 and not a first-parent commit at vsh-serde-saphyr@0.0.1

The two commits differ only in `.github/workflows/fork-publish.yml`.
The registry already had `0.0.1`, so the release was complete.

Queue row: [VER-084][ver-084].
A patch: it accepts a history that fails today and adds no configuration key.

## Changes

1. `git::tagged_commit` accepts a tag whose commit HEAD contains off the first-parent history, when the manifest version at that commit equals the version at HEAD.
2. Its depth is that of the merge that brought it in: the oldest first-parent commit that contains it.
3. A tag on a commit HEAD does not contain, or on a commit at another version, still fails the plan with the current message.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `release-plan`: which existing tag names a package's release commit.

## Impact

1. `src/git.rs` and the rustdoc of `src/pending.rs`.
2. The design of the open change `release-at-version-commit`, split from `reusable-release-workflow`, which stated the first-parent rule.
3. The operator instructions and skill do not state which tagged commits count, so they do not change.

[ver-084]: ../../../tasks.yaml
