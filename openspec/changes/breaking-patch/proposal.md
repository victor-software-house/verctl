# An API break shipped as a patch from a fragment

## Motivation

The verify job on [#321][pr-321] fails its Semver step.
The publish options change replaced the public field `PackageSpec.publish_argv` with `PackageSpec.publish`.
Against verctl 0.8.0, cargo-semver-checks reports `struct_pub_field_missing`.
The library has no callers outside verctl, and the change is meant to ship in the next patch.

The only way to ship it as a patch today is a `[package.metadata.cargo-semver-checks.lints]` table in `Cargo.toml`.
The release that removed Version PR auto-merge did that in [#230][pr-230].
[#235][pr-235] then had to remove the table once 0.7.2 was on crates.io.
The table lowers a lint for every change until someone deletes it, and the changelog never mentions it.

A fragment already lives exactly as long as one release: the Version PR consumes it.
An exception written in the fragment ends with that release by itself.
Another repository of ours ships its upstream drift exception the same way.
A patch changeset carries it, the gate skips only for the release that consumes it, and the changelog lists it.

Queue row: [VER-121][ver-121].

## Changes

1. A fragment may bump a package by `patch!`: a patch release that breaks the package's API on purpose.
   `minor!`, `major!`, and `none!` fail the parse, because only a patch needs the mark.
2. `verctl semver` skips a crate whose package's pending release is a patch that a `patch!` fragment marks.
   Its row reads `skipped: patch! fragment`.
3. The CHANGELOG section of that release lists the entry under a `### Breaking` heading, above the other groups.
4. The next release checks the package again. No file needs to be deleted.

## Capabilities

### New Capabilities

- `breaking-patch`: the `patch!` bump and how a fragment carries it.

### Modified Capabilities

- `semver-check`: a `patch!` fragment skips the check for one release.
- `changelog-groups`: a `### Breaking` heading for `patch!` entries.

## Impact

1. Code: `src/fragment.rs` (the bump value), `src/semver_check.rs` (the decision), `src/release.rs` (the heading), `src/presentation.rs` (the row label), tests.
2. Bundle: `verctl instructions`, the operator skill, and `README.md`, through their templates.
3. Callers: none must change. A Changesets tool cannot read `patch!`, as it already cannot read `verctl:group`.
4. Release: a patch. The change breaks verctl's own library API, so a fragment marks `verctl: patch!`.
   That fragment also covers the publish options break, so the feature ships through itself.

[pr-230]: https://github.com/victor-software-house/verctl/pull/230
[pr-235]: https://github.com/victor-software-house/verctl/pull/235
[pr-321]: https://github.com/victor-software-house/verctl/pull/321
[ver-121]: ../../../tasks.yaml
