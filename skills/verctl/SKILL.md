---
name: verctl
description: >-
  Operate verctl version PRs from Changesets-format fragments. Use when the
  user mentions verctl, mise run ver, Version Packages, prepare-release,
  .changeset fragments, or stack-agnostic changelog templates. Not
  @changesets/cli and not a forkctl verb.
license: MIT
version: 0.8.6-rc.1
---

# verctl

`verctl instructions` is the installed contract.  
Actions run `verctl` from `PATH`.

## Invocation

```sh
mise run ver status
mise run ver check
mise run ver version --pr
```

Never `mise run ver --`.

The `--` in `#USAGE mount` is mise's completion bootstrap.

## Commands

| Command | Aliases | Purpose |
|:--|:--|:--|
| `instructions` | — | Print the installed-version operator contract |
| `status` | — | List pending fragments, the asset plan, and HEAD's incomplete releases. `--github-output` writes the release workflow's plan |
| `check` | — | Run the named checks, or `fragments` and `served` when none is named |
| `version` | — | Apply fragment bumps. `--pr` opens the Version PR. `--dry-run` previews |
| `release` | — | Release the versions on HEAD: publish with cargo or bun, and create the GitHub Release. `release asset <target>` builds one asset |
| `render` | — | Rewrite pins and render served templates at the versions on HEAD |
| `ci` | — | Plan the validation jobs declared in `[ci]` |
| `semver` | — | Check the API of each crates.io crate whose next release is a patch |

Every long option also reads `VERCTL_<LONG>` (`VERCTL_CONFIG`, `VERCTL_DRY_RUN`, `VERCTL_FORMAT`).  
A flag wins over its variable, an empty variable is unset, and `--help` names each one.  
A workflow or action sets them in `env:` and runs the bare verb; never rebuild argv in a shell.

## Output

Every command returns one typed report.  
Use `--format json` for the same data without ANSI, whatever `CLICOLOR_FORCE` or `FORCE_COLOR` hold.  
JSON errors use stdout, while human errors use stderr.  
`--quiet` suppresses only successful human output.

## Workflow

1. A pull request that changes behaviour adds a fragment.
2. Each push to the default branch runs the release workflow.  
   It releases every incomplete version HEAD holds, from HEAD, then opens or updates the Version PR.
3. A person merges the Version PR, and verctl never merges it.
4. The push that merges it releases the new versions.

## Configuration

A repo declares everything in `.ctl/ver.yaml`, the one file verctl reads.  
`.ctl/` is shared with the other ctl CLIs, and templates live in `.ctl/templates/`.  
There is no `verctl.toml`.

## Fragments

### Format

Fragments are `.changeset/*.md` with YAML fences (quoted or unquoted keys).  
An optional `verctl:group: added | changed | fixed` key files the entry under that heading.  
Once one fragment in a section names a group, an ungrouped one goes under Changed.  
`patch!` ships an API break as a patch: `semver` skips the package while the fragment is pending, and the entry goes under Breaking.  
Changelog Markdown is rendered with minijinja.

### The first release

A new package starts at `0.0.0`.  
Its first fragment is a `patch`, so its first release is `0.0.1`.  
Moving to `0.1.0` or `1.0.0` is a separate operator decision, never the first fragment's bump.

`version` and `check` refuse a `minor` or `major` fragment on a `0.0.0` package.  
Only the operator sets a package's `first_minor: true` in `.ctl/ver.yaml`, which opens the package at `0.1.0`.  
`release` and `status` leave a package at `0.0.0` out, so a repo can adopt the release workflow before its first release.

### Release candidates

Only the operator sets a package's `prerelease: <tag>` in `.ctl/ver.yaml`, such as `prerelease: rc`.
While it is set, each Version PR writes a candidate such as `0.8.2-rc.1`, published and released so that only a consumer who names it receives it.
Removing the key makes the next Version PR write the stable version, whose section lists every candidate's changes.
`verctl instructions` holds the version rules and what each publisher does with a candidate.

## Changelog

Each entry links the pull request or commit that added its fragment.  
List maintainers and Apps in `changelog.internal_authors`.  
Only an unlisted person whose pull request comes from a fork gets a byline.

## Release workflow

Consumers use verctl's reusable workflow (`.github/workflows/release.yml`, called from a thin caller), not changesets/action.  
Its steps run `mise run ver <verb>` with settings in `env:`.

## Version PR

verctl never merges the Version PR or enables auto-merge on it: a release starts only when a person merges it.

`version --pr` opens the Version PR against `GITHUB_REF_NAME`, else `VERCTL_DEFAULT_BRANCH`, else the branch `origin/HEAD` names.  
It fails before writing anything when none is set; it never guesses `main`.

`proposal.title` and `proposal.commit_message` in `.ctl/ver.yaml` set the title and commit message.  
Unset or empty, the title is `chore(release): version packages` and the message is the title.  
The Version PR gets no label.

### Checks on pull requests

