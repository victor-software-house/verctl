---
verctl: patch
verctl:group: fixed
---

`publish`, `pin`, and `assets` read a package's version the way `prepare` does: a failed read names the package and its file, and a `VERSION` package with no file yet reads as `0.0.0` instead of failing. A `VERSION` path whose directory is missing now fails the plan before `prepare` writes anything.
