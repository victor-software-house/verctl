---
verctl: patch
verctl:group: fixed
---

Every GitHub call goes to `GITHUB_API_URL` when it is set, as GitHub Actions sets it, and to api.github.com otherwise.  
Before, only `check --versions` read it, to check who opened the Version PR.  
So a run on GitHub Enterprise Server sent its other calls to api.github.com.
