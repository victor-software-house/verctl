---
verctl: patch
---

`verctl publish` fetches `origin/<default>` at depth 1 only when that ref is missing. GitHub git HTTPS authenticates as `x-access-token`; the token is never in the URL and never sent as Bearer. The publish action no longer shells out to `git fetch`.
