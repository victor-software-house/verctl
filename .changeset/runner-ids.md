---
verctl: minor
---

Runners are configuration. `[[ci.jobs]]` and `[[assets.targets]]` are both
lists of `id` + `runs_on`, where `runs_on` is the literal GitHub label list
rather than a single runner name. Omitting `[ci]` runs one `verify` job on
`ubuntu-latest`, so a repo that says nothing behaves as before. New `verctl
ci` writes the matrix a `plan` job hands to `runs-on:`.

Breaking for anyone who wrote an asset target: a target is a table, so
`targets = ["linux-x64"]` becomes `targets = [{ id = "linux-x64" }]`, and
`runner = "macos-15"` becomes `runs_on = ["macos-15"]`. An unknown target id
with a partial record now fails instead of building against an empty target
triple.
