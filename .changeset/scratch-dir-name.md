---
verctl: patch
"verctl:group": fixed
---

Two version PRs prepared in one process in the same clock tick get two scratch directories, where the second had failed with `File exists`.
