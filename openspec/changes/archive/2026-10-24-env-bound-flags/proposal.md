# Read every flag from the environment

## Why

The three actions rebuilt verctl's argv in bash: each turned `VERCTL_CONFIG`
back into `-c` and `VERCTL_DRY_RUN` back into `--dry-run` through an array,
and the `config` input restated verctl's default, `.ctl/ver.yaml`, because
`-c ""` would fail. The operator ruled on 2026-10-15 that every ctl flag has
an environment variable, with precedence flag, then variable, then config
file, then default, and that a caller sets `env:` and runs the bare verb.
ctl-core 0.6.21 ships the binding (`EnvPrefix`, `env_under_flags`).

Queue row: VER-067.

## What Changes

1. verctl takes ctl-core 0.6.21 and puts `env_prefix = "VERCTL"` on its root
   parser. Every long option at every depth reads `VERCTL_<LONG>`, and help
   names each variable.
2. The actions set `env:` and run `exec verctl prepare --pr`,
   `exec verctl publish`, and `exec verctl assets --upload`. Their `config`
   input defaults to empty, which verctl reads as unset.
3. Integration tests start the binary through one helper that removes the
   colour variables and every `VERCTL_*` variable.

## Capabilities

### New Capabilities

- `env-flags`: verctl's flags from the environment, and the actions that use
  them.

### Modified Capabilities

None.

## Impact

1. Code: `src/cli.rs`, `Cargo.toml`, `Cargo.lock`.
2. Actions: the `config` input default changes from `.ctl/ver.yaml` to empty;
   a caller that passes nothing gets the same file. The asset action passes
   its `target` input as `VERCTL_BUILD` instead of `VERCTL_TARGET`, an
   internal name.
3. Tests: `tests/common::verctl()` replaces 30 direct spawns.
