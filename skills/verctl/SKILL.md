---
name: verctl
description: >-
  Operate verctl version PRs from Changesets-format fragments. Use when the
  user mentions verctl, mise run ver, Version Packages, prepare-release,
  .changeset fragments, or stack-agnostic changelog templates. Not
  @changesets/cli and not a forkctl verb.
license: MIT
version: 0.5.4
---

# verctl

`verctl instructions` is the installed contract. Actions run `verctl` from
PATH.

## Invocation

```sh
mise run ver status
mise run ver check
mise run ver prepare --pr
```

Never `mise run ver --`. The `--` in `#USAGE mount` is mise's
completion bootstrap.

## Commands

| Command | Aliases | Purpose |
|:--|:--|:--|
| `instructions` | — | Print the installed-version operator contract |
| `status` | — | List pending .changeset fragments |
| `check` | — | Validate fragments, or `--versions` against the default branch |
| `prepare` | — | Apply fragment bumps. `--pr` opens the Version PR. `--dry-run` previews |
| `publish` | — | Publish the versions on HEAD (cargo / bun + GitHub Release) |
| `pin` | — | Rewrite collocated tool pins to the versions on HEAD |
| `assets` | — | Plan or build native GitHub Release tarballs declared under `assets` |
| `ci` | — | Plan the validation jobs declared in `[ci]` |
| `docs` | — | Wait for docs.rs to build each crates.io crate released at HEAD |
| `semver` | — | Check the API of each crates.io crate whose next release is a patch |

Every long option also reads `VERCTL_<LONG>` (`VERCTL_CONFIG`,
`VERCTL_DRY_RUN`, `VERCTL_FORMAT`): a flag wins over its variable, an empty
variable is unset, and `--help` names each one. A workflow or action sets
them in `env:` and runs the bare verb; never rebuild argv in a shell.

Every command returns one typed report. Use `--format json` for the same data
without ANSI, whatever `CLICOLOR_FORCE` or `FORCE_COLOR` say; JSON errors use
stdout, while human errors use stderr. `--quiet` suppresses only successful
human output.

Fragments are `.changeset/*.md` with YAML fences (quoted or unquoted
keys). An optional `verctl:group: added | changed | fixed` key files the
entry under that heading; once one fragment in a section names a group, an
ungrouped one goes under Changed. Changelog Markdown is rendered with minijinja. Each entry links
the pull request or commit that added its fragment. List maintainers and
Apps in `changelog.internal_authors`; only an unlisted person whose pull
request comes from a fork gets a byline. Consumers use
verctl's reusable workflow (`.github/workflows/release.yml`, called from a
thin caller), not changesets/action; its steps run `mise run ver <verb>`
with settings in `env:`.
Do not assemble changelog strings in ad-hoc Rust. Do not add a Node adapter.

`prepare --pr` opens the Version PR against `GITHUB_REF_NAME`, else
`VERCTL_DEFAULT_BRANCH`, else the branch `origin/HEAD` names, and fails before
writing anything when none is set; it never guesses `main`. The title and
commit message default to `chore(release): version packages`;
`VERCTL_PR_TITLE` and `VERCTL_COMMIT_MESSAGE`, or the action's `title` and
`commit-message` inputs, override them, and an empty value is unset.
The PR comes from `verctl/release/<base>`, derived and never configured;
`check --versions` exempts that branch locally. `--auto-merge`
(`VERCTL_AUTO_MERGE`) enables a squash auto-merge, and refuses before any
write when no ruleset on the base requires a status check, or when the token
equals `VERCTL_JOB_TOKEN`, the job's own token, whose merge starts no
workflow.

A new package starts at `0.0.0`. Its first fragment is a `patch`, so its
first release is `0.0.1`. Moving to `0.1.0` or `1.0.0` is a separate operator
decision, never the first fragment's bump. `prepare` and `check` refuse a
`minor` or `major` fragment on a `0.0.0` package; `first_minor: true` on the
package in `.ctl/ver.yaml` opens it at `0.1.0`, and only the operator sets it.
`publish` and `publish --plan` leave a package at `0.0.0` out, so a repo can
adopt the release workflow before its first release.

A repo declares everything in `.ctl/ver.yaml`, the one file verctl reads;
`.ctl/` is shared with the other ctl CLIs and templates live in
`.ctl/templates/`. There is no `verctl.toml`.

Prefer a template over a pin: a served file is generated, and a `pins` entry
admits a file is hand-authored. A template declares its own target with
top-level `{%- set … -%}` exports. Commit it — one the repo neither tracks nor
ignores fails the run, because rendering nothing would serve the stale file
instead. An ignored one is disowned and left alone; one outside the source
tree was never verctl's to render.

A served mise task execs the tool from PATH, never
`"$(mise where <tool>)/<tool>"`. `mise where` resolves from the surrounding
config, so it ignores the task's own `#MISE tools` pin, while mise already puts
that pin first on PATH. Removing it is only safe together with rendering the
pin, or the task freezes on whatever version the file happens to name.

`pin` rewrites those collocated pins onto HEAD.

Machines are `runners` entries with `labels`, declared once and named by
the jobs that run on them: a `ci` job takes `runners`, one check each; an
`assets` target takes a single `runner`, because one tarball is one
machine. Only `labels` reach `runs-on`; a name resolves against
`runners` or fails.

verctl decides how many jobs there are; the workflow file decides where a
fixed job runs. `plan`, `crate`, and `prepare` are always one job
each, so their `runs-on` is a literal in the consumer's workflow — not a
config key. Do not add one, and do not add a `release` runner section.

Stop when a 0.x package gets `major`, when verctl refuses a `0.0.0`
package's bump, or when a fragment names a package that is not in `packages`.

`publisher: none` on a package releases it on GitHub only (tag, Release,
assets), with no registry upload; use it for private binaries.

`publish` acts on version commits: the commit that set each package's
version. `publish --plan` lists every one whose release is incomplete
(registry, tag, Release, assets unless `--skip-assets`), oldest first, and
`publish` releases only the packages whose version commit is HEAD. Both
report `nothing to release` when every release is complete.

`docs` waits for docs.rs to build each crates.io library crate released at
HEAD and fails on a build that failed or never arrived (`--timeout`, minutes,
default 60).

`semver` runs `cargo semver-checks --release-type minor` on each crates.io
crate whose next release is a patch, so only an API break fails, and skips a
crate with a pending `minor` fragment or a minor the Version PR already moved.
A crate at 1.0 or later fails: the rule covers 0.x.

`publish` asks the registry before and after each publisher: the crates.io
sparse index for a stock cargo package bound for crates.io, the recipe's
`query` otherwise. A listed version is `already`; a publisher that exits zero
while the registry never lists the version fails. Set `query` on a package or
a `publishers` entry for another registry, and `query: []` to turn the check
off.

`tags.template` (default `v{version}`) names release tags. `{name}` makes
one tag and one Release per package; without it, differing versions refuse.
Each Release body is that package's CHANGELOG section. The tag is created
at HEAD and re-read; a tag that names another commit fails.
`verctl publish` fetches the default branch (that one ref, with history)
only when `origin/<default>` is missing. It rewrites `origin/HEAD` only
from `VERCTL_DEFAULT_BRANCH`. GitHub git HTTPS uses `x-access-token`,
never Bearer, and never puts the token in the URL.
