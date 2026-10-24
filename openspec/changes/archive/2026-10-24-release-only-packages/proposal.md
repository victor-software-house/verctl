# Release a package on GitHub only

## Why

A repository that ships private binaries sets `publish = false` on every crate.
verctl has no publisher that uploads nothing, so such a repository declares a
stand-in publish command: a check that succeeds only when every crate refuses
to publish. That command is a workaround for a missing mode.

Queue row: VER-057.
A patch: no public type changes, and `PackageSpec.publisher` already takes a name.

It fits [VER-011](../release-state-machine/proposal.md): in the release state
machine, a release-only package is one with no Registry block.
This change gives that state a name now.

## What changes

1. `publisher: none` on a package releases it on GitHub only.
   `publish` runs no upload command for it, then creates its tag and Release.
   `assets` builds its tarballs as before.
2. Such a package never counts as shipped.
   With no registry to ask, an existing Release under its tag proves it was released, as an already-published package does today.
3. A `registry` or `publish` setting beside `publisher: none` fails the load and names the package.

## Impact

1. `src/publisher.rs`, `src/publish.rs`, `src/config.rs`, tests, operator docs, README.
