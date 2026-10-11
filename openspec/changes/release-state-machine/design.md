# Design

## Context

See [proposal.md](proposal.md) for why and [research.md](research.md) for the
prior art. Today verctl keeps no model of a release. Each command rebuilds the
state it needs from indirect signals:

1. `check --versions` decides "this is the Version PR" from a branch name or a
   label read from an event snapshot, which races its own label.
2. `publish` decides "this commit is a release" from a CHANGELOG heading and
   default-branch ancestry. Both stay true for every later commit, so a rerun
   can tag a commit that was never published.
3. "Already published" is six substrings of registry stderr.
4. The stack is guessed from the manifest file name in four places, the
   lockfile refresh is decided in three, the tag shape in three, the default
   branch from five inputs, and a file that carries a version has two
   mechanisms.

The workaround table below lists all 27 workarounds found, with the ledger rows
that propose more of the same.

## Goals / Non-Goals

**Goals:**

1. A release is an explicit state machine. Every state is derived from facts
   verctl can observe, every transition has a guard and a postcondition, and
   every transition is safe to repeat.
2. Cargo crates and Bun packages are native: version, dependents, lockfile,
   publish order, registry check, publish, and auth are built in.
3. One place per concern in `.ctl/ver.yaml`, no key that only prints, and every
   unknown key refused at every level.
4. Every workaround in the table is removed, and the ledger rows that proposed
   more of them close.

**Non-Goals:**

1. Pre-release mode. The model leaves room for it as a state; it is a later
   change.
2. Automating the first publication of a package. verctl reports it as a
   precondition and stops.
3. Ecosystems other than Cargo and Bun as native. npm, pnpm, yarn, Python and
   others use the `command` ecosystem.
4. Moving the actions out of this repository (VER-043) and the GitHub App
   decision (VER-044). The design states what each needs.

## 1. The model

### 1.1 Facts

verctl stores no release state. It observes these facts on every run and
derives everything else from them.

| Fact | Observed from |
|:--|:--|
| Intent | fragments under `.changeset/` on the commit being read |
| Declared version | each package's manifest at a commit |
| Release commit `R(p, v)` | the first-parent history of the default branch: the commit where package `p`'s manifest version became `v` |
| Lockfile agreement | the lockfile's entry for each workspace package equals its manifest version |
| Published | the registry: the crates.io sparse index entry, the npm packument, or the `command` ecosystem's `published` argv |
| Tagged | the remote tag for `p@v`, peeled to a commit |
| Released | the GitHub Release for that tag: draft or published |
| Assets | each declared asset on that Release, with `state: uploaded` |

### 1.2 Stages

Each package has two independent axes.

The current release, for the version declared on the default branch:

```text
merged ──publish──▶ published ──tag──▶ tagged ──release──▶ drafted ──asset──▶ … ──finalize──▶ released
   │                                                          (no assets: tagged ──release──▶ released)
   └── never published and the registry needs a first publish ──▶ bootstrap (stop, operator acts)
```

The next release, for fragments not yet applied:

```text
none ──fragment lands──▶ pending ──propose──▶ proposed ──merge──▶ (current axis: merged)
                                    proposed ──main moves──▶ stale ──propose──▶ proposed
                                    proposed ──plan empties──▶ stale ──propose──▶ none (PR closed)
```

A tag that peels to a commit other than `R(p, v)` is the `conflict` stage. It
stops every transition for that package until an operator resolves it.

### 1.3 Transitions

| Transition | Guard | Effect | Postcondition, checked after |
|:--|:--|:--|:--|
| `propose` | no package's current release is short of `released`; the plan is not empty | apply the plan to a tree at the default branch tip, commit it on the proposal branch, open or update the Version PR | the proposal branch passes `check` |
| `propose` (close) | the plan is empty and a Version PR is open | close it with a comment naming why | no Version PR is open |
| `publish` | HEAD is `R(p, v)`; every workspace dependency of `p` is published; the registry does not list `p@v` | the ecosystem publishes `p` | the registry lists `p@v` |
| `tag` | every package the tag names is published | create the tags at `R(p, v)` and push all ready tags in one atomic push | each remote tag peels to `R(p, v)` |
| `release` | the tag exists | create the GitHub Release as a draft when assets are declared, otherwise published; if one exists, update its body | the Release exists with the CHANGELOG section as its body |
| `asset` | the Release is a draft; the asset is not uploaded | build at `R(p, v)` and upload | the asset is `uploaded` |
| `finalize` | every declared asset is uploaded | publish the draft | the Release is published |

