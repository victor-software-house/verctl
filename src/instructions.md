# verctl agent instructions

`verctl` consumes Changesets-format fragments and prepares a Version PR.
It is not `@changesets/cli` and not a forkctl verb.

## Invocation

```
mise run ver -- status
mise run ver -- instructions
```

Until a tagged GitHub Release exists, use a local or
`cargo install --git` build, then `verctl`.

## Fragments

`.changeset/<slug>.md`. YAML fence. Quoted or unquoted keys. Values are
`major` | `minor` | `patch` | `none`. Body is the changelog summary.

```md
---
forkctl: patch
"@scope/pkg": minor
---

What changed.
```

Write the fragment on the same PR that ships the behavior. Do not
save them up for a release dump.

Do not invent a third fragment format. Do not infer bumps from
`feat:` / `fix:` unless the repo has turned that mode on (not first slice).

## Changelog

Render through minijinja (Jinja2) files. Defaults live in this crate
(`templates/changelog.jinja`, `templates/dependency-changelog.jinja`).
Repos may override the paths. The adapter does not concatenate Markdown
in Rust beyond a typed context.

`internalAuthors` is adapter policy, not a template `if`. Match the
GitHub login GitHub actually resolves from the commit author. Those
logins omit the byline.

Context: `summary`, `pull_request`, `commit`, `continuations`,
`summary_has_terminal`, `dependencies`.

## Version PR

Happy path is GitHub Actions: `victor-software-house/verctl/actions/version-pr`,
not `changesets/action`, not a GitHub App. The workflow already has
`git`, `gh`, and `${{ github.token }}`. No PAT. No App install.

Do not hand-edit versions. `verctl check --versions` compares each
declared manifest to the merge-base of HEAD and the default branch.
It fails when they differ. Exempt only on the `version-packages`
branch locally, or when the GitHub event carries the Version PR
label (`verctl:version` by default, `[prepare].version_label`).
`prepare --pr` applies that label. Not `GITHUB_HEAD_REF`. CI
does not skip. A fragment-only commit does not change versions.

This repo splits mise envs: `dev` compiles this checkout, `release`
is the published tarball. Local default is `dev` (`.miserc.toml`).
Version PR sets `MISE_ENV=release`. Publish sets `dev,release`.
`verctl pin` rewrites collocated `github:…/verctl` entries (and
`?ref=v…` includes) to the versions on HEAD. Run it after the
GitHub Release tarball exists, then `mise -E release lock`. The
Version PR still installs the previous tarball.

`prepare` writes versions, per-package CHANGELOG.md (next to each
manifest), and consumes fragments (same as `changeset version`).
`[prepare].after` is one argv run after bumps; `[prepare].stage`
lists extra globs that command may write. Any other dirty path
fails. `--pr` also opens or force-updates `version-packages` (body is the
changelog).
Auth is `GITHUB_TOKEN` / `GH_TOKEN` only. Push uses that token over
HTTPS, not the machine git/ssh account. We do not call `git` or `gh`
as commands. Local `--pr` is recovery when that same token is already
in the environment.

`prepare --dry-run` and `prepare --preview` print the same plan and
write nothing. `prepare --pr --dry-run` also lists consumed fragments
and whether the Version PR would open or update. Preview does not
require GitHub auth.

`publish` ships the versions already on HEAD, then a GitHub Release.
It refuses unless each package version has a matching CHANGELOG
heading (the Version PR writes those) and, when `origin` exists,
HEAD is an ancestor of the default branch (`origin/HEAD`, then
`GITHUB_BASE_REF`, then `main`/`master`). It does not sniff the
commit subject and does not treat `GITHUB_REF_NAME` as the default
(that is the branch being pushed). `actions/publish` points
`origin/HEAD` at `github.event.repository.default_branch` because
checkout never creates that symref. Locally `git clone` already
has it; otherwise `git remote set-head origin --auto`. How is `[publishers.NAME]`: argv + placeholders.
Cargo and bun are stock recipes (first-class examples). Override or
add another stack without a new verb. GitHub Packages for bun uses a
nearby `bunfig.toml` as `--config`. Pretty output is ctl-core
tables; tests pass `--color never`.

Happy path after the Version PR merges is
`victor-software-house/verctl/actions/publish`. It runs when the
`version-packages` PR is merged. Native tarballs are a second job,
only when `[assets].targets` is non-empty. PR CI stays on one
runner. `verctl assets` prints the matrix; `--build` + `--upload`
is `actions/asset`. OIDC trusted publishing is later (VER-007).

## Stop conditions

Stop and ask when a fragment is not valid YAML, when a package name is
unknown to `[release]` config, or when a `major` fragment lands on a
0.x package.
