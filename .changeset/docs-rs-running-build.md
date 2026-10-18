---
verctl: patch
---

`verctl docs` keeps waiting while docs.rs builds a crate. docs.rs reports `doc_status: false` both while a build runs and after it failed, and `docs` used to fail the release on the first such answer. A crate now fails only when the timeout runs out while docs.rs still reports no documentation.
