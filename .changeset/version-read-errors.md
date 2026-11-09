---
verctl: patch
verctl:group: fixed
---

`publish`, `pin`, `assets`, and `check --versions` read a package's version the way `prepare` does: a failed read names the package and its file, and a `VERSION` package with no file yet reads as `0.0.0` instead of failing. A `VERSION` path whose directory is missing now fails the plan before `prepare` writes anything, and `check --versions` fails on a declared package whose file is missing instead of leaving it out of the report.
