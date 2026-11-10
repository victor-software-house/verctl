# Proposal

## Motivation

A package whose files changed with no fragment naming it is released without the change, and nothing reports it.
[`package-discovery`](../package-discovery/proposal.md) finds the packages.
This change records which files each one owns, so `check` can tell a changed package from an unchanged one.

Queue row: [VER-083][ver-083].
This change was split from `package-discovery` at one boundary of its work.
That change finds and names the packages, and this one maps files to them.

## Changes

1. Each package owns files.
   By default these are the directory of its version source and everything below it that no deeper package owns.
   Turborepo uses the same rule to map a changed file to its package.
   `files` overrides it with globs, for a single-file package such as one reusable workflow.
2. `check` reports a package whose owned files changed since the merge-base with no fragment naming it, as Bumpy's `check --strict` does.
   An unchanged package is then never bumped, and a changed one is not forgotten.
   A file no package owns is reported once and counts against no package.

## Capabilities

### New Capabilities

- `package-ownership`: which files each package owns, and the `check` report of a changed package with no fragment.

## Impact

1. Code: `src/config.rs` (`files`), the `check` command, tests.
2. Depends on [`package-discovery`](../package-discovery/proposal.md) for the packages it maps files to.

[ver-083]: ../../../tasks.yaml
