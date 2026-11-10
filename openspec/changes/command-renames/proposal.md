# Rename prepare to version and pin to render

## Why

`prepare` is the step that writes versions, changelogs, and served files, and
its report lists the follow-up commands under `next`. The
[release-state-machine](../release-state-machine/proposal.md) plan names that
step `version` and the field `after`. The operator approved both names on
2026-10-12, with `render` for `pin`. On 2026-11-09 the operator placed the
three renames in the 0.8.0 minor, with no change to what each command does
(operator, 2026-11-09).

Every caller names these verbs: the reusable `release.yml`,
`actions/version-pr`, and each consumer's own scripts. A rename breaks each
of them once, so it belongs in the one minor that carries every break.

Queue row: VER-091.

## What changes

1. **BREAKING:** `verctl prepare` becomes `verctl version`, with the same
   flags (`--pr`, `--dry-run`, `--dir`, `--config`) and the same variables.
2. **BREAKING:** the report field `next` becomes `after`, in JSON and in the
   human view.
3. **BREAKING:** `verctl pin` becomes `verctl render`, with the same flags
   and behaviour: it rewrites the `pins` entries to the versions on HEAD.
   Rendering templates through `ctl_core::served` stays out of this change.
4. **BREAKING:** `verctl prepare` and `verctl pin` stay as hidden commands
   that exit 2 and name the new verb. They are not aliases and run nothing.
5. **BREAKING (library):** `verctl::cli::Command::Prepare` becomes
   `Command::Version`, `PrepareArgs` becomes `VersionArgs`, and
   `Command::Pin` becomes `Command::Render`.
6. The `prepare` and `pins` keys in `.ctl/ver.yaml` keep their names; the
   config reshape is a later change.

## Impact

1. Code: `src/cli.rs`, `src/main.rs`, `src/presentation.rs`, and the tests
   that run either verb.
2. Workflows and served files: `.github/workflows/release.yml`,
   `actions/version-pr/action.yml`, `.ctl/ver.yaml`, and
   `.ctl/operator/instructions.md.jinja`.
3. Docs: `src/instructions.md`, `skills/verctl/SKILL.md`, `README.md`,
   `AGENTS.md`, and five specs that name `prepare` in their scenarios.
4. Consumers rename their calls in their 0.8.0 adoption pull requests.
