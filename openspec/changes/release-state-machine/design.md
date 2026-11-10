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

Section 3 lists all 27 workarounds found, with the ledger rows that propose
more of the same.

## Parts

This change holds the goals, the configuration, the decisions, the
workarounds, and the migration that every part shares. Each part is its own
change with its own proposal:

1. [derived-release-state][drs]: the facts, the stages, the transitions, the
   commands, and the workflows.
2. [release-completion][rc]: a release that does not publish, drafts, and a
   first publication.
3. [`version-pr-identity`][vpi]: what identifies a Version PR, and the proposal
   branch.
4. [version-pr-content][vpc]: what a Version PR may change.
5. [building-blocks][bb]: the blocks, providers, steps, and the dependency
   graph.
6. [built-in-ecosystems][bie]: cargo, bun and bun-release in Rust, and mise.

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
6. Every workaround in section 3 is removed, and the ledger rows that proposed
   more of them close.

**Non-Goals:**

1. Pre-release mode. The model leaves room for it as a state; it is a later
   change.
2. More than one release base branch. The proposal branch name carries the
   base, so a second base is additive later.
3. Built-in ecosystems beyond Cargo and Bun. Other stacks use extensions.
4. Moving the actions out of this repository (VER-043) and the GitHub App
   decision (VER-044). The design states what each needs.

## 1. Configuration

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
| `prepare.version_label` | Version PR identity ([`version-pr-identity`][vpi]) |
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

## 2. Decisions

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
   is verified too.** See [`version-pr-identity`][vpi] for the four alternatives.
4. **One Version PR per base, on `verctl/release/<base>`.** See [`version-pr-identity`][vpi].
5. **Push to the release branch triggers release, then propose.** Alternative:
   publish on `pull_request: closed` from the proposal branch. Lost: that event
   happens once, so a failed run needs a manual dispatch on the right commit.
6. **Built-ins are providers of building blocks; extensions are steps run as
   mise tasks or argv.** See [building-blocks][bb] for the alternatives.
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
12. **Bun packs; verctl talks to the registry.** See [built-in-ecosystems][bie] for the three
    options.
13. **mise installs, verifies, and runs; verctl versions and releases.** See
    [built-in-ecosystems][bie].
14. **Templates are the only served-file mechanism.** Agreed by the operator.
15. **Commands are renamed with no aliases.** Agreed by the operator.
16. **`first_minor` stays.** It is an operator policy written in the file the
    pull request shows. It applies only to a package at `0.0.0`.
17. **One minor, then patches.** See section 4.

## 3. Workarounds removed

| Id | Workaround | Removed by |
|:--|:--|:--|
| W1 | `prepare.after: [cargo, update, --workspace]` plus `stage: [Cargo.lock]` in six repositories, and the Rust toolchain each consumer's Version PR job installs for it | the cargo Lockfile block ([built-in-ecosystems][bie]); the six sections are deleted |
| W2 | driver and package `after`, printed in a `next` row | removed; the Lockfile block runs and is reported under `after` |
| W3 | dead `drivers.toml` | deleted |
| W4 | `prepare.stage_ignored` | a step's `writes` names exactly what it may change ([building-blocks][bb]) |
| W5 | the label race and the `labeled`/`unlabeled` CI trigger (VER-029) | Version PR identity ([`version-pr-identity`][vpi]); the trigger types go |
| W6 | the guard exemption on the `version-packages` branch | identity plus content ([version-pr-content][vpc]) |
| W7 | `head.ref == 'version-packages'` in every publish workflow | push-to-branch release ([derived-release-state][drs]) |
| W8 | a Version PR never closes; a fragment stays on disk when no pending fragment gives any of its packages a real bump | `propose` closes; the version step consumes every pending fragment, whatever its bumps |
| W9 | `VERCTL_DEFAULT_BRANCH`, the default-branch fetch, five default-branch inputs | the `branch` key; the release commit is derived, not proved |
| W10 | a rerun can tag a commit that was never published | release commit from history and tag after publish ([derived-release-state][drs]) |
| W11 | "already published" from error text; `--tolerate-republish` reported as a fresh publish | registry query before publish (decision 9) |
| W12 | publish order by declaration order | dependency graph ([building-blocks][bb]) |
| W13 | `mise.release.toml` hand-bumped bootstrap pin; consumers pin verctl twice | this repository deletes the file, which no lane loads. A repository that pins its own tool moves it with the built-in `mise-pin` step ([built-in-ecosystems][bie]). Consumers pin one action ref once VER-043 releases the action with the verctl it runs |
| W14 | pins and templates for one concern | templates only |
| W15 | `?ref=v` hard-coded in the pin regex and a template | the `tags` map in the template context |
| W16 | operator docs rendered on a second, test-only path | `ctl_core::served` renders every served file; `render` updates them and `check` compares them |
| W17 | the `#USAGE mount` line copied into a template and pinned by a test and a doc rule | the template reads it from the context |
| W18 | `first_minor` | kept as policy (decision 16) |
| W19 | `VERCTL_PR_TITLE`, `VERCTL_COMMIT_MESSAGE` | `proposal.title`, `proposal.commit_message` |
| W20 | `CACHE_WRITE`, because publish runs on a pull request event | release runs on the release branch |
| W21 | an App token in four repositories while the docs say "not a GitHub App" | the docs state the token `propose` needs ([derived-release-state][drs]); VER-044 decides the App |
| W22 | the CI plan job compiles verctl for a verb the release already has | kept by design in this repository, which dogfoods HEAD; consumers use the released action; VER-016 closes as stale |
| W23 | assets take the first package's version and tag | `packages[].assets` |
| W24 | example workflows maintained by hand | served from templates, so `check` keeps them current |
| W25 | documented features that do not exist: `internalAuthors`, changelog links, overridable changelog templates, OIDC, pre-release bootstrap text, served-file checks | each is built here (OIDC, served-file checks) or deleted from the docs (`internalAuthors`, links, template override, bootstrap text) |
| W26 | unused code: `run_limited`, `Skip::Ci`, `noun` | deleted |
| W27 | schema rules the code does not keep: no `deny_unknown_fields` below the top level, silent pin skips, lazy runner names, `version_label` treating `None` and `""` alike | every struct refuses unknown keys; names resolve at load; `version_label` is gone |

Ledger rows that propose more of the same close when the work that removes
their cause lands. They are VER-013, VER-016, VER-020, VER-028, VER-029,
VER-030, VER-031, VER-033, VER-034, VER-035, VER-036, VER-038, and VER-010.

## 4. Migration: one minor, then patches

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
`release bootstrap`, and further built-in step providers. Adding an enum value
or an optional key does not break a valid config.

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

## 5. Risks

1. [The five consumers change in one window] → The PR for each consumer is
   prepared against the `next` build before 0.4.0 releases, and a removed key
   fails with its replacement.

[drs]: ../derived-release-state/design.md
[rc]: ../release-completion/design.md
[vpi]: ../version-pr-identity/design.md
[vpc]: ../version-pr-content/design.md
[bb]: ../building-blocks/design.md
[bie]: ../built-in-ecosystems/design.md
