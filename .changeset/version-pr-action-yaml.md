---
verctl: patch
---

The Version PR action loads again. A colon followed by a space in its `title` input description made the file invalid YAML, so GitHub failed every workflow that used the action at "Set up job", from v0.4.6 on.
