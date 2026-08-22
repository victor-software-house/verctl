---
verctl: patch
---

`actions/publish` fetches the default branch at depth 1 before pointing `origin/HEAD` at it, so a shallow checkout of the merge SHA still has that ref.
