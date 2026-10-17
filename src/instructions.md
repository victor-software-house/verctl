# verctl agent instructions

`verctl` consumes Changesets-format fragments and prepares a Version PR.
It is not `@changesets/cli` and not a forkctl verb.

## Invocation

```sh
mise run ver status
mise run ver instructions
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

Every long option also reads `VERCTL_<LONG>`: `VERCTL_CONFIG`,
`VERCTL_DRY_RUN`, `VERCTL_FORMAT`, and so on, with `-` turned into `_`. A
flag on the command line wins over its variable, and an empty variable is
unset. `--help` names each one. An action or a CI step sets them in `env:`
and runs the bare verb; it never rebuilds argv in a shell.

Install a released tarball through mise
(`github:victor-software-house/verctl`), or `cargo install verctl`.

## Fragments

`.changeset/<slug>.md`. YAML fence. Quoted or unquoted keys. Values are
`major` | `minor` | `patch` | `none`. Body is the changelog summary.

```md
---
forkctl: patch
"@scope/pkg": patch
---

What changed.
```

An optional `verctl:group` key files the entry under a changelog
heading: `added`, `changed`, or `fixed`. The colon keeps it from naming
a package. A section where no fragment names a group stays a flat list.
Once one does, the section gets `### Added`, `### Changed`, and
`### Fixed` headings in that order, and a fragment without the key goes
under `### Changed`.

A new package starts at `0.0.0`. Its first fragment is a `patch`, so
its first release is `0.0.1`: `0.0.x` is the initial line. Moving to
`0.1.0`, where a package claims a usable API, or to `1.0.0` is a
separate operator decision, never the first fragment's bump.

`prepare` and `check` refuse a `minor` or `major` fragment on a package
at `0.0.0`, including its pre-releases and builds, naming the package.
To open at `0.1.0` on purpose,
the operator sets `first_minor: true` on that package in `.ctl/ver.yaml`;
the key does nothing once the package has released. `check` plans the
pending fragments against `.ctl/ver.yaml` the way `prepare` does, so a
repo that runs it in its verify task fails the pull request that adds
such a fragment.

Write the fragment on the same PR that ships the behavior. Do not
save them up for a release dump.

Do not invent a third fragment format. Do not infer bumps from
`feat:` / `fix:` unless the repo has turned that mode on (not first slice).

## Changelog

Render through minijinja (Jinja2) files. Defaults live in this crate
(`templates/changelog.jinja`, `templates/dependency-changelog.jinja`).
Repos may override the paths. The adapter does not concatenate Markdown
in Rust beyond a typed context.

Each entry links where its fragment came from: the pull request a
trailing `(#N)` names in the subject of the first-parent commit that
added the fragment, else that commit. Links use the `origin` remote; a
checkout without a GitHub `origin` writes bare entries. Release notes
credit no maintainer: list maintainers and the Apps that open pull
requests in `changelog.internal_authors` in `.ctl/ver.yaml`. With a
token, an entry credits its pull request author only when the login is
not listed, the author is a person (`type` `User`), and the pull request
comes from a fork; without a token no one is credited. GitHub's
`author_association` cannot decide it, because the App token behind a
Version PR reads a private organization member as `CONTRIBUTOR`.

Context: `summary`, `pull_request`, `commit`, `continuations`,
`summary_body` (the summary without one trailing period, for an entry
that ends with a reference), `summary_has_terminal`, `dependencies`.

## Version PR

Happy path is GitHub Actions: `victor-software-house/verctl/actions/version-pr`,
not `changesets/action`. The workflow already has
`git`, `gh`, and `${{ github.token }}`. No PAT. Only `auto-merge` needs an App token.

Do not hand-edit versions. `verctl check --versions` compares each
declared manifest to the merge-base of HEAD and the default branch.
It fails when they differ. Exempt only on the `version-packages`
branch locally, or when the GitHub event carries the Version PR
label (`verctl:version` by default, `prepare.version_label`).
`prepare --pr` applies that label. Not `GITHUB_HEAD_REF`. CI
does not skip. A fragment-only commit does not change versions.

