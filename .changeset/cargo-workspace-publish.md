---
verctl: patch
---

A cargo package at a virtual workspace manifest publishes each workspace member that publishes, dependencies first, with `cargo publish --package <member>` under its own crates.io index check. A rerun after a partial release uploads only the members still missing, and a registry rate limit stops the run before any tag, naming the crate and the time to retry. Before this, such a repository overrode the upload with `cargo publish --workspace`, which cannot resume, and a rerun could tag a release whose later crates were never uploaded.
