# Proposal

## Why

verctl names release tags with one template, `tags.template`, default `v{version}`.
With `{name}` in it, each package gets its own tag and Release; without it, packages whose versions differ are refused.
There is no way to say that some packages always share a version and others move on their own, which Changesets (`fixed`, `linked`), Nx (`projectsRelationship`), and release-please (`linked-versions`) all offer.
Go also forces a tag shape: a module in a subdirectory is released by the tag `<dir>/v<version>`, and no other form is read by the Go toolchain.

Queue row: VER-083.

## What Changes

1. `groups` names sets of packages that share one version: `groups: {cli: {packages: [verctl, verctl-core]}}`.
   A fragment that bumps one member bumps the group to the highest bump in it, as Changesets `fixed` does.
   A package in no group moves on its own.
2. With no `groups` key, verctl reads `fixed` and `ignore` from `.changeset/config.json` when the file exists, so a repository moving from Changesets keeps its groups without restating them.
   `linked` has no equivalent and fails the load with a message that names the group to write instead.
3. A single-package repository keeps `v{version}`.
   A repository with more than one package or group defaults to `{name}@{version}`, the Changesets form.
4. Adoption never retags. When a package already has release tags, verctl reads the newest one that parses as a version and keeps that tag's shape, so a repository tagged `v0.0.75` by hand goes on with `v0.0.76`.
   A tag shape verctl cannot match against `tags.template` fails `check` and names the template that fits.
5. A Go module in a subdirectory is always tagged `{dir}/v{version}`, and a template that would give it another tag fails the load.
6. A group gets one tag and one Release, whose body holds each member's CHANGELOG section.
7. Floating tags are opt-in: `tags.floating: [major]` moves `v1` (or `{name}/v1`) to each new release in that major, as GitHub Actions consumers expect.
   They are lightweight pointers outside the immutability rules, and no Release names them.

## Capabilities

### New Capabilities

- `release-groups`: fixed groups of packages and the tag each package or group gets.

### Modified Capabilities

- `release-tag-order`: tags are created per package or per group.

## Impact

1. Code: `src/config.rs` (`groups`, tag defaults), `src/bump.rs`, `src/publish.rs`, `src/release.rs`, tests.
2. A repository with one package, or one that already sets `tags.template`, sees no change. A Changesets repository keeps its groups and its tag history.
3. Depends on [`package-discovery`](../package-discovery/proposal.md).
