---
verctl: patch
verctl:group: fixed
---

Every GitHub call now goes to `GITHUB_API_URL` when it is set, as GitHub Actions sets it, and to api.github.com otherwise. Before, only the Version PR identity check in `check --versions` read it, so a GitHub Enterprise Server run sent its other calls to api.github.com.
