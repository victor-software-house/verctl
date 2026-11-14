---
verctl: patch!
---

`verctl check registry` fails a pull request or a merge group while the default branch holds a version whose release is incomplete, naming each `<name>@<version>`, where it passed before.
It needs `GITHUB_TOKEN` on those events, and on any other event the pending release is a notice.
Before it decides, it waits for a run of the `release_workflow` file at the default branch's tip, up to the new `--release-wait <SECONDS>` (`VERCTL_RELEASE_WAIT`, 1800 when omitted), and fails naming the run and the limit when the run outlasts it.
`--crates-io-index` (`VERCTL_CRATES_IO_INDEX`) now sets the sparse index of that search for `check` as it does for `release` and `status`.

A pull request labelled `verctl: unfreeze` passes the freeze, and no other proof of `registry`.
A merge group reads the label of the pull request its head ref names.
`release` and `release --dry-run` run `registry` without the freeze.

`verctl status` names the enforcement tier of the default branch's rules, `merge queue`, `rulesets without a queue` or `neither`, and shows it as unknown, with the reason, without a token or when GitHub refuses the read.
Its JSON report gains `tier`.
