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

Section 10 lists all 27 workarounds found, with the ledger rows that propose
more of the same.

## Goals / Non-Goals

**Goals:**

1. A release is an explicit state machine. Every state is derived from facts
   verctl can observe, every transition has a guard and a postcondition, and
   every transition is safe to repeat.
2. Cargo crates and Bun packages are first-class built-ins, and no stack loses
   anything to them: every built-in is one implementation of a building block
   that a repository can replace or extend.
3. mise is part of the design: it installs and verifies verctl, runs every
   extension step, and owns `mise.lock`.
4. No lockfile is ever written by verctl. Each lockfile is written only by the
   tool that owns it; verctl reads lockfiles to check them.
5. One place per concern in `.ctl/ver.yaml`, no key that only prints, and every
   unknown key refused at every level.
6. Every workaround in section 10 is removed, and the ledger rows that proposed
   more of them close.

**Non-Goals:**

1. Pre-release mode. The model leaves room for it as a state; it is a later
   change.
2. More than one release base branch. The proposal branch name carries the
   base, so a second base is additive later.
3. Built-in ecosystems beyond Cargo and Bun. Other stacks use extensions.
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
| Release commit `R(p, v)` | the first-parent history of the release branch: the commit where package `p`'s manifest version became `v` |
| Lockfile agreement | the lockfile's entry for each workspace package equals its manifest version, read by verctl's parser |
| Published | the registry: the crates.io sparse index, the npm packument, or an extension's `published` check |
| Tagged | the remote tag for `p@v`, peeled to a commit |
| Released | the GitHub Release for that tag: draft or published |
| Assets | each declared asset on that Release with `state: uploaded` |

### 1.2 Stages

Each package has two independent axes.

The current release, for the version declared on the release branch:

```text
merged ──publish──▶ published ──tag──▶ tagged ──release──▶ drafted ──asset──▶ … ──finalize──▶ released
   │                                                          (no assets: tagged ──release──▶ released)
   └── never published and the registry needs a first publish ──▶ bootstrap (stop, operator acts)
```

The next release, for fragments not yet applied:

```text
none ──fragment lands──▶ pending ──propose──▶ proposed ──merge──▶ (current axis: merged)
                                    proposed ──main moves──▶ stale ──propose──▶ proposed
                                    proposed ──no fragment left──▶ stale ──propose──▶ none (PR closed)
```

A tag that peels to a commit other than `R(p, v)` is the `conflict` stage. It
stops every transition for that package until an operator resolves it.

### 1.3 Transitions

| Transition | Guard | Effect | Postcondition, checked after |
|:--|:--|:--|:--|
| `propose` | no package's current release is short of `released`; a fragment is pending | run the version step on a tree at the release branch tip, commit it on the proposal branch, open or update the Version PR | the proposal branch passes `check` |
| `propose` (close) | no fragment is pending and a Version PR is open | close it with a comment naming why | no Version PR is open |
| `publish` | HEAD is `R(p, v)`; every workspace dependency of `p` is published; the `verify` steps pass; the registry does not list `p@v` | the package's publish block runs | the registry lists `p@v` |
| `tag` | every package the tag names is published | create the tags at `R(p, v)` and push all ready tags in one atomic push | each remote tag peels to `R(p, v)` |
| `release` | the tag exists | create the GitHub Release as a draft when assets are declared, otherwise published; if one exists, update its body | the Release exists with the CHANGELOG section as its body |
| `asset` | the Release is a draft; the asset is not uploaded | build at `R(p, v)` and upload | the asset is `uploaded` |
| `finalize` | every declared asset is uploaded | publish the draft, then run the `released` steps | the Release is published and each `released` step's check passes |

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

## 2. Identifying the Version PR

### 2.1 What can identify a pull request

GitHub lets a pull request's title, body, base, and state change after it is
opened, but not its head branch or head repository: the update endpoint takes
no head ([GitHub REST][gh-pr-update]). Labels change too, and a label applied
after creation is missing from the `opened` event, which is the VER-029 race.

So the identity of a Version PR is three facts that cannot drift:

1. its head repository is the repository itself, not a fork;
2. its head branch is the proposal branch for its base;
3. its base is the release branch.

`propose` finds its pull request by those three facts through the pull request
list endpoint, filtered by `head` and `base`. `check` reads them from the
pull request under test. Title, body, and labels are never read.

### 2.2 Identity and content are both required

