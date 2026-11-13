---
verctl: patch!
---

The reusable release workflow loses its `docs` job and its `docs` input, and the `verctl docs` verb is removed with no hidden one left behind.
verctl waits for no docs.rs build: docs.rs builds every crates.io release on its own, and a repository checks its docs build in pull request CI.
A caller that passes `docs:` to the workflow, or runs `verctl docs`, drops it.