Rules that hold across all transitions:

1. A transition whose postcondition already holds does nothing and reports
   `already`. A rerun on any later commit is therefore safe.
2. A failed transition stops the run. Nothing that depends on it is attempted.
   The next run starts from the facts.
3. `release` handles the oldest unreleased release commit first, then the
   next. It never reads "the release" from HEAD.
4. Publish happens before tag. A tag never claims a version the registry does
   not have.
5. verctl never decides one fact from another. A published version does not
   imply a tag, and a tag does not imply a Release.

### 1.4 The Version PR is verified by its content

`check` runs on every pull request. Let `base` be the merge-base with the
default branch.

1. The consumed fragments are those present at `base` and absent at HEAD.
2. Every package whose manifest version differs from `base` must be exactly
   what the plan of the consumed fragments gives, including dependents.
3. Every planned bump must appear, and each changed package's CHANGELOG must
   carry the section for its new version.
4. Every lockfile agrees with the manifests.
5. Every served file equals what its template renders at the versions on HEAD.

A hand-edited version fails rule 2 because no consumed fragment justifies it.
The Version PR passes on its first run because its content is the proof. No
branch name, label, or event payload is read. A Version PR consumes fragments
that are already on the default branch; a pull request that adds a fragment and
applies it fails rule 2.

## 2. Ecosystems

The manifest file name selects the ecosystem: `Cargo.toml` is `cargo`,
`package.json` is `bun`. A `package.json` with no tracked `bun.lock` in scope
fails the load and names `ecosystem: command`. Any other manifest declares
`ecosystem: command`.

| Concern | cargo | bun | command |
|:--|:--|:--|:--|
| Version | `package.version`, or `workspace.package.version` when the package inherits it | `version` | `version.read` / `version.write` argv |
| Unpublished packages | `publish = false` in Cargo.toml | `"private": true` | `published` argv is absent |
| Dependents | workspace path dependencies with a `version` requirement, from `cargo metadata --no-deps` | workspace dependencies through `workspace:` | none |
| Publish order | normal and build dependencies; dev dependencies excluded | `dependencies`, `peerDependencies`, `optionalDependencies` | declaration order |
| Lockfile | `cargo update --workspace` when `Cargo.lock` is tracked | `bun install --lockfile-only` when `bun.lock` is tracked; Bun 1.4.1 or later required | `lockfile.run` argv and its `lockfile.writes` globs |
| Lockfile check | parse `Cargo.lock`: each workspace package entry equals its manifest | parse `bun.lock`: each workspace `version` equals its manifest | none |
| Published? | sparse index entry for the registry | packument for the registry | `published` argv exit status |
| Publish | `cargo publish --locked -p <name>`, one package at a time | `bun pm pack`, inspect the packed package.json, `bun publish <tarball>` | `publish` argv |
| Auth | OIDC exchange with crates.io, or `CARGO_REGISTRY_TOKEN` | one OIDC exchange per package with npm, or a registry token | the argv's own environment |
| Asset build | `cargo build --release --locked --target <triple>` | none built in | `assets.build` argv |

The packed package.json check refuses a tarball that still holds `workspace:`
or `catalog:`, or whose dependency on a workspace package names a version other
than that package's manifest.

## 3. Configuration

One file, `.ctl/ver.yaml`, one place per concern. Every struct at every level
refuses unknown keys. A removed key fails the load and names its replacement.

