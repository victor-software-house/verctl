---
verctl: patch!
---

`verctl release` and `verctl status` take HEAD as the release reference.
A run releases, from HEAD, every version HEAD's manifests hold whose release is incomplete, whichever commit set it.
A version HEAD no longer holds stays unreleased, `status --github-output` writes HEAD as the only commit, and a release no longer refuses a skipped version.
The `waiting` list leaves the `release` report, and `declared` in the `--github-output` file is the comma-separated asset names HEAD's Release holds.