`check` applies two independent rules to every pull request:

1. **Identity.** Only a pull request that is a Version PR by 2.1 may change a
   manifest version or add a CHANGELOG section. Any pull request may add or
   remove a fragment.
2. **Content.** A Version PR's diff must be exactly the version step applied to
   its base: every changed version equals the plan of the fragments present at
   the merge-base and absent at HEAD, every lockfile agrees with its manifests,
   and every served file equals its template's render.

Identity alone trusts whoever pushes to the proposal branch. Content alone
lets any branch carry a release. Together, a hand edit fails on any branch,
and the Version PR passes on its first CI run with no label. A repository can
also protect the proposal branch with a ruleset so only the release identity
pushes to it.

Alternatives considered:

1. **Label** (today). Lost: mutable by anyone with write access, and absent
   from the event that opens the PR.
2. **Title or body marker.** Lost: both are editable.
3. **Content only** (the first draft of this design). Lost to identity plus
   content: the operator asked for deterministic identification, and content
   alone would let a feature branch carry a release.
4. **Head branch alone** (`GITHUB_HEAD_REF`). Lost: a fork can use the same
   branch name, which is why VER-012 refused it. The head repository check
   closes that.

### 2.3 How many release branches, and their names

The proposal branch is `verctl/release/<base>`, for example
`verctl/release/main`. It is derived, not configured.

| Option | Gain | Cost | Verdict |
|:--|:--|:--|:--|
| One Version PR per base branch | one place to review a release; dependents and the shared lockfile stay in one diff | packages release together | **chosen** |
| One Version PR per package (release-please `separate-pull-requests`) | packages release on their own schedule | dependents span PRs that must merge in order; every PR rewrites the shared `Cargo.lock` or `bun.lock`, so each merge conflicts the others | rejected |
| Branch named after the version (`release/v0.4.0`) | the name says what ships | the version changes while the PR is open (a `minor` fragment turns `0.3.13` into `0.4.0`), and a PR's head branch cannot be renamed, so each change would close the PR and open another, losing its review; with several packages there is no single version to name | rejected |
| Branch named after the base | deterministic; a maintenance base such as `release/0.3` gets its own PR without a rename | none found | **chosen** |

Changesets uses the same rule (`changeset-release/<base>`); release-please
names a branch per base and per component.

## 3. Building blocks

### 3.1 The blocks

A release is built from these blocks. Each has one contract: its inputs, what
it may change, and the fact verctl checks after it runs.

| Block | Does | May change | verctl checks after |
|:--|:--|:--|:--|
| Manifest | read and write a package's version; list its workspace dependencies; say whether it publishes | the manifest | the version reads back |
| Lockfile | bring a lockfile into agreement with the manifests by running the tool that owns it | the declared lockfile only | the parser reads agreement |
| Artifact | produce the file a registry receives, and inspect it | a scratch directory | the inspection passes |
| Registry | query "is `p@v` published", publish an artifact, wait until visible, authenticate | the registry | the query lists `p@v` |
| Forge | the Version PR, tags, Releases, assets | GitHub | each object reads back |
| Served files | render templates at the versions on HEAD | the rendered files | each file equals its render |
| Binary asset | build a native binary for one target | a scratch directory | the archive exists |
| Step | run a mise task or an argv at a named point | only its declared `writes` | its `check`, when declared |

### 3.2 Providers

An ecosystem is a set of providers for the Manifest, Lockfile, Artifact, and
Registry blocks.

1. **Built-in `cargo` and `bun`.** Selected by the manifest file name.
2. **Declared ecosystems.** A repository names one under `ecosystems` and gives
   each block a step. A package selects it with `ecosystem: <name>`.
3. **Per-package override.** Any package may replace one block of its
   ecosystem with a step and keep the rest built in, for example a cargo crate
   that publishes through a mise task while verctl still bumps its version and
   refreshes `Cargo.lock`.

So a built-in is a default, never a ceiling.

### 3.3 Steps: the extension unit

A step is named once under `steps` and listed by name where it applies.

```yaml
steps:
  version-file:
    task: version:write-file        # a mise task, run with `mise run --raw`
    writes: [src/version.rs]        # the only paths it may change
  smoke:
    run: [bun, run, smoke]          # or an argv
  announce:
    task: release:announce
    check: [bun, run, announce:sent]  # postcondition; exit 0 means done
```