`auto-merge: true` on `actions/version-pr` enables GitHub auto-merge on the
Version PR, as a squash titled `<title> (#<number>)` with the PR description
as body. GitHub merges once the base branch's required checks pass. The action
fails when no ruleset on the base branch requires a status check, because GitHub would
merge at once, and when `token` is `GITHUB_TOKEN`, because a merge made with
it starts no publish workflow. Pass a GitHub App token. Cancel one Version
PR's auto-merge with `gh pr merge --disable-auto <number>`.

This repo compiles itself: `mise.toml` holds the toolchain, and
`mise run ver` runs this checkout. `release` is the published
tarball. Local default is `dev` (`.miserc.toml`), which adds the hook
tools; CI runs `ci`. This repo's own lanes build the verctl they run:
a lane that installed its own tarball could only ever run the
previous release, which is exactly when new behaviour is needed. Consumers run `release`, because the
verctl they install is independent of the version they release.
`verctl pin` rewrites collocated `github:…/verctl` entries (and
`?ref=v…` includes) to the versions on HEAD; `prepare` calls it, so
the Version PR commit carries them. `mise.release.toml` is not a
`pins` entry — it names a tarball that must already exist. Bump
it by hand when a lane wants a newer verb, then `mise -E release
lock`.

`prepare` writes versions, per-package CHANGELOG.md (next to each
manifest), and consumes fragments (same as `changeset version`).
`prepare.after` is one argv run after bumps; `prepare.stage`
lists extra globs that command may write. Any other dirty path
fails. `--pr` also opens or force-updates `version-packages` (body is the
changelog). The PR targets `GITHUB_REF_NAME`, else `VERCTL_DEFAULT_BRANCH`,
else the branch `origin/HEAD` names; with none of them, `--pr` fails
rather than guess `main`. Title and commit message default to
`chore(release): version packages`; `VERCTL_PR_TITLE` and
`VERCTL_COMMIT_MESSAGE` override them, and an empty value is unset.
Auth is `GITHUB_TOKEN` / `GH_TOKEN` only. Push uses that token over
HTTPS, not the machine git/ssh account: `--pr` runs `git push`, so it
needs `git` on PATH, with a credential helper that reads the token from
the push's environment and replaces any helper you configured. The
push skips repository hooks. We do not call `gh`. Local `--pr` is
recovery when that same token is already in the environment.

verctl reads the repository that covers the directory it runs in, as
it is on disk. Inside a git hook it ignores `GIT_INDEX_FILE`,
`GIT_WORK_TREE`, and `GIT_DIR`, and it never writes the index.

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
(that is the branch being pushed). `verctl publish` points `origin/HEAD`
at `origin/<default>` only when `VERCTL_DEFAULT_BRANCH` is set. It
fetches that one ref (with history, not at depth 1) only when it is
missing. GitHub git HTTPS authenticates as `x-access-token`; the token
is never in the URL and never sent as Bearer. Locally `git clone`
already has the ref; otherwise `git remote set-head origin --auto`. How is a `publishers` entry: argv + placeholders.
Cargo and bun are stock recipes (first-class examples). Override or
add another stack without a new verb. `publisher: none` releases a
package on GitHub only: `publish` creates its tag and Release, and
`assets` its tarballs, with no registry upload. Use it for a private
binary, such as a Cargo package with `publish = false`. A `registry`
or `publish` on the same package fails the load, and so does a
`publishers.none` entry. Every other publisher must render a command.
Before a publisher runs, `publish` asks the registry. A stock cargo
package whose publish target is crates.io (its `registry`, its manifest's
`publish`, and Cargo's `registry.default` all agree) is read from the
crates.io sparse index; any other package runs its recipe's `query`
(stock bun: `bun pm view {name}@{version} version`) in its directory. A
listed version is `already`, and its publisher does not run. After a
publish, `publish` asks again for about two minutes and fails unless the
registry lists the version. A package with no check (`query: []`, or a
cargo package with an overridden command or another target and no `query`)
succeeds on a zero exit or on stderr saying the version exists. GitHub Packages for bun uses a
nearby `bunfig.toml` as `--config`. One typed report feeds ctl-core pretty,
colorless, and JSON output; tests pass `--color never`. JSON is never
coloured, whatever `CLICOLOR_FORCE` or `FORCE_COLOR` say, and a parse
error under `--format json` is a JSON error even when the bad argument
comes first.

Tag names come from `tags.template` (default `v{version}`). The
placeholders are `{version}` and `{name}` — the same vocabulary
`patterns.match` uses. A template with no `{name}` names one tag and
refuses when the release has more than one distinct version. A
template with `{name}` names one tag and one Release per package; each
Release body is that package's own CHANGELOG section (path from
`packages[].changelog`, default `CHANGELOG.md` beside the manifest).
A template that would render the same tag for two packages fails at
plan time. Each Release is created at HEAD (`target_commitish` is that
SHA). After create, and when a Release already exists, the tag is
re-read and the run fails if it names another commit. A retry at the
same commit is a no-op.

