---
verctl: patch
---

`verctl check --versions` works in the depth-1 pull request checkout of `actions/checkout`. On Actions it first fetches `VERCTL_DEFAULT_BRANCH`, the event's default branch, and `GITHUB_BASE_REF` with their whole history when the checkout lacks them or is shallow, HEAD's parents included, so the merge-base can be computed. A full clone that has them fetches nothing.
