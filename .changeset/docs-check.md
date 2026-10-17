---
verctl: patch
---

New `verctl docs` waits for docs.rs to build each crates.io library crate whose release tag names HEAD, and fails naming any crate docs.rs failed to build or had not built within `--timeout` minutes (default 60). verctl 0.4.5 shipped without a docs.rs build and nothing noticed; the release workflow's `docs` job now runs this after every publish.
