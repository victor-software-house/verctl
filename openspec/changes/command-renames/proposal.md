# Rename prepare, pin, publish, and assets

## Why

`prepare` is the step that writes versions, changelogs, and served files, and
its report lists the follow-up commands under `next`. The
[release-state-machine](../release-state-machine/proposal.md) plan names that
step `version` and the field `after`. The operator approved both names on
2026-10-12, with `render` for `pin`. On 2026-11-09 the operator placed the
three renames in the 0.8.0 minor, with no change to what each command does
(operator, 2026-11-09). On 2026-11-10 the operator added `publish` to
`release` and `assets` to `release asset`, the surface of
[derived-release-state](../derived-release-state/design.md) §4, and moved
the release plan into `status` now, so 0.9.0 needs no second CLI break
(operator, 2026-11-10).

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
   [served-file-check](../served-file-check/proposal.md) adds template
   rendering to it. Rendering through `ctl_core::served` stays out.
4. **BREAKING:** `verctl prepare` and `verctl pin` stay as hidden commands
   that exit 2 and name the new verb. They are not aliases and run nothing.
5. **BREAKING (library):** `verctl::cli::Command::Prepare` becomes
   `Command::Version`, `PrepareArgs` becomes `VersionArgs`, and
   `Command::Pin` becomes `Command::Render`.
6. **BREAKING:** `verctl publish` becomes `verctl release`, with the same
   flags and behaviour except `--plan`.
7. **BREAKING:** `verctl assets` becomes `verctl release asset <target>`
   (`VERCTL_TARGET`), which builds and uploads one tarball; `--dry-run`
   builds it and uploads nothing.
8. **BREAKING:** `status` takes over the plan. Its report lists the asset
   plan and the version commits with an incomplete release under
   `unreleased`. `status --github-output` writes `has_assets`, `tag`,
   `matrix`, `commits`, `pending`, and `assets`, under the names
   `assets --github-output` and `publish --plan` used. `status --vendored`
   plans a vendored set.
9. **BREAKING:** hidden `publish` and `assets` commands and a hidden
   `release --plan` flag fail and name the replacement, as in item 4.
10. The `prepare`, `pins`, `assets`, and `publish` keys in `.ctl/ver.yaml`
    keep their names; the config reshape is a later change. The workflow
    job ids and the `actions/publish` directory keep theirs too, so check
    names do not change; moving the actions is VER-043.

## Impact

1. Code: `src/cli.rs`, `src/main.rs`, `src/presentation.rs`, and the tests
   that run any renamed verb.
2. Workflows and served files: `.github/workflows/release.yml`,
   `actions/version-pr/action.yml`, `actions/publish/action.yml`,
   `actions/asset/action.yml`, `.ctl/ver.yaml`, and
   `.ctl/operator/instructions.md.jinja`.
3. Docs: `src/instructions.md`, `skills/verctl/SKILL.md`, `README.md`,
   `AGENTS.md`, `examples/ver.yaml`, the pending fragments, and the specs
   that name a renamed verb.
4. Consumers rename their calls in their 0.8.0 adoption pull requests.
