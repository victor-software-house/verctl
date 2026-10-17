---
verctl: patch
---

The crate documentation builds on docs.rs again. The public `process::run_inherit` docs linked to a private constant, which `private_intra_doc_links = "deny"` turns into an error under rustdoc, so every docs.rs build since 0.3.14 failed.
