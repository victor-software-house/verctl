---
verctl: minor
verctl:group: changed
---

`verctl release` now refuses a run whose `.github/workflows/` tree differs from the default branch's tip, before any upload, tag, or Release.  
Actions takes the workflows from `GITHUB_SHA`.  
The tree compared is the one at that commit when the variable names a commit in the repository, and the one at HEAD otherwise.

In the reusable workflow HEAD is the version commit, so a workflow change merged after a Version PR does not block its release.

A run from an old commit executes the workflows as they were then, and could publish to the registry and then fail to tag.  
`--allow-workflow-change` (`VERCTL_ALLOW_WORKFLOW_CHANGE`) lets the run go on.

`--allow-off-default-branch` (`VERCTL_ALLOW_OFF_DEFAULT_BRANCH`) lets `verctl release` run from a commit the default branch does not contain.  
A check that cannot run still refuses.

Before, `release` skipped the default-branch proof when it could not run.  
The error names the cause: no git repository, no HEAD commit, or no `origin` remote.  
A run that uses either flag lists what it went past under `allowed`, as a row in the human view and as `allowed: [{check, reason}]` in JSON.

`verctl release` now refuses a package with a skipped version, because publishing past it would skip that release for good.  
No flag allows it.

A version v is skipped when three things hold in the first-parent range from HEAD:

- some commit in the range has v;
- v is below HEAD's version;
- no newer commit in the range has a version strictly below v.

The range ends before a released version, `0.0.0`, a revision with no version, or a version that is not semver, and otherwise at the root.  

A version is released when the registry lists it for every checked upload of the package.  
A package with an unchecked upload, or with none, has it released once its tag exists, because `release` creates that tag after the upload.

For a workspace, only these members count at an earlier version:

- a member on the package's version;
- a member that crates.io lists at or below that version.

The other members joined later or move on their own.  
With no member left, the tag decides.

An abandoned version, one that a later downward change superseded, is not listed.  
`verctl status --github-output` lists each skipped version once, at its oldest commit in the range, oldest first.  
The workflow then releases it before the newer one.

The reusable `release.yml` takes two boolean inputs, `allow-workflow-change` and `allow-off-default-branch`, default `false`, and passes them to verctl through `env:`.

In the library, `publish::run_pending` takes an `Allow` argument, and the report it returns lists `allowed`.
