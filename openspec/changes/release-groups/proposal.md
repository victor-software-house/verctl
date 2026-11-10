# Proposal

## Why

verctl names release tags with one template, `tags.template`, default `v{version}`.
With `{name}` in it, each package gets its own tag and Release; without it, packages whose versions differ are refused.
There is no way to say that some packages always share a version and others move on their own.
Changesets (`fixed`, `linked`), Nx (`projectsRelationship`), and release-please (`linked-versions`) all offer that.

Go also forces a tag shape.
A module in a subdirectory is released by the tag `<dir>/v<version>`, and the Go toolchain reads no other form.

Queue row: [VER-083][ver-083].

## What Changes

1. `groups` names sets of packages that share one version: `groups: {cli: {packages: [verctl, verctl-core]}}`.
   A fragment that bumps one member bumps the group to the highest bump in it, as Changesets `fixed` does.
   A package in no group moves on its own.
2. With no `groups` key, verctl reads `fixed` and `ignore` from `.changeset/config.json` when the file exists, so a repository moving from Changesets keeps its groups without restating them.
   `linked` has no equivalent and fails the load with a message that names the group to write instead.
3. A single-package repository keeps `v{version}`.
   A repository with more than one package or group defaults to `{name}/v{version}` (operator, 2026-11-09).
   No shape is a common standard:
   - Changesets, Lerna, and Nx default to `{name}@{version}`;
   - release-please and release-plz default to `{name}-v{version}`;
   - Knope defaults to `{name}/v{version}`.

   The slash has three effects:
   - it makes each package a tag namespace (`git tag -l 'cli/*'`);
   - it gives a floating major the plain form `cli/v1`;
   - it parses exactly from the last `/v`, even for a scoped npm name or a name holding `-v`.
4. Adoption never retags.
   When a package already has release tags, verctl reads the newest one that parses as a version and keeps that tag's shape.
   So a repository tagged `v0.0.75` by hand goes on with `v0.0.76`.
   A tag shape verctl cannot match against `tags.template` fails `check` and names the template that fits.
5. A Go module in a subdirectory is always tagged `{dir}/v{version}`, because the Go toolchain reads no other form.
   A template that would give it another tag fails the load.
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

[ver-083]: ../../../tasks.yaml
