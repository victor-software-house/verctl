---
verctl: patch
---

`verctl publish` fetches the one missing default-branch ref with its history, not at depth 1, so the ancestry proof can walk parents. It rewrites `origin/HEAD` only from `VERCTL_DEFAULT_BRANCH`, never from `GITHUB_BASE_REF`.
