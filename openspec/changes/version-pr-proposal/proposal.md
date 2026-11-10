# Name the Version PR in the config file

## Why

The Version PR's title and commit message come from two environment variables,
`VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`. The reusable `release.yml` and
`actions/version-pr` each forward them from a `title` and a `commit-message`
input. A repository that wants another title therefore states it in its caller
workflow, away from `.ctl/ver.yaml`, where every other release setting lives.

`prepare.version_label` labels the Version PR, default `verctl:version`.
Since [#188][pr188] nothing reads the label: `check --versions` knows the
Version PR by its head repository, head branch, and base. The label is
an unused key that costs two GitHub calls, create the label and apply it,
on every run that opens or updates the pull request.

Both are breaking changes the operator placed in the 0.8.0 minor on
2026-11-09 (operator, 2026-11-09). This is BREAKING item 9's first slice and
task 3.1 and 3.2 of [release-state-machine](../release-state-machine/proposal.md).

Queue row: VER-090.

## What changes

1. **BREAKING:** a top-level `proposal` block in `.ctl/ver.yaml` holds
   `title` and `commit_message`. `title` defaults to
   `chore(release): version packages`, and `commit_message` defaults to the
   title, which is what verctl writes today with neither variable set.
2. **BREAKING:** verctl no longer reads `VERCTL_PR_TITLE` or
   `VERCTL_COMMIT_MESSAGE`. The reusable `release.yml` and
   `actions/version-pr` drop their `title` and `commit-message` inputs.
3. **BREAKING:** `prepare.version_label` is removed, and the Version PR gets
   no label. The `prepare` block refuses unknown keys, and a file that still
   sets `version_label` fails to load with a message that says the label is
   gone.
4. The reusable `release.yml` runs the caller's pinned verctl, so a caller
   that moves to the 0.8.0 workflow must pin verctl 0.8.0 too. Every job that
   runs verctl fails first, naming both versions, when the installed verctl is
   older than 0.8.0.

## Impact

1. Code: `src/config.rs`, `src/release.rs`, `src/github.rs`, `src/main.rs`.
2. Workflows: `.github/workflows/release.yml`, `actions/version-pr/action.yml`.
3. Docs: `src/instructions.md`, `skills/verctl/SKILL.md`, `README.md`,
   `AGENTS.md`, and rustdoc on the changed items.
4. Consumers set `proposal` in `.ctl/ver.yaml` in their 0.8.0 adoption pull
   requests, in place of the inputs.

[pr188]: https://github.com/victor-software-house/verctl/pull/188
