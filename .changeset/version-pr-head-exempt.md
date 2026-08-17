---
verctl: patch
---

`check --versions` treats the GitHub event label `verctl:version` as the Version PR. `prepare --pr` applies that label. Not `GITHUB_HEAD_REF`.
