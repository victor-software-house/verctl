---
verctl: patch
---

verctl no longer arms auto-merge on the Version PR: a release starts only when a person merges it. `prepare` drops `--auto-merge` and `VERCTL_AUTO_MERGE`, and the reusable `release.yml` and `actions/version-pr` drop their `auto-merge` input, so a caller that still passes `auto-merge` must remove it when it moves to this release.
