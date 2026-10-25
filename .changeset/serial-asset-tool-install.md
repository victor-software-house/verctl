---
verctl: patch
---

The release workflow's asset jobs install tools one at a time. On macOS runners rustup failed to rename a component download when two Rust toolchains installed at once, which left a release without its darwin-arm64 asset.
