---
verctl: minor
verctl:group: changed
---

`verctl prepare` is now `verctl version`, with the same flags, variables, and behaviour.  
`verctl pin` is now `verctl render`, which also renders the served-file templates.  
`render` takes `--config` and `--dry-run` only: `pin` shared `--plan` and `--skip-assets` with `publish` and ignored both.  
Its human rows are `file` and `files` where `pin` printed `pin` and `pins`.  
The old verbs are hidden and exit 2 with an error that names the new one, so `mise run ver prepare --pr` becomes `mise run ver version --pr`.  
The reusable `release.yml` and `actions/version-pr` call the new verbs.

The JSON and the human view of `version` list follow-up commands under `after`, a key that was `next`.

In the library, `verctl::cli::Command::Prepare` is `Command::Version`, `PrepareArgs` is `VersionArgs`, and `Command::Pin` is `Command::Render`.  
The `prepare` and `pins` keys in `.ctl/ver.yaml` keep their names.
