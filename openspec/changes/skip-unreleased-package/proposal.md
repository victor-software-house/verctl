# Leave a package at 0.0.0 out of the release plan

## Why

A new package starts at `0.0.0` and releases `0.0.1` first.
`publish --plan` does not know that: it plans every package whose registry does not list its version, and no registry lists `0.0.0`.
A crate that adopted the reusable release workflow at `0.0.0` failed every push to its default branch.
The plan named the commit where `0.0.0` first appeared, which predates the `ver` task, so the publish and docs.rs jobs failed with `no task ver found`, and the Version PR job, which runs after them, was skipped.
No `0.0.1` Version PR could open until one was opened by hand with `prepare --pr`.
Had that commit carried the `ver` task, `publish` would have tried to release `0.0.0`.

Queue row: VER-077.

## What changes

1. `pending` leaves out a package whose version is `0.0.0`, with or without a pre-release or build suffix: the same test `bump::apply` uses for a new package.
   It does so before any registry or GitHub lookup, so the placeholder costs no request.
2. `publish --plan` and `publish` therefore report `nothing to release` for a repository whose only package is at `0.0.0`, and the reusable workflow goes on to open the Version PR.
3. `publish` asks no changelog section of a package at `0.0.0`, so a released package beside a new one still passes the changelog check.
4. A package with a released version in the same repository is planned as before.
5. A package at `0.0.0` gets no tag, and a dry run leaves it out, so a new package beside a released one does not break a shared `v{version}` tag.

## Override

None. A `0.0.0` version is never a release under verctl's rules: `prepare` moves a new package to `0.0.1` or, with `first_minor: true`, to `0.1.0`.
To release a package, give it a fragment; the Version PR moves it off `0.0.0`.

Alternatives considered:

1. Treat `0.0.0` as complete in `state_of`. It lost because the registry and GitHub lookups would still run for a version that can never be released.
2. Tell repositories to adopt the workflow only after `0.0.1`. It lost because the first release is exactly what the workflow should do, and the instructions already say a package starts at `0.0.0`.

## Impact

1. `src/pending.rs`, `src/publish.rs`, and `src/bump.rs` share one test for an unreleased version.
2. The instructions, the operator skill, and the README state that a package at `0.0.0` is not planned.
3. A `patch` changeset ships it.
