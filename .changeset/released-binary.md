---
verctl: patch
---

Actions run the released tarball, mise splits dev/release/ci, publish is exact-SHA plus a matching changelog heading, `check --versions` blocks hand edits, and `verctl pin` rewrites collocated github:verctl refs after the tarball exists.
