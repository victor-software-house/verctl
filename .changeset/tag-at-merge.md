---
verctl: patch
---

`publish` creates the tag and GitHub Release of a package whose uploads all have a registry check, or that has no upload, before its uploads, so a retry after a failed upload creates no tag that GitHub would refuse for a later workflow change. `docs` waits only for crates the crates.io index lists.
