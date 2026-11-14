---
verctl: patch
"verctl:group": fixed
---

Two version PRs prepared in one process in the same clock tick no longer share a scratch directory, where the second failed with `File exists`.
