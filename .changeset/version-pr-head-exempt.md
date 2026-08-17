---
verctl: patch
---

`check --versions` treats `GITHUB_HEAD_REF=version-packages` as the Version PR. Actions checkout is detached, so git HEAD is not that branch.