| Concern | Key | Default when unsaid |
|:--|:--|:--|
| Default branch | `branch` | `main` |
| Packages | `packages[].name`, `packages[].path` | required |
| Ecosystem | the manifest file name; `packages[].ecosystem.command` for anything else | by file name |
| Registry | `packages[].registry`, naming an entry of `registries` | `crates-io` for cargo, `npm` for bun |
| Registry URL and auth | `registries.<name>.url`, `registries.<name>.auth` (`oidc` or `token`) | built in: `crates-io` and `npm`, both `oidc` |
| Changelog | `packages[].changelog` | `CHANGELOG.md` beside the manifest |
| First release policy | `packages[].first_minor` | `false` |
| Native binaries | `packages[].assets.<target>`, naming a runner | none |
| Machines | `runners.<name>.labels` | built-in hosted runners |
| Validation jobs | `ci.<job>.runners` | one `verify` on `ubuntu-latest` |
| Tag shape | `tags` | `v{version}` |
| Version PR | `proposal.branch`, `proposal.title`, `proposal.commit_message` | `version-packages`, `chore(release): version packages`, the title |
| Served files | `templates.source`, `templates.suffix` | `.ctl/templates`, `.jinja` |

Removed, each with what replaces it:

| Removed | Replaced by |
|:--|:--|
| `drivers`, `packages[].driver`, `format`, `keys`, `read`, `write` | the ecosystem; `ecosystem.command.version` for other stacks |
| `drivers.*.after`, `packages[].after` | the ecosystem's lockfile step |
| `publishers`, `packages[].publisher`, `noun`, `publish` | the ecosystem's publish; `ecosystem.command.publish` |
| `packages[].registry` as alias or URL, `config_name` | `registries` named once |
| `prepare.after`, `prepare.stage`, `prepare.stage_ignored` | the ecosystem's lockfile step; `ecosystem.command.lockfile` |
| `prepare.version_label` | content verification in `check` |
| `pins`, `patterns` | templates |
| `tags.template` | `tags` |
| top-level `assets` | `packages[].assets` |

Templates receive the versions map, a `tags` map from package name to its tag,
and the verctl surface. They render through `ctl_core::served` (ctl-core
0.6.10), so verctl and its tests use one renderer. verctl keeps discovery, the
`templates` config, and the context.

A repository like unixd, two crates with their own tags, writes:

```yaml
packages:
  - name: unixd-cache
    path: crates/unixd-cache/Cargo.toml
  - name: unixd
    path: crates/unixd/Cargo.toml
tags: "{name}-v{version}"
```

The lockfile refresh, the publish order, and the registry follow from the
manifests.

## 4. Commands

| Command | Does | Replaces |
|:--|:--|:--|
| `status` | both axes per package, the next transition, and the versioned plan (`--format json`, `--github-output`) | `status`, `assets --github-output`, `ci` stays separate |
| `check` | the five rules of 1.4 | `check`, `check --versions` |
| `version` | apply the plan to the working tree: manifests, dependents, lockfiles, changelogs, served files, consumed fragments; report what ran under `after` | `prepare` |
| `propose` | `version` on the default branch tip, then open, update, or close the Version PR | `prepare --pr` |
| `release` | reconcile the oldest unreleased release commit: publish, tag, release, finalize | `publish` |
| `release asset <target>` | build and upload one asset | `assets --build --upload` |
| `render` | re-render every served file at the versions on HEAD | `pin` |
| `ci` | plan the validation jobs | unchanged |
| `instructions` | print the installed contract | unchanged |

`--dry-run` on `version`, `propose`, and `release` reports the transitions it
would run and changes nothing.

## 5. Workflows

Two workflows replace three.

1. `ci.yml` on `pull_request`: `verify` runs `verctl check`. No `labeled` or
   `unlabeled` types.
2. `release.yml` on push to the default branch and `workflow_dispatch`:
   1. `plan`: `verctl status --github-output` emits the release commit, the
      tags, and the asset matrix.
   2. `publish`: when a release is pending, check out the release commit and
      run `verctl release`.
   3. `assets`: one job per target, each checks out the release commit and runs
      `verctl release asset <target>`.
   4. `finalize`: `verctl release` again, which publishes the draft.
   5. `propose`: `verctl propose`, with a token whose pull request events start
      CI without approval.

A pull request opened with `GITHUB_TOKEN` has its CI runs held for approval,
so `propose` needs a GitHub App token. VER-044 decides whether verctl gets its
own App.

## 6. Decisions

Each decision names the alternative that lost.

1. **State is derived from facts, every run.** Alternative: a state file or a
   versions manifest (release-please, Changesets pre mode). It lost because a
   partial failure leaves the file wrong, and the facts are cheap to read.