A step listed in the top-level `at` runs once per run; a step listed in
`packages[].at` runs once for that package. Inputs reach a step as
environment variables: `VERCTL_PACKAGE`, `VERCTL_VERSION`,
`VERCTL_PREVIOUS_VERSION`, `VERCTL_TAG`, `VERCTL_RELEASE_COMMIT`, and
`VERCTL_PLAN`, the path of the plan as JSON; a run-level step gets only
`VERCTL_PLAN`. The outcome is the exit status. verctl then checks what the step
was allowed to do.

Steps attach at these points:

| Point | When | Contract |
|:--|:--|:--|
| `version` | after manifests and lockfiles are written, before served files render | may change only its `writes`; its changes ride the Version PR |
| `verify` | before any release side effect | may change nothing; a failure stops the release |
| `publish` | replaces a package's built-in publish | the registry query must list `p@v` afterwards |
| `released` | after `finalize` | must declare `check`, so a rerun knows it already ran |

Steps replace `prepare.after`, `prepare.stage`, and `prepare.stage_ignored`,
and they answer VER-034's request for publish-time steps. The difference from
the old hooks is that every step has a declared effect verctl checks, and
nothing it does is only printed.

Alternatives considered:

1. **No hooks** (the first draft). Lost: a stack the built-ins do not cover
   would have no way to release.
