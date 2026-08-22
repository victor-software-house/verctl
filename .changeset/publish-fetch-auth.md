---
verctl: patch
---

`actions/publish` skips the default-branch fetch when `origin/<default>` already exists, and fetches with `x-access-token` when it does not. Bearer is not a git HTTPS credential.
