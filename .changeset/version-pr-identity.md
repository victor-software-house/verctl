---
verctl: minor
---

`check --versions` knows a Version PR by its head repository, its head branch `verctl/release/<base>`, and its current base, read through the API, and no longer reads the `verctl:version` label. The Version PR's first CI run passes without a `labeled` trigger; the CI job needs `GITHUB_TOKEN` with `pull-requests: read`. A version the default branch carries at its merge-base with HEAD is not a hand edit, so merging the default branch into another branch passes. `Skip::from_env` takes no config and returns a `Result`, `report_with` and `require_with` take the default-branch candidates, `VersionRow` gains `released`, and the payload label readers are removed.
