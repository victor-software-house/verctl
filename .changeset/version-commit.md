---
verctl: patch
---

`publish` now acts on version commits, the commit that set each package's version. `publish --plan` lists every version commit whose release is incomplete (registry, tag, Release, and declared assets unless `--skip-assets`), oldest first, and writes `commits`, `pending`, and `assets` to `$GITHUB_OUTPUT`. `publish` releases only the packages whose version commit is HEAD and fails before any side effect on any other commit, so a later commit can no longer tag a version that was never published. When every release is complete, both report `nothing to release` and exit zero, with `"outcome": "nothing-to-release"` in JSON.
