---
verctl: patch
---

The cargo driver's lockfile follow-up is `cargo update --workspace`, not
`cargo generate-lockfile`. A bump makes one line of `Cargo.lock` stale;
`generate-lockfile` re-resolved all 138 packages and moved a dependency the
bump never touched.

Lockfile detection now stops at the repository root for cargo and JavaScript
alike. Both branches walk one shared scope, so a `Cargo.lock` or `bun.lock`
in some unrelated parent directory can no longer claim a manifest that is not
part of it.
