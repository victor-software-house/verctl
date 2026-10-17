---
verctl: patch
---

The Version PR now comes from `verctl/release/<base>`, where `<base>` is the branch it targets, instead of `version-packages`. `check --versions` exempts that branch locally, and no longer exempts `version-packages`. `prepare --pr --auto-merge` (`VERCTL_AUTO_MERGE`) enables the squash auto-merge that `actions/version-pr` used to run from a shell script. Before anything is written, it refuses when no ruleset on the base requires a status check, or when its token equals `VERCTL_JOB_TOKEN`. The action now sets both variables and runs `verctl prepare --pr`. A workflow that publishes when the Version PR merges must accept the new head branch. Close an open `version-packages` pull request after upgrading.