2. **The release commit comes from first-parent history.** Alternative: HEAD,
   or "HEAD changed the version" (bun-release). Both lose a release once
   another commit lands. Alternative: a trailer in the merge commit. It lost
   because squash, merge, and rebase merges treat trailers differently.
3. **Content verification replaces the label and the branch name.**
   Alternative: create the PR with the label in one call. It lost because it
   still trusts a signal anyone with write access can set, and still needs the
   event to carry it.
4. **Push to the default branch triggers release, then propose.** Alternative:
   publish on `pull_request: closed` from `version-packages`. It lost because
   it ties publishing to an event that happens once, so a failed run needs a
   manual dispatch on the right commit.
5. **The package manager writes the lockfile, and `check` reads it.**
   Alternative: edit Cargo.lock and bun.lock as text (knope, release-please).
   It lost because the edit is wrong when two versions of a crate are in the
   graph, and because bun's lockfile shape is bun's to define. Override: none,
   because a lockfile that disagrees with its manifest breaks `--locked` and
   ships stale dependency versions. Disable path: do not track the lockfile.
6. **Dependents are bumped from the workspace graph.** Override: a fragment may
   bump a dependent further. Disable path: none, because a path dependency
   whose requirement excludes the new version does not build.
7. **The registry decides "published".** Alternative: stderr text, or
   `--tolerate-republish` alone. Both lost: text changes between registries,
   and the flag needs auth and cannot plan.
8. **Publish one package at a time in dependency order.** Alternative:
   `cargo publish --workspace`. It lost because it refuses to run when any
   selected version exists, so it cannot resume.
9. **A release with assets is a draft until every asset is uploaded.**
   Override: none. Disable path: declare no assets.
10. **verctl performs the OIDC exchange.** Alternative: require
    `rust-lang/crates-io-auth-action` and a bun workaround in every workflow.
    It lost because Bun has no exchange at all, and the npm exchange is per
    package. Disable path: `registries.<name>.auth: token`.
11. **Templates are the only served-file mechanism.** Alternative: keep pins
    for files a repository writes by hand. It lost because the four pinned files
    in two repositories are all expressible as templates, and two mechanisms
    for one concern is what VER-020 and VER-028 exist to police.
12. **No hooks.** Alternative: keep `prepare.after` for custom steps. It lost
    because every known use is a lockfile refresh, which the ecosystem now owns,
    and an unknown command writing unknown files is what `stage` and
    `stage_ignored` were invented to admit.
13. **`first_minor` stays.** It is an operator policy written in the file the
    pull request shows, not a workaround. It applies only to a package at
    `0.0.0`.

## 7. Workarounds removed