Happy path after the Version PR merges is
`victor-software-house/verctl/actions/publish`. It runs when the
`version-packages` PR is merged. Native tarballs are a second job,
only when an `assets` target is declared. `verctl assets` prints the
matrix; `--build` + `--upload` is `actions/asset`. Publish pushes
nothing but the tag. `pins` are rewritten by `prepare`, on the
Version PR commit the tag will name, so the released tree carries its
own pins. A pin that must name an already-published tarball — a repo's
own bootstrap tool — is not a `pins` entry. OIDC trusted publishing
is later (VER-007).

Prefer a template over a pin. A served file under `.ctl/templates/`
declares its own target in top-level `{%- set … -%}` exports and is
rendered onto the Version PR commit, so it cannot drift; a `pins`
entry admits a file is hand-authored. Commit the template: one the
repo neither tracks nor ignores fails the run, because rendering
nothing would leave the stale file served. An ignored one is disowned
and left alone, a symlink is not followed, and one outside the source
tree was never verctl's to render — which is what keeps this crate's
changelog templates out. See docs/served-files.md.

A served mise task execs the tool from PATH, never
`"$(mise where <tool>)/<tool>"`. `mise where` resolves a version from
the surrounding config, so it ignores the task's own `#MISE tools`
pin, and mise already puts that pin first on PATH. Drop it only in
the same change that renders the pin from a template, or the task
freezes on whatever version the file happens to name.

Machines are configuration, not a hardcoded map. A machine is declared
once under `runners` with `labels`, the literal GitHub label list
that GitHub reads as AND — one machine, however many labels. Jobs name
machines: a `ci` job takes `runners`, one check each, and
an `assets` target takes a single `runner`, because one tarball is one
machine. A name resolves against `runners` or fails; only labels
reach `runs-on`. Omit `ci` for one `verify` on `ubuntu-latest`; PR CI
stays unary by default because compiling on two hosts for a PR is
waste. A built-in target (`darwin-arm64`, `linux-x64`) fills in
`runner`, `os`, `arch`, and `triple`; any other name must state all
four. `verctl ci` writes the matrix for a `plan` job, since `runs-on`
exists before the job does.

verctl decides how many jobs there are; the workflow file decides
where a fixed job runs. `ci` and `assets` exist because only the
repo knows how many verify checks or tarballs it wants, and a static
YAML file cannot declare N jobs without knowing N. `plan`, `crate`,
`pin`, and `prepare` are always exactly one job each, so their
`runs-on` is a literal in the workflow the consumer owns — not a
config key. Do not add one. There is no `release` runner section and
none is planned: publishing a crate twice is wrong the same way two
machines per tarball is wrong.

## Stop conditions

Stop and ask when a fragment is not valid YAML, when a package name is
unknown to the declarations, when a `major` fragment lands on a
0.x package, or when verctl refuses a `0.0.0` package's bump:
`first_minor` is the operator's to set.
