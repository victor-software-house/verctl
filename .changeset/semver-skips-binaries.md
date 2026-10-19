---
verctl: patch
---

`verctl semver` leaves out a crate with no library target, which has no API to check. It ran `cargo semver-checks --package <crate>` on a binary-only crate, which cargo-semver-checks refuses, so a workspace package with a binary member failed.
