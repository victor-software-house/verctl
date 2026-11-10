---
name: verctl
description: >-
  Operate verctl version PRs from Changesets-format fragments. Use when the
  user mentions verctl, mise run ver, Version Packages, prepare-release,
  .changeset fragments, or stack-agnostic changelog templates. Not
  @changesets/cli and not a forkctl verb.
license: MIT
version: 0.7.4
---

# verctl

`verctl instructions` is the installed contract. Actions run `verctl` from
PATH.

## Invocation

```sh
mise run ver status
mise run ver check
mise run ver version --pr
```

Never `mise run ver --`. The `--` in `#USAGE mount` is mise's
completion bootstrap.

## Commands

| Command | Aliases | Purpose |
|:--|:--|:--|
| `instructions` | — | Print the installed-version operator contract |
| `status` | — | List pending fragments, the asset plan, and the incomplete version commits. `--github-output` writes the release workflow's plan |
| `check` | — | Validate fragments, or `--versions` against the default branch |
| `version` | — | Apply fragment bumps. `--pr` opens the Version PR. `--dry-run` previews |
| `release` | — | Release the versions on HEAD (cargo / bun + GitHub Release). `release asset <target>` builds and uploads one asset |
| `render` | — | Rewrite pins and render served templates at the versions on HEAD |
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

`version --pr` opens the Version PR against `GITHUB_REF_NAME`, else
`VERCTL_DEFAULT_BRANCH`, else the branch `origin/HEAD` names, and fails before
writing anything when none is set; it never guesses `main`. `proposal.title` and
`proposal.commit_message` in `.ctl/ver.yaml` set the title and commit
message; unset or empty, the title is `chore(release): version packages` and
the message is the title. The Version PR gets no label.
The PR comes from `verctl/release/<base>`, derived and never configured;
`check --versions` exempts that branch locally. verctl never merges the
Version PR or enables auto-merge on it: a release starts only when a person
merges it.

A new package starts at `0.0.0`. Its first fragment is a `patch`, so its
first release is `0.0.1`. Moving to `0.1.0` or `1.0.0` is a separate operator
decision, never the first fragment's bump. `version` and `check` refuse a
`minor` or `major` fragment on a `0.0.0` package; `first_minor: true` on the
package in `.ctl/ver.yaml` opens it at `0.1.0`, and only the operator sets it.
`release` and `status` leave a package at `0.0.0` out, so a repo can
adopt the release workflow before its first release.

A repo declares everything in `.ctl/ver.yaml`, the one file verctl reads;
`.ctl/` is shared with the other ctl CLIs and templates live in
`.ctl/templates/`. There is no `verctl.toml`.

Prefer a template over a pin: a served file is generated, and a `pins` entry
admits a file is hand-authored. A template declares its own target with
top-level `{%- set … -%}` exports. Commit it: one the repo neither tracks nor
ignores fails the run, because rendering nothing would serve the stale file
instead. An ignored one is disowned and left alone; one outside the source
tree was never verctl's to render.

A served mise task execs the tool from PATH, never
`"$(mise where <tool>)/<tool>"`. `mise where` resolves from the surrounding
config, so it ignores the task's own `#MISE tools` pin, while mise already puts
that pin first on PATH. Removing it is only safe together with rendering the
pin, or the task freezes on whatever version the file happens to name.

`render` rewrites those pins and renders every template at the versions on
HEAD. `check` fails when a served file or a pin differs from that, with or
without fragments, and `render` is the fix for both. A file that a template and
a `pins` entry both claim fails `version`, `check`, and `render` before any
write.

Machines are `runners` entries with `labels`, declared once and named by
the jobs that run on them: a `ci` job takes `runners`, one check each; an
`assets` target takes a single `runner`, because one tarball is one
machine. Only `labels` reach `runs-on`; a name resolves against
`runners` or fails.

verctl decides how many jobs there are; the workflow file decides where a
fixed job runs. `plan`, `crate`, and `version` are always one job
each, so their `runs-on` is a literal in the consumer's workflow, not a
config key. Do not add one, and do not add a `release` runner section.

Stop when a 0.x package gets `major`, when verctl refuses a `0.0.0`
package's bump, or when a fragment names a package that is not in `packages`.

`publisher: none` on a package releases it on GitHub only (tag, Release,
assets), with no registry upload; use it for private binaries.
verctl has no stock Python publisher, so a `pyproject.toml` package names
its `publisher`, or `none`; one whose `project.dynamic` lists `version`
fails, because its build backend sets the version. A `go.mod` package keeps
its version as `// vX.Y.Z` on the `module` line and, with no publisher,
releases on GitHub only; a v2 bump needs the `/v2` module path first. A
`VERSION` file holds only the version; a missing one in an existing
directory reads as `0.0.0` until `version` writes it, and with no publisher
it releases on GitHub only.
`marker: true` versions any one file, such as a reusable workflow, on its
one `# version: X.Y.Z` line, and also releases on GitHub only.

An `assets` tarball holds the one `binary`. Set `assets.dir` and
`assets.files` instead when it holds more (a second executable, a service
unit, a licence); `release asset` packs `tar czf <asset> -C <dir> <files>` with
`{triple}` and the other placeholders replaced.

`vendored.<name>` declares release assets built from someone else's source
at its own `version`, outside fragments and `release`, for a mise `github:`
tool. `status --vendored <name>` (`VERCTL_VENDORED`) plans it, and
`release asset <id> --vendored <name>` runs its `build`, packs `files` from
`dir` into `archive` (default `{name}-{version}-{id}.tar.gz`), and unless
`--dry-run` creates the `tag` Release (default `{name}-{version}`, never
marked latest) when it is missing and attaches the archive and its
`.sha256`. A config that vendors may declare no
packages.

`release` acts on version commits: the commit that set each package's
version. `status` lists every one whose release is incomplete (registry,
tag, Release, assets unless `--skip-assets`), oldest first, under
`unreleased`; it needs `GITHUB_TOKEN` or `GH_TOKEN`, and without one it
reports `unreleased` as not checked. `status --github-output <file>` fetches
the history and writes the `commits`, `pending`, `assets`, `has_assets`,
`tag`, and `matrix` outputs. `release --plan` fails and names it. `release`
releases only the packages whose version commit is HEAD, and reports
`nothing to release` when every release is complete. A release tag
on a commit HEAD holds only through a merge passes when the version there
matches HEAD's.

Before any upload, tag, or Release, `release` proves the release commit:
the default branch contains it, and its `.github/workflows/` is the same tree
as at the default branch's tip. `--allow-off-default-branch` and
`--allow-workflow-change` (`VERCTL_ALLOW_OFF_DEFAULT_BRANCH`,
`VERCTL_ALLOW_WORKFLOW_CHANGE`; `release.yml` inputs `allow-off-default-branch`
and `allow-workflow-change`) go past one, and the report lists it under
`allowed`. A check that cannot run refuses anyway. A package with an earlier
unreleased version commit since its last tag is refused with no flag:
`status` lists that commit first, so release it before the newer one.

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
`verctl release` fetches the default branch (that one ref, with history)
only when `origin/<default>` is missing. It rewrites `origin/HEAD` only
from `VERCTL_DEFAULT_BRANCH`. GitHub git HTTPS uses `x-access-token`,
never Bearer, and never puts the token in the URL.
