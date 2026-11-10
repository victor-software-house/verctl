---
verctl: patch
verctl:group: fixed
---

`publish`, `render`, `assets`, and `check --versions` read a package's version the way `version` does: a failed read names the package and its file, and a `VERSION` package with no file yet reads as `0.0.0` instead of failing. A `VERSION` path whose directory is missing now fails the plan before `version` writes anything, and `check --versions` fails on a declared package whose file is missing instead of leaving it out of the report.
