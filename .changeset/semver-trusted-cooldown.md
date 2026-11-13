---
verctl: patch
---

`verctl semver` runs `cargo semver-checks` once more past the publish-age cooldown when every crate the cooldown rejected has a crates.io owner that `CARGO_TRUSTED_OWNERS` lists.
A check blocked by any other crate fails as before, and its error names the rejected crates.
