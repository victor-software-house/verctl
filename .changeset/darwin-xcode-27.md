---
verctl: patch
verctl:group: changed
---

The built-in `darwin-arm64` target builds on `xcode-27`, GitHub's macOS 27 image, where it used `macos-latest`.
GitHub serves that image as a public preview, with weekly updates and no Actions SLA, until a GA `macos-27` label exists.
The binary's deployment target stays Rust's default for `aarch64-apple-darwin`, so it still runs on older macOS releases.
