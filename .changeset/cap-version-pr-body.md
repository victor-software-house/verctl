---
verctl: patch
verctl:group: fixed
---

A Version PR opens when its changelog is longer than GitHub's 65,536-character body limit.
The body keeps every whole line that fits, drops a heading left at the end, and ends with a line that points at the changelog files in the PR's diff.