2. **Free-form hooks** (today's `prepare.after` plus `stage` globs). Lost: the
   effect is unknown, so `stage` and `stage_ignored` had to be invented to
   admit it, and the per-package `after` only printed.
3. **A plugin interface** (WASM or a library ABI). Lost: heavy to build and to
   keep stable; a mise task is already a portable extension unit with its own
   tools, environment, and dependency graph.

### 3.4 Built-in cargo

| Block | Provider |
|:--|:--|
| Manifest | `package.version`, or `workspace.package.version` when inherited; dependents from `cargo metadata --no-deps`; `publish = false` never publishes |
| Lockfile | `cargo update --workspace` when `Cargo.lock` is tracked |
| Artifact | packed by `cargo publish` itself |
| Registry | sparse index query; `cargo publish --locked -p <name>` one package at a time in dependency order; the crates.io OIDC exchange when `auth: oidc` |
| Binary asset | `cargo build --release --locked --target <triple>` |

### 3.5 Built-in bun, and bun-release in Rust

| Block | Provider |
|:--|:--|
| Manifest | `version`; dependents through `workspace:`; `"private": true` never publishes |
| Lockfile | `bun install --lockfile-only` when `bun.lock` is tracked; Bun 1.4.1 or later |
| Artifact | `bun pm pack`, then verctl inspects the packed `package.json`: no `workspace:` or `catalog:`, and each workspace dependency names its current manifest version |
| Registry | verctl's own npm registry client: packument query, the per-package OIDC exchange, the publish request, staged publish, dist-tags, trust configuration, sigstore provenance |

This is bun-release in Rust: its OIDC exchange, packument check, visibility
wait, tag and Release, and bootstrap all become verctl code, and the
TypeScript library retires once the bun ecosystem ships.

| Option | Owns | Gains | Costs |
|:--|:--|:--|:--|
| (a) shell out to `bun publish` | nothing past the argv | least code | no provenance, no staged publish, text-only output, no OIDC, so verctl would still need its own exchange |
| (b) `bun pm pack`, then verctl's registry client | the registry protocol and auth | Bun keeps packing, so the tarball is what `bun publish` would ship; provenance, staged publish, OIDC, and typed results become possible | verctl maintains a registry client; it must inspect the packed manifest because Bun reads workspace versions from `bun.lock` |
| (c) native packing and publishing | everything | no Bun at publish time | a port of npm's file-selection rules with no maintained crate, and rules that drift from Bun's (Bun always packs a root CHANGELOG; pnpm resolves `workspace:` from manifests, Bun from the lockfile) |

Option (b) is chosen. pnpm 12 and aube are working Rust references for the
request body, the OIDC exchange, and provenance, but neither offers them as a
library: pnpm's crates declare `publish = false`, and aube's registry crate
has no publish. verctl therefore writes its own client on `reqwest`, signs
provenance with the `sigstore-sign` crate, and follows the wire format npm,
pnpm 12, and Bun all send.

## 4. mise

verctl ships through mise and runs inside mise tasks. The design uses mise for
what it owns and keeps out of what it does not.

| Concern | Owner | How |
|:--|:--|:--|
| Install and verify verctl | mise `github:` backend | asset chosen by os and arch tokens in its name; checksum from the GitHub asset digest; GitHub artifact attestations checked by default on public GitHub |
| Run an extension step | mise | `mise run --raw <task>`; the task's `#MISE tools` install first; the exit status is the outcome; `--raw` keeps stdout clean when a config sets redactions |
| Order work inside a step | mise | a step's task may `depends` on other tasks; verctl never builds a task graph |
| `mise.lock` | mise | verctl never edits it; only `mise lock` writes it; verctl compares its parsed entries, never its bytes, because mise 2026.10.3 and 2026.10.7 write the same two `rust` entries in opposite order, and `min_version` can set only a floor, not one version for every machine |
| A repository's own tool pin | the built-in `mise-pin` step, listed under `released` | after `finalize`, it runs `mise lock <tool>@<version>` with `MISE_LOCKFILE_MODE=generate`, so a version with no release changes neither file (the default mode rewrites `mise.toml` before it fails); then verctl reads `mise.lock` and requires a URL and checksum for every platform the lock lists, because `mise lock` skips a platform with no asset and still exits 0, and `mise install --locked` checks only the runner's platform |
| Package lockfiles | the package manager, called by verctl | `[deps]` is experimental and has no cargo provider, so verctl runs `cargo` and `bun` from PATH, where mise put them |
| Versions, changelogs, the Version PR, publishing | verctl | mise has no model for them |

What a release must therefore carry for mise to install and verify it: asset
names with os and arch tokens, the GitHub asset digest (GitHub computes it),
and GitHub artifact attestations, which the asset job creates. mise records
`provenance = "github-attestations"` in `mise.lock` only when a release has
them. Today's verctl releases have none, so consumers' locks carry a checksum
and no provenance. A recorded provenance is trusted as written by default: an
install checks the checksum and does not verify the attestation again, so a
hand edit to `mise.lock` can claim provenance that never existed. With
`MISE_LOCKED_VERIFY_PROVENANCE=1`, mise verifies the attestation on every
install and refuses a claim that does not verify, so the served CI workflow
sets it on `mise install --locked`.

Two mise facts bound the design. `MISE_LOCKFILE_MODE=generate` is documented
as a trial mode; if it changes, the `mise-pin` step keeps its read-back check
and stops on a partial write instead of committing it. And from mise
2026.10.8, a task granted secrets ignores `--raw` unless the task itself is
raw or interactive, so a step that needs secrets declares `raw = true`.

## 5. Configuration

One file, `.ctl/ver.yaml`, one place per concern. Every struct at every level
refuses unknown keys. A removed key fails the load and names its replacement.

| Concern | Key | Default when unsaid |
|:--|:--|:--|
| Release branch | `branch` | `main` |
| Packages | `packages[].name`, `packages[].path` | required |
| Ecosystem | the manifest file name; `packages[].ecosystem` naming an entry of `ecosystems` | by file name |
| Declared ecosystems | `ecosystems.<name>.{manifest, lockfile, artifact, registry}`, each naming steps | none |
| Block override | `packages[].{lockfile, publish}`, naming a step | the ecosystem's block |
| Steps | `steps.<name>.{task or run, writes, check}` | none |
| Where steps run | `at.{version, verify, released}` for every package, `packages[].at.{…}` for one, each a list of step names | none |
| Registry | `packages[].registry`, naming an entry of `registries` | `crates-io` for cargo, `npm` for bun |
| Registry URL, auth, provenance | `registries.<name>.{url, auth, provenance}` | built in: `crates-io` and `npm` with `auth: token` |
| Changelog | `packages[].changelog` | `CHANGELOG.md` beside the manifest |
| First release policy | `packages[].first_minor` | `false` |
| Native binaries | `packages[].assets.<target>`, naming a runner | none |
| Machines | `runners.<name>.labels` | built-in hosted runners |
| Validation jobs | `ci.<job>.runners` | one `verify` on `ubuntu-latest` |
| Tag shape | `tags` | `v{version}` |
| Version PR text | `proposal.title`, `proposal.commit_message` | `chore(release): version packages`, the title |
| Served files | `templates.source`, `templates.suffix` | `.ctl/templates`, `.jinja` |

Removed, each with what replaces it:

| Removed | Replaced by |
|:--|:--|
| `drivers`, `packages[].driver`, `format`, `keys`, `read`, `write` | the ecosystem's Manifest block; a declared ecosystem for other stacks |
| `drivers.*.after`, `packages[].after` | the Lockfile block |
| `publishers`, `packages[].publisher`, `noun`, `publish` argv | the Registry block; `packages[].publish` naming a step |
| `packages[].registry` as alias or URL, `config_name` | `registries` named once |
| `prepare.after`, `prepare.stage`, `prepare.stage_ignored` | `version` steps with `writes` |
| `prepare.version_label` | Version PR identity (2.1) |
| `pins`, `patterns` | templates |
| `tags.template` | `tags` |
| top-level `assets` | `packages[].assets` |

Templates receive the versions map, a `tags` map from package name to its tag,
and the verctl surface. They render through `ctl_core::served` (ctl-core
0.6.11 or later), so verctl and its tests use one renderer. verctl keeps discovery, the
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

A Python package through a declared ecosystem, with a step that writes a
version file:

```yaml
ecosystems:
  uv:
    manifest: {read: read-version, write: write-version}
    lockfile: uv-lock
    registry: {published: pypi-has, publish: pypi-publish}
steps:
  read-version: {task: "version:read"}
  write-version: {task: "version:write", writes: [pyproject.toml]}
  uv-lock: {run: [uv, lock], writes: [uv.lock]}
  pypi-has: {task: "pypi:has"}
  pypi-publish: {task: "pypi:publish"}
  version-file: {task: "version:file", writes: [src/thing/_version.py]}
packages:
  - name: thing
    path: pyproject.toml
    ecosystem: uv
    at: {version: [version-file]}
```

## 6. Commands

| Command | Does | Replaces |
|:--|:--|:--|
| `status` | both axes per package, the next transition, and the versioned plan (`--format json`, `--github-output`) | `status`, `assets --github-output` |
| `check` | identity and content (2.2), lockfile agreement, served files | `check`, `check --versions` |
| `version` | the version step on the working tree; reports what ran under `after` | `prepare` |
| `propose` | the version step on the release branch tip, then open, update, or close the Version PR | `prepare --pr` |
| `release` | reconcile the oldest unreleased release commit: verify, publish, tag, release, finalize | `publish` |
| `release asset <target>` | build and upload one asset | `assets --build --upload` |
| `release bootstrap <package>` | the operator-run first publication and trust setup, from bun-release | bun-release's bootstrap |
| `render` | re-render every served file at the versions on HEAD | `pin` |
| `ci` | plan the validation jobs | unchanged |
| `instructions` | print the installed contract | unchanged |

`--dry-run` on `version`, `propose`, and `release` reports the transitions and
steps it would run and changes nothing.

## 7. Workflows

Two workflows replace three.

1. `ci.yml` on `pull_request`: `verify` runs `verctl check`. No `labeled` or
   `unlabeled` types.
2. `release.yml` on push to the release branch and `workflow_dispatch`:
   1. `plan`: `verctl status --github-output` emits the release commit, the
      tags, and the asset matrix.
   2. `publish`: when a release is pending, check out the release commit and
      run `verctl release`.
   3. `assets`: one job per target; each checks out the release commit, runs
      `verctl release asset <target>`, and attests the archive.
   4. `finalize`: `verctl release` again, which publishes the draft and runs
      the `released` steps.
   5. `propose`: `verctl propose`, with a token whose pull request events start
      CI without approval.

A pull request opened with `GITHUB_TOKEN` has its CI runs held for approval,
so `propose` needs a GitHub App token. VER-044 decides whether verctl gets its
own App.

## 8. Decisions

Each decision names the alternatives and why they lost.

1. **State is derived from facts, every run.** Alternatives: a versions
   manifest (release-please), or a committed ledger of consumed fragments
   (pnpm 12). Lost: a partial failure leaves a recorded state wrong, the facts
   are cheap to read, and in a Version PR the fragment deletion already is the
   record that `check` reads.
2. **The release commit comes from first-parent history.** Alternatives: HEAD,
   or "HEAD changed the version" (bun-release). Both lose a release once
   another commit lands. A merge-commit trailer lost because squash, merge, and
   rebase merges treat trailers differently.
3. **Version PR identity is head repository, head branch, and base; content
   is verified too.** See 2.2 for the four alternatives.
4. **One Version PR per base, on `verctl/release/<base>`.** See 2.3.
5. **Push to the release branch triggers release, then propose.** Alternative:
   publish on `pull_request: closed` from the proposal branch. Lost: that event
   happens once, so a failed run needs a manual dispatch on the right commit.
6. **Built-ins are providers of building blocks; extensions are steps run as
   mise tasks or argv.** See 3.3 for the alternatives.
7. **No lockfile is written by verctl.** Each lockfile is written by its owner:
   `cargo update --workspace`, `bun install --lockfile-only`, `mise lock`, or a
   declared lockfile step. verctl only parses lockfiles to check them.
   Alternative: edit `Cargo.lock` or `bun.lock` as text (knope,
   release-please). Lost: wrong when two versions of a crate are in the graph,
   and the shape is the owner's to define. Override: a declared lockfile step.
   Disable path: do not track the lockfile.
8. **Dependents are bumped from the workspace graph.** Override: a fragment may
   bump a dependent further. Disable path: none, because a path dependency
   whose requirement excludes the new version does not build.
9. **The registry decides "published".** Alternatives: stderr text, or
   `--tolerate-republish` alone. Lost: text changes between registries, and
   the flag needs auth and cannot plan.
10. **Publish one package at a time in dependency order.** Alternative:
    `cargo publish --workspace`. Lost: it refuses to run when any selected
    version exists, so it cannot resume.
11. **A release with assets is a draft until every asset is uploaded.**
    Disable path: declare no assets.
12. **Bun packs; verctl talks to the registry.** See 3.5 for the three
    options.
13. **mise installs, verifies, and runs; verctl versions and releases.** See
    section 4.
14. **Templates are the only served-file mechanism.** Agreed by the operator.
15. **Commands are renamed with no aliases.** Agreed by the operator.
16. **`first_minor` stays.** It is an operator policy written in the file the
    pull request shows. It applies only to a package at `0.0.0`.
17. **One minor, then patches.** See section 11.

## 9. Which steps were a symptom

Before this design, steps existed only as `prepare.after`, and the six known
uses all refreshed `Cargo.lock`. The Lockfile block now does that, so those six
repositories declare no step at all. Steps remain for what no built-in can
know: generated version files, smoke tests before publish, announcements after
a release, and every block of a stack verctl does not ship.

## 10. Workarounds removed

| Id | Workaround | Removed by |
|:--|:--|:--|
| W1 | `prepare.after: [cargo, update, --workspace]` plus `stage: [Cargo.lock]` in six repositories, and the Rust toolchain each consumer's Version PR job installs for it | the cargo Lockfile block (3.4); the six sections are deleted |
| W2 | driver and package `after`, printed in a `next` row | removed; the Lockfile block runs and is reported under `after` |
| W3 | dead `drivers.toml` | deleted |
| W4 | `prepare.stage_ignored` | a step's `writes` names exactly what it may change (3.3) |
| W5 | the label race and the `labeled`/`unlabeled` CI trigger (VER-029) | Version PR identity (2.1); the trigger types go |
| W6 | the guard exemption on the `version-packages` branch | identity plus content (2.2) |
| W7 | `head.ref == 'version-packages'` in every publish workflow | push-to-branch release (7) |
| W8 | a Version PR never closes; a fragment stays on disk when no pending fragment gives any of its packages a real bump | `propose` closes; the version step consumes every pending fragment, whatever its bumps |
| W9 | `VERCTL_DEFAULT_BRANCH`, the default-branch fetch, five default-branch inputs | the `branch` key; the release commit is derived, not proved |
| W10 | a rerun can tag a commit that was never published | release commit from history (1.1); tag after publish (1.3) |
| W11 | "already published" from error text; `--tolerate-republish` reported as a fresh publish | registry query before publish (decision 9) |
| W12 | publish order by declaration order | dependency graph (3.4, 3.5) |
| W13 | `mise.release.toml` hand-bumped bootstrap pin; consumers pin verctl twice | this repository deletes the file, which no lane loads; a repository that pins its own tool moves it with the built-in `mise-pin` step (4); consumers pin one action ref once VER-043 releases the action with the verctl it runs |
| W14 | pins and templates for one concern | templates only |
| W15 | `?ref=v` hard-coded in the pin regex and a template | the `tags` map in the template context |
| W16 | operator docs rendered on a second, test-only path | `ctl_core::served` renders every served file; `render` updates them and `check` compares them |
| W17 | the `#USAGE mount` line copied into a template and pinned by a test and a doc rule | the template reads it from the context |
| W18 | `first_minor` | kept as policy (decision 16) |
| W19 | `VERCTL_PR_TITLE`, `VERCTL_COMMIT_MESSAGE` | `proposal.title`, `proposal.commit_message` |
| W20 | `CACHE_WRITE`, because publish runs on a pull request event | release runs on the release branch |
| W21 | an App token in four repositories while the docs say "not a GitHub App" | the docs state the token `propose` needs (7); VER-044 decides the App |
| W22 | the CI plan job compiles verctl for a verb the release already has | kept by design in this repository, which dogfoods HEAD; consumers use the released action; VER-016 closes as stale |
| W23 | assets take the first package's version and tag | `packages[].assets` |
| W24 | example workflows maintained by hand | served from templates, so `check` keeps them current |
| W25 | documented features that do not exist: `internalAuthors`, changelog links, overridable changelog templates, OIDC, pre-release bootstrap text, served-file checks | each is built here (OIDC, served-file checks) or deleted from the docs (`internalAuthors`, links, template override, bootstrap text) |
| W26 | unused code: `run_limited`, `Skip::Ci`, `noun` | deleted |
| W27 | schema rules the code does not keep: no `deny_unknown_fields` below the top level, silent pin skips, lazy runner names, `version_label` treating `None` and `""` alike | every struct refuses unknown keys; names resolve at load; `version_label` is gone |

Ledger rows that propose more of the same close when the work that removes
their cause lands: VER-013, VER-016, VER-020, VER-028, VER-029, VER-030,
VER-031, VER-033, VER-034, VER-035, VER-036, VER-038, and VER-010.

## 11. Migration: one minor, then patches

In 0.x, a minor is where breaking changes go. Every breaking change therefore
ships in one release, 0.4.0, and everything after it is a patch that adds
without breaking.

**What 0.4.0 holds:** every interface a consumer sees. The final
`.ctl/ver.yaml` schema and every removal, the command set, the release state
machine, identity and content in `check`, the proposal branch name, the two
workflows, and the report fields. Only values that work ship: `auth: token` is
the one registry auth value in 0.4.0.

**What patches add after it:** implementations behind those interfaces and new
opt-in values. Examples: `auth: oidc`, `provenance: true`, staged publish,
`release bootstrap`, further built-in step providers, and the shared Rust
baseline from ctl-core as a second source of served files, written by `render`
and checked by `check` (VER-045). Adding an enum value, an optional key, or a
served-file source does not break a valid config.

How the work lands without five releases:

| Option | Gain | Cost | Verdict |
|:--|:--|:--|:--|
| An integration branch `next`; each part is a reviewed PR into `next`; one PR merges `next` into `main` | `main` stays releasable for an urgent 0.3.x patch the whole time | a long-lived branch to keep current with `main`; one large final merge, though each part was reviewed | **chosen** |
| Parts land on `main`; the Version PR is held open until the last part | no long-lived branch | `main` is half-migrated, and this repository releases itself from `main`; an urgent 0.3.x patch would need a second release base, which is a non-goal | rejected |
| Old and new paths side by side behind a switch | each part ships alone | a compatibility layer, which the project rules forbid | rejected |

The five consumers change once, in the window when 0.4.0 releases: qctl,
forkctl, ctl-core, unixd, and yamled delete their `prepare` section and move
to the two workflows; qctl and forkctl move four pinned files to templates.

Rollback: revert the merge of `next` and release a patch; consumers restore
their previous config from git.

## 12. Risks

1. [The five consumers change in one window] → The PR for each consumer is
   prepared against the `next` build before 0.4.0 releases, and a removed key
   fails with its replacement.
2. [The release commit walk reads history] → Workflows already fetch full
   history; the walk stops at the last tagged version of each package.
3. [Bun below 1.4.1 leaves the lockfile stale] → The bun ecosystem refuses an
   older Bun by version.
4. [verctl maintains an npm registry client] → It follows the documented wire
   format that npm, pnpm 12, and Bun all send, and it ships behind `auth` and
   `provenance` values that patches add.
5. [`mise lock` can exit 0 with platforms missing] → The `mise-pin` step reads
   `mise.lock` back and requires a URL and checksum for every listed platform.
6. [A pull request that adds and applies a fragment in one diff now fails] →
   Intended: fragments land with the change that needs them, and the Version
   PR applies what is on the release branch.

[gh-pr-update]: https://docs.github.com/en/rest/pulls/pulls#update-a-pull-request
