---
verctl: patch
---

`actions/version-pr` takes `auto-merge: true` to merge the Version PR once the base branch's required checks pass. It refuses a base branch with no required status check and the default `GITHUB_TOKEN`.
