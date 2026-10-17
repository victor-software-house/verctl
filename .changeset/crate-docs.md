---
verctl: patch
---

The docs.rs page builds again and has a real landing page: a quick start, how a release flows through the modules, and the crate's features. docs.rs renders with `--cap-lints warn`, so a lint that only its newer nightly fires no longer fails the build. Every fallible public function documents when it returns an error.
