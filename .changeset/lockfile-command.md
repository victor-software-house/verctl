---
verctl: patch
---

The cargo driver's lockfile follow-up is `cargo update --workspace`, not
`cargo generate-lockfile`. A bump makes one line of `Cargo.lock` stale;
`generate-lockfile` re-resolved all 138 packages and moved a dependency the
bump never touched.
