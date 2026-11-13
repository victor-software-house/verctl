# Drop the docs.rs wait

## Motivation

The release workflow's `docs` job holds a runner after every crates.io publish while `verctl docs` polls docs.rs, three minutes or more per release, and prints nothing until it ends.
release-plz, cargo-release and cargo-dist publish and stop, with no docs.rs wait.
docs.rs builds every crates.io release on its own, and its guide checks a build before the release instead, with `cargo docs-rs` in CI ([docs.rs builds]).
This repository's `doc` task already builds rustdoc on nightly with the docs.rs metadata on every pull request.
A docs.rs build that still fails shows on docs.rs, and a crate owner can rebuild it from crates.io.
The operator chose to remove the wait on 2026-11-13.

Queue row: [VER-140][ver-140].

## Changes

1. The `docs` job and the `docs` input leave the reusable `release.yml`.
2. The `verctl docs` verb and its module go, with no hidden verb.
3. The docs.rs requirements leave `docs-build`, `release-tag-order` and `workspace-publish`.

## Release

A breaking patch, through a `patch!` fragment: a caller that passes `docs:` to the reusable workflow, or runs `verctl docs`, must drop it.

[docs.rs builds]: https://docs.rs/about/builds
[ver-140]: ../../../tasks.yaml
