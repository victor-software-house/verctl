---
verctl: minor
verctl:group: changed
---

`verctl release` now refuses a release commit whose `.github/workflows/` tree differs from the default branch's tip, before any upload, tag, or Release. A retry of an old version commit runs its workflows as they were then, and could publish to the registry and then fail to tag. `--allow-workflow-change` (`VERCTL_ALLOW_WORKFLOW_CHANGE`) lets the run go on.

`--allow-off-default-branch` (`VERCTL_ALLOW_OFF_DEFAULT_BRANCH`) lets `verctl release` run from a commit the default branch does not contain. A check that cannot run still refuses. A run that uses either flag lists what it went past under `allowed`, as a row in the human view and as `allowed: [{check, reason}]` in JSON.

`verctl release` now refuses a package with a skipped version, because publishing past it would skip that release for good. No flag allows it. A version v is skipped when some commit in the first-parent range from HEAD has v, v is below HEAD's version, and no newer commit in the range has a version strictly below v. The range ends before a released version, `0.0.0`, a revision with no version, or a version that is not semver, and otherwise at the root. An abandoned version, one that a later downward change superseded, is not listed. `verctl status --github-output` lists each skipped version once, at its oldest commit in the range, oldest first, so the workflow releases it before the newer one.

The reusable `release.yml` takes two boolean inputs, `allow-workflow-change` and `allow-off-default-branch`, default `false`, and passes them to verctl through `env:`.
