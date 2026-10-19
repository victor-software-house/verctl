# Publish a cargo workspace one member at a time

## Why

A repository whose crates share one version declares one package at its
virtual workspace manifest and overrides the upload with
`cargo publish --locked --workspace`. That command cannot finish a release
that stops partway, and a rerun reports a release that never happened:

1. `cargo publish --workspace` checks every selected crate first and refuses
   when any version already exists, so a rerun after a partial upload uploads
   nothing ([research](../release-state-machine/research.md), section 3,
   fact 1).
2. An overridden upload command has no registry check, so verctl decides
   "published" from stderr. A rerun fails with "already exists", verctl reads
   that as published, and it tags the commit and opens the Release while the
   crates after the first failure stay unpublished. This is gap W11 of
   [release-state-machine](../release-state-machine/design.md).
3. crates.io accepts a burst of 5 new crates per account, then 1 every 10
   minutes ([rate limits](https://crates.io/docs/rate-limits)). A first
   release of a workspace with 7 new library crates stops at the sixth with a
   429, so it needs at least three runs about 10 minutes apart.
4. The whole command runs under one 10-minute timeout. A workspace of 8 crates
   verify-builds every crate inside it.

Queue row: VER-069.
A patch: it changes what verctl does for a configuration that fails today,
and adds no configuration key.

It takes decisions 9 and 10 of
[release-state-machine](../release-state-machine/design.md) and the
building-blocks requirement "Cargo is a built-in" for this one case, ahead
of the breaking minor, as [reusable-release-workflow](../reusable-release-workflow/proposal.md)
took slice B. The orchestrator decided this approach on 2026-10-18, over a
per-repository publish task (which would duplicate verctl's logic) and a
crates.io limit exception (which would leave the false success).

## What changes

1. A package on the stock cargo publisher whose `path` is a virtual workspace
   manifest publishes each publishable workspace member with
   `cargo publish --locked --manifest-path <path> --package <member>`, in
   dependency order, each under its own timeout.
2. Before each upload verctl reads the member's crates.io index file, and
   skips a member that lists its version. A rerun after a partial release
   uploads only the members still missing.
3. A 429 from crates.io stops the run with a non-zero exit that names the
   member and the time crates.io gives for the next attempt. No tag or
   Release is created until every member is on the index.
4. The package's release is complete only when every publishable member's
   version is listed, and `verctl docs` waits for every member that has a
   library target.

## Impact

1. New `src/members.rs`; `src/publisher.rs`, `src/publish.rs`,
   `src/pending.rs`, `src/docsrs.rs`, and `src/registry.rs`.
2. Operator docs name the mode.
3. A repository that overrides the upload with `--workspace` drops the
   override to take it.
