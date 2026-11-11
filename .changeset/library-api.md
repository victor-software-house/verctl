---
verctl: patch!
---

The library API changes, and the `verctl` command does not.
`config::PackageSpec::publish_argv` becomes `PackageSpec::publish`, which holds publisher options or the whole upload command.
`fragment::PackageBump` gains `breaking`, and `semver_check::Decision` gains `BreakingPatch`.
`pending::pending`, `publish::run`, and `publish::run_pending` take the path of the loaded config after the config.
