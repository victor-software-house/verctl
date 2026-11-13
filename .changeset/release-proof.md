---
verctl: patch!
---

`verctl check trust` reads the owner's token from `VERCTL_OWNER_TOKEN` in place of `VERCTL_REGISTRY_TOKEN`.
The publish token and the owner's token then never share a variable: set the new one where `check trust` runs.
No command verctl starts inherits it.

`verctl release` runs the registry checks before its first upload: once something is left to release, and before the draft, it runs each check the new `release_checks` key names, `[registry, trust]` when omitted.
`trust` runs only when `VERCTL_OWNER_TOKEN` is set, and the report lists it as skipped otherwise.

A failed check stops the run naming each failure, with nothing published, and `release_checks: []` turns the proof off.
`release --dry-run` prints the plan and runs the same checks, and fails when one does.

`--crates-io-api` (`VERCTL_CRATES_IO_API`) points `check` and `release` at another crates.io API, such as its staging instance.
`--crates-io-index` (`VERCTL_CRATES_IO_INDEX`) does the same for the sparse index `release` and `status` read.
