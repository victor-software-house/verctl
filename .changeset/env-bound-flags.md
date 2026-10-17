---
verctl: patch
---

Every long option now reads `VERCTL_<LONG>` from the environment, through ctl-core 0.6.21: `VERCTL_CONFIG`, `VERCTL_DRY_RUN`, `VERCTL_FORMAT`, and the rest. A flag on the command line wins over its variable, an empty variable is unset, and `--help` names each variable. The `version-pr`, `publish`, and `asset` actions set these variables and run the bare verb, and their `config` input defaults to empty, which means `.ctl/ver.yaml`.
