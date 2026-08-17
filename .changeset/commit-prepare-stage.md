---
verctl: patch
---

`prepare --pr` commits dirty paths matching `[prepare].stage` from the same git-status walk as the unexpected-dirty check, including deletions. `[prepare].stage_ignored` opts into gitignored matches.
