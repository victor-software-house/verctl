# Check the docs.rs build of each released crate

## Why

verctl 0.4.5 and qctl 0.5.4 both shipped with no docs.rs build, and nothing
noticed.

Queue row: VER-066. This change was split from
[reusable-release-workflow][rrw] at its capability boundaries; it ships as a patch.

## What Changes

1. A new `verctl docs` waits for docs.rs to build each crates.io crate
   released at a version commit, and fails when the build failed.
2. The reusable workflow runs it in its own `docs` job, so a slow docs.rs
   queue never delays a publish or the Version PR.

## Capabilities

### New Capabilities

- `docs-build`: the docs.rs build check after a crates.io release.

### Modified Capabilities

None.

## Impact

1. Code: a new `src/docs.rs` and `src/cli.rs`.
2. Workflow: the `docs` job in `.github/workflows/release.yml`.

[rrw]: ../reusable-release-workflow/proposal.md
