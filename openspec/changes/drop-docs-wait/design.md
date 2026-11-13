# Design

## 1. The docs build check before the release

**Decision.**
verctl waits for no docs.rs build.
The check that a crate's docs build is the pull request's `doc` task, which runs rustdoc on nightly with the docs.rs metadata, as docs.rs advises with `cargo docs-rs`.

**Alternative.**
Keep the job, off by default, or give it a progress line per poll.
It lost because the wait proves nothing the pull request's build did not, costs a runner for minutes on every release, and no other release tool does it.