`check [CHECK]...` runs `fragments`, `served`, `versions`, `registry`, and `trust`, each once, as named; bare `check` runs `fragments` and `served`, and `--all` (`-a`) runs every check.  
The PR comes from `verctl/release/<base>`, derived and never configured.  
On that branch, `check versions` replays the version step on the merge-base in place of the version comparison.  
It fails any change the replay does not make.  
The replay runs no lockfile owner and no `prepare.after` command.

Any other pull request fails when it adds a CHANGELOG section for a version.  
It also fails when it changes whether a package publishes without a fragment that bumps it above `none`.

`check registry` needs no token: each crates.io crate the default branch holds exists and lists every login `owners` names.  
It reports the newest version's trusted publishing as evidence.  
A crate only the pull request adds passes with a notice to run `verctl release bootstrap` once it lands.  
`check trust` needs an owner's token in `VERCTL_OWNER_TOKEN`.  
It proves that a trusted publisher entry names the repository and `release_workflow` (`release.yml` when omitted).

## Release

### HEAD is the release reference

`release` releases, from HEAD, every version HEAD's manifests hold whose release is incomplete, whichever commit set it.  
`status` lists those (registry, tag, published Release, assets unless `--skip-assets`) under `unreleased`.  
It needs `GITHUB_TOKEN` or `GH_TOKEN`.  
Without one, or when a lookup cannot run, it reports `unreleased` as not checked.

`status --github-output <file>` fetches the history and writes the `commits`, `pending`, `assets`, `declared`, `has_assets`, `tag`, and `matrix` outputs.

`release` reports `nothing to release` when every release is complete.  
A version HEAD's manifests do not hold stays unreleased.  
A Release with declared assets is a draft until every asset is uploaded, and `release` run again publishes it.  
A hand-written workflow that runs `actions/publish` and then `actions/asset` runs `verctl release` again after its uploads.  
`--skip-assets` also publishes an existing draft without its assets.  
A release tag on a commit HEAD holds only through a merge passes when the version there matches HEAD's.

### Proofs and refusals

Before any upload, tag, or Release, `release` proves the release commit.  
The default branch contains it.  
The `.github/workflows/` tree at `GITHUB_SHA`, else HEAD, equals the one at the default branch's tip.

`--allow-off-default-branch` and `--allow-workflow-change` go past one, and the report lists it under `allowed`.  
Their variables are `VERCTL_ALLOW_OFF_DEFAULT_BRANCH` and `VERCTL_ALLOW_WORKFLOW_CHANGE`, and the `release.yml` inputs are `allow-off-default-branch` and `allow-workflow-change`.  
A check that cannot run refuses anyway.

Once something is left to release, and before the draft, `release` runs the checks `release_checks` names: `[registry, trust]` when omitted, `[]` for none.  
`trust` runs only with `VERCTL_OWNER_TOKEN`, and is reported as `skipped` otherwise.  
A failed check stops the run with nothing published.

`release --dry-run` (`-n`) prints the plan and runs the same checks.  
`--crates-io-api` (`VERCTL_CRATES_IO_API`) and `--crates-io-index` (`VERCTL_CRATES_IO_INDEX`) point the run at another crates.io, such as staging.

`verctl release` fetches the default branch (that one ref, with history) only when `origin/<default>` is missing.  
It rewrites `origin/HEAD` only from `VERCTL_DEFAULT_BRANCH`.  
GitHub git HTTPS uses `x-access-token`, never Bearer, and never puts the token in the URL.

### Registry tokens and checks

`release` reads the registry token from `VERCTL_REGISTRY_TOKEN` or `VERCTL_TRUSTED_PUBLISHING_TOKEN`, never from a flag.  
It fails when both are set.  
Only the publisher and its registry `query` get the token, under `--registry-token-env` (`VERCTL_REGISTRY_TOKEN_ENV`, default `CARGO_REGISTRY_TOKEN`).  
No command verctl starts sees `GITHUB_TOKEN` or `GH_TOKEN`.  
`release.yml` sets all three in its publish step.

`release` checks the registry before and after each publisher.  
For a stock cargo package that publishes to crates.io, it reads the crates.io sparse index.  
Otherwise it runs the recipe's `query`.  
A listed version is `already`.

A publisher that exits zero while the registry never lists the version fails.  
Set `query` on a package or a `publishers` entry for another registry, and `query: []` to turn the check off.

`publish` as a mapping holds options of the package's publisher and keeps its registry check.  
`publish: {verify: false}` makes cargo upload with `--no-verify`, for a crate CI already packs on every commit to the default branch.  
`publish` as a list replaces the whole upload command.  
An option the publisher does not have fails the load.

## Tags and GitHub Releases

`tags.template` (default `v{version}`) names release tags.  
`{name}` makes one tag and one Release per package; without it, differing versions refuse.  
Each Release body is that package's CHANGELOG section.  
The tag is created at HEAD and re-read; a tag that names another commit fails.

