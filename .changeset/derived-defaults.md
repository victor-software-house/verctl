---
verctl: patch
---

`prepare --pr` no longer guesses `main` for the Version PR base. It targets `GITHUB_REF_NAME`, then `VERCTL_DEFAULT_BRANCH`, then the branch `origin/HEAD` names, and fails before writing anything when none is set. An empty `VERCTL_PR_TITLE` or `VERCTL_COMMIT_MESSAGE` now counts as unset. The `version-pr` action defaults `title` and `commit-message` to empty, so verctl's own default applies; a set `title` without a `commit-message` now becomes the commit message too.
