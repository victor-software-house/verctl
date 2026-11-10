---
verctl: minor
verctl:group: changed
---

`verctl publish` now refuses a release commit whose `.github/workflows/` tree differs from the default branch's tip, before any upload, tag, or Release. A retry of an old version commit runs its workflows as they were then, and could publish to the registry and then fail to tag. `--allow-workflow-change` (`VERCTL_ALLOW_WORKFLOW_CHANGE`) lets the run go on.

`--allow-off-default-branch` (`VERCTL_ALLOW_OFF_DEFAULT_BRANCH`) lets `verctl publish` run from a commit the default branch does not contain. A check that cannot run still refuses. A run that uses either flag lists what it went past under `allowed`, as a row in the human view and as `allowed: [{check, reason}]` in JSON.

`verctl publish` now refuses a package whose first-parent history since its last release tag holds an earlier version commit, because publishing would skip that version. No flag allows it. `verctl publish --plan` now lists each such version at the commit that set it, oldest first, so the workflow releases it before the newer one.

The reusable `release.yml` takes two boolean inputs, `allow-workflow-change` and `allow-off-default-branch`, default `false`, and passes them to verctl through `env:`.
