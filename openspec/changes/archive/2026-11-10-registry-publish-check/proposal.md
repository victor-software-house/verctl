# Ask the registry whether a version is published

## Motivation

`verctl publish` runs the publisher every time and decides "already
published" from six substrings of its stderr. A rerun after a partial
failure works only when the registry's wording matches.

The operator ruled on 2026-10-15 that the minimal slice B of
[release-state-machine][rsm] is unpaused. This part is its decision 9: the
registry decides "published".
Queue row: [VER-066][ver-066].
This change was split from [reusable-release-workflow][rrw] at its capability
boundaries; it ships as a patch.

## Changes

1. A publisher recipe gains `query`, and the stock `cargo` and `bun`
   publishers get one: a read of the crates.io sparse index, and
   `bun pm view`.
2. `verctl publish` queries the registry before it runs a publisher. When the
   registry lists the version, the package is `already`. Otherwise the
   publisher runs, and the publish fails unless the registry lists the version
   within a bounded wait.
3. A package with no registry check keeps the stderr rule.

## Capabilities

### New Capabilities

- `publish-registry-check`: how verctl asks the registry, which packages have
  a check, and what a listed version means for the publisher.

### Modified Capabilities

None.

## Impact

1. Code: `src/publish.rs` and `src/publisher.rs`.
2. Dependency: `reqwest` 0.13.5 with `rustls`, approved by the operator on
   2026-10-17.

[rsm]: ../../release-state-machine/proposal.md
[rrw]: ../../reusable-release-workflow/proposal.md
[ver-066]: ../../../../tasks.yaml
