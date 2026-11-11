---
verctl: patch
verctl:group: added
---

A package's `publish` key now also takes a mapping of options for its publisher.
`publish: {verify: false}` makes cargo upload with `--no-verify` and keeps the crates.io check and the member-by-member workspace upload.
An option the publisher does not have fails the load.
A `Cargo.toml` with `publish = false` or `[]`, and a `package.json` with `"private": true`, now release on GitHub only with no `publisher: none`.
