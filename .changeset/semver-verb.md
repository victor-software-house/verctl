---
verctl: patch
---

`verctl semver` checks the API of each crates.io crate whose next release is a patch with `cargo semver-checks --release-type minor`, so only a break fails. It skips a crate whose package has a pending minor fragment or whose minor the Version PR already moved, and fails on a crate at 1.0 or later.