## Assets

### Package tarballs

An `assets` tarball holds the one `binary`.  
Set `assets.dir` and `assets.files` instead when it holds more (a second executable, a service unit, a licence).  
`release asset` packs `tar czf <asset> -C <dir> <files>` with `{triple}` and the other placeholders replaced.  
A target that ships other files, such as a tool built only on Linux, lists its own `files`, which replace `assets.files` for that target.
`release asset --prebuilt <path>` (`VERCTL_PREBUILT`) packs a binary built earlier as `bin` and runs neither `prepare` nor `build`.
A target that packs `files` refuses it.

### Vendored sets

`vendored.<name>` declares release assets built from someone else's source at its own `version`, for a mise `github:` tool.  
Fragments and `release` leave it alone.  
`status --vendored <name>` (`VERCTL_VENDORED`) plans it.  
A config that vendors may declare no packages.

`release asset <id> --vendored <name>` runs its `build` and packs `files` from `dir` into `archive` (default `{name}-{version}-{id}.tar.gz`).  
Unless `--dry-run`, it creates the `tag` Release when it is missing (default `{name}-{version}`, never marked latest).  
It then attaches the archive and its `.sha256`.

## Version files

`publisher: none` on a package releases it on GitHub only (tag, Release, assets), with no registry upload.  
A manifest that opts out needs none: `Cargo.toml` with `publish = false` or `[]`, or `package.json` with `"private": true`.  
verctl has no stock Python publisher, so a `pyproject.toml` package names its `publisher`, or `none`.  
One whose `project.dynamic` lists `version` fails, because its build backend sets the version.

A `go.mod` package keeps its version as `// vX.Y.Z` on the `module` line.  
With no publisher, it releases on GitHub only, and a v2 bump needs the `/v2` module path first.

A `VERSION` file holds only the version.  
A missing one in an existing directory reads as `0.0.0` until `version` writes it.  
With no publisher it releases on GitHub only.

`marker: true` versions any one file, such as a reusable workflow, on its one `# version: X.Y.Z` line, and also releases on GitHub only.

## Pins and templates

### Served templates

Prefer a template over a pin: a served file is generated, and a `pins` entry admits a file is hand-authored.  
A template declares its own target with top-level `{%- set … -%}` exports.

Commit it: one the repo neither tracks nor ignores fails the run, because rendering nothing would serve the stale file instead.  
An ignored one is disowned and left alone; one outside the source tree was never verctl's to render.

`render` rewrites those pins and renders every template at the versions on HEAD.  
`check` fails when a served file or a pin differs from that render, whether or not fragments are pending.  
`render` is the fix for both.  
A file that a template and a `pins` entry both claim fails `version`, `check`, and `render` before any write.

### Served mise tasks

A served mise task execs the tool from `PATH`, never `"$(mise where <tool>)/<tool>"`.  
`mise where` resolves from the surrounding config, so it ignores the task's own `#MISE tools` pin.  
mise already puts that pin first on `PATH`.

Removing the `mise where` call is only safe together with rendering the pin.  
Otherwise the task freezes on whatever version the file happens to name.

### Version patterns

A `patterns.match` needs a boundary on each side of `{version}`.  
A side's literal is the text between `{version}` and the pattern's edge, on that side.

A side is bounded when its literal holds at least one character that is not a digit, `.`, `-`, or `+`.  
Under `whole_line: true`, the line start or end bounds the side, unless the literal on that side is made only of those four characters.

The character next to `{version}` must not be a digit, or a `.` with a digit beyond it: `v{version}.5x` would read `v1.2.3.5x` as 1.2.3.

The load refuses a pattern with an open side, so `0.1.1-rc1` fails to match rather than being half rewritten.  
A `?ref=v1.2.3-rc1` include fails too.

## Runners and CI

Machines are `runners` entries with `labels`, declared once and named by the jobs that run on them.  
A `ci` job takes `runners`, one check each.  
An `assets` target takes a single `runner`, because one tarball is one machine.  
Only `labels` reach `runs-on`; a name resolves against `runners` or fails.

verctl decides how many jobs there are; the workflow file decides where a fixed job runs.  
The release workflow's `plan`, `publish`, and `propose` jobs run on its `runner` input, a JSON label list that defaults to `["ubuntu-26.04"]`.  
It is never a `.ctl/ver.yaml` key, and there is no `release` runner section.

## Docs and semver

verctl waits for no docs.rs build: a repository checks its docs build in pull request CI, before the release.

`semver` runs `cargo semver-checks --release-type minor` on each crates.io crate whose next release is a patch, so only an API break fails.  
It skips a crate with a pending `minor`, `major`, or `patch!` fragment, or a minor the Version PR already moved.  
A crate at 1.0 or later fails: the rule covers 0.x.

## Stop conditions

Stop in any of these cases:

- a 0.x package gets `major`;
- verctl refuses a `0.0.0` package's bump;
- a fragment names a package that is not in `packages`.
