---
verctl: patch
---

`verctl publish` now works in a shallow clone. With `VERCTL_DEFAULT_BRANCH` set, it fetches the whole history of that branch before looking for the commit that set each version, so a depth-1 checkout no longer fails with "the clone is shallow". Without the variable, the error now names it. When GitHub refuses to create a Release with 403, the error now says why: the release commit's `.github/workflows` differs from the default branch, so the Releases API also needs `workflows: write`, which `GITHUB_TOKEN` cannot hold. The operator instructions now say to adopt the release workflow only when no release is pending.