| Id | Workaround | Removed by |
|:--|:--|:--|
| W1 | `prepare.after: [cargo, update, --workspace]` plus `stage: [Cargo.lock]` in six repositories, and the Rust toolchain each consumer's Version PR job installs for it | the cargo ecosystem refreshes the lockfile (2); the six sections are deleted |
| W2 | driver and package `after`, printed in a `next` row | removed; `version` runs the step and reports it under `after` |
| W3 | dead `drivers.toml` | deleted |
| W4 | `prepare.stage_ignored` | removed with `prepare` (decision 12) |
| W5 | the label race and the `labeled`/`unlabeled` CI trigger (VER-029) | content verification (1.4); the trigger types go |
| W6 | the guard exemption on the `version-packages` branch | content verification (1.4) |
| W7 | `head.ref == 'version-packages'` in every publish workflow | push-to-main release (5) |
| W8 | a Version PR never closes; `none`-only fragments are never consumed | `propose` closes; consumed means present at base and absent at HEAD, whatever the bump |
| W9 | `VERCTL_DEFAULT_BRANCH`, the default-branch fetch, five default-branch inputs | the `branch` key; the release commit is derived, not proved |
| W10 | a rerun can tag a commit that was never published | release commit from history (1.1); tag after publish (1.3) |
| W11 | "already published" from error text; `--tolerate-republish` reported as a fresh publish | registry query before publish (decision 7) |
| W12 | publish order by declaration order | dependency graph (2) |
| W13 | `mise.release.toml` hand-bumped bootstrap pin; consumers pin verctl twice | this repository deletes the file, which no lane loads; consumers pin one action ref once VER-043 releases the action with the verctl it runs |
| W14 | pins and templates for one concern | templates only (decision 11) |
| W15 | `?ref=v` hard-coded in the pin regex and a template | the `tags` map in the template context |
| W16 | operator docs rendered on a second, test-only path | `ctl_core::served` renders every served file; `render` updates them and `check` compares them |
| W17 | the `#USAGE mount` line copied into a template and pinned by a test and a doc rule | the template reads it from the context |
| W18 | `first_minor` | kept as policy (decision 13) |
| W19 | `VERCTL_PR_TITLE`, `VERCTL_COMMIT_MESSAGE` | `proposal.title`, `proposal.commit_message` |
| W20 | `CACHE_WRITE`, because publish runs on a pull request event | release runs on the default branch |
| W21 | an App token in four repositories while the docs say "not a GitHub App" | the docs state the token `propose` needs (5); VER-044 decides the App |
| W22 | the CI plan job compiles verctl for a verb the release already has | kept by design in this repository, which dogfoods HEAD; consumers use the released action; VER-016 closes as stale |
| W23 | assets take the first package's version and tag | `packages[].assets` |
| W24 | example workflows maintained by hand | served from templates, so `check` keeps them current |
| W25 | documented features that do not exist: `internalAuthors`, changelog links, overridable changelog templates, OIDC, pre-release bootstrap text, served-file checks | each is built here (OIDC, served-file checks) or deleted from the docs (`internalAuthors`, links, template override, bootstrap text) |
| W26 | unused code: `run_limited`, `Skip::Ci`, `noun` | deleted |
| W27 | schema rules the code does not keep: no `deny_unknown_fields` below the top level, silent pin skips, lazy runner names, `version_label` treating `None` and `""` alike | every struct refuses unknown keys; names resolve at load; `version_label` is gone |

Ledger rows that propose more of the same close when the phase that removes
their cause lands: VER-013, VER-016, VER-020, VER-028, VER-029, VER-030,
VER-031, VER-033, VER-034, VER-035, VER-036, VER-038, and VER-010.

## 8. Migration

Each phase is one pull request with a `minor` fragment, and updates qctl,
forkctl, ctl-core, unixd, and yamled in the same window. There is no
compatibility layer: a removed key fails the load and names its replacement.

1. **Ecosystems (0.4.0).** cargo and bun modules: version, dependents, lockfile
   refresh and check, publish order, registry query. `version` and `propose`
   replace `prepare`; the report field `next` becomes `after`. Remove
   `drivers`, `publishers`, the per-package driver keys, and `prepare`.
   Refuse unknown keys everywhere. Delete `drivers.toml`. Consumers delete their
   `prepare` section.
2. **Release reconciliation (0.5.0).** Derived release commit, `release` and
   `release asset`, draft releases, conflicts, atomic tag push, `status` with
   both axes, the `branch` and `tags` keys, `packages[].assets`. Workflows move
   to push-to-main.
3. **Content-verified check and the proposal lifecycle (0.6.0).** The five
   rules of 1.4, `propose` closing and refusing while a release is incomplete,
   the `proposal` keys. Remove the label, the branch exemption, and the
   `labeled` trigger.
4. **Served files (0.7.0).** `ctl_core::served`, the `tags` map, `render`,
   served-file checks, templates for the operator docs and the example
   workflows. Remove `pins` and `patterns`; qctl and forkctl move four files to
   templates.
5. **Auth (0.8.0).** OIDC exchange for crates.io and npm, `registries`, the
   `bootstrap` stage. bun-release's publish path folds in here.

Rollback for any phase is a revert and a `patch` release; consumers restore the
previous config from git.

## 9. Risks

1. [Five repositories change config in every phase] → Each phase lists the
   exact edits per repository, and a removed key fails with its replacement.
2. [The release commit walk reads history] → Workflows already fetch full
   history; the walk stops at the last tagged version of each package.
3. [Bun below 1.4.1 leaves the lockfile stale] → The bun ecosystem refuses an
   older Bun by version.
4. [OIDC needs registry-side setup] → The `bootstrap` stage names the missing
   setup and stops before any side effect; `auth: token` remains.
5. [A Version PR that adds and applies a fragment in one pull request now
   fails] → Intended: fragments land with the change that needs them, and the
   Version PR applies what is on the default branch.
