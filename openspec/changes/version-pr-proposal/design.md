# Design

## 1. One block, two keys

`proposal.title` and `proposal.commit_message` sit in a top-level `proposal`
block, the name the [release-state-machine](../release-state-machine/design.md)
plan uses for Version PR settings. The block refuses unknown keys.

Alternative considered: `prepare.title` and `prepare.commit_message`. It lost
because `prepare` is the version step, renamed to `version` in
[command-renames](../command-renames/proposal.md). The title belongs to the
pull request, which a later `propose` verb takes over.

## 2. Defaults equal today's output

`src/release.rs` writes `chore(release): version packages` as the title when
`VERCTL_PR_TITLE` is unset or empty. It uses the title as the commit message
when `VERCTL_COMMIT_MESSAGE` is unset or empty. The keys keep both rules:
an unset or empty `title` is that string, and an unset or empty
`commit_message` is the resolved title. A repository that set neither
variable sees the same pull request and commit after the upgrade.

Public override: the two keys. Disable path: none is needed, since both
always resolve to a value.

## 3. No fallback to the variables

verctl stops reading both variables in the same release. Reading them as a
fallback would keep two places for one setting, the problem this change
removes. A caller that still sets them sees no error from verctl. The
reusable workflow and the action refuse the removed inputs, because GitHub
fails a call that passes an undeclared input.

Alternative considered: fail when either variable is set. It lost because
an empty variable in a job's environment is common, and verctl already
treats empty as unset.

## 4. The label goes without replacement

`prepare.version_label` and `github::ensure_version_label` are removed. A
`.ctl/ver.yaml` that sets `version_label` fails to load. The message names
the key, shows that the Version PR is no longer labeled, and names the fix:
delete the line.
A load error is the only way to show this, because the file is the only place
the key was ever set.

`Prepare` gains `deny_unknown_fields`, so any other unknown key under
`prepare` also fails. That is part of the `.ctl/ver.yaml` BREAKING item and costs nothing
extra in the same minor.

## 5. Version skew between the workflow and verctl

`release.yml` installs the caller's tools with `mise install --locked`, and
each step runs `mise run ver <verb>`, the verctl the caller's served task
pins. The workflow ref and the verctl version are therefore chosen apart.
A caller on the 0.8.0 workflow with a 0.7 verctl would run `prepare` with no
title variables.
It would fail late, after the tree is written.

The `plan` job's first verctl step reads `verctl --version` and fails when
it is below 0.8.0. The message names the installed version and the minimum,
and says to move the verctl pin and the workflow ref together.

The minimum is a literal in the workflow, raised only when the workflow needs a
newer verctl. Every other job needs `plan`, so the check runs once
(operator, 2026-11-10).

`publish`, `assets`, and `docs` check out each version commit and run the
verctl it pins. A verctl older than 0.8.0 lacks the `release` verb, so a
pending commit that pins one fails there before it can publish.

Alternative considered: install the verctl that matches the workflow ref.
It lost because the caller's `mise.lock` is the record of what ran.
verctl's own release already passes `verctl-artifact` to put a fresh build
first on PATH.
