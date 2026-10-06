---
verctl: patch
---

A failed driver command now names its argv and exit status, for example `driver command failed: false: exit status: 1`, so a command killed by a signal can be told apart from one that exited non-zero.
