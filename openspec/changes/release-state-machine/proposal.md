# Make the release an explicit state machine

## Why

verctl has no model of a release. Each command rebuilds the state it needs
from indirect signals, and each gap has grown a workaround. An inventory of
this repository and its five consumers found 27 of them, and 13 ledger rows
that propose more of the same. Measured examples:

1. Six of six repositories restate the same lockfile refresh by hand, because
   verctl works it out and only prints it.
2. The Version PR's own CI failed the hand-edit guard on the v0.1.1 release,
   because the guard reads a label from an event snapshot taken before the label
   existed (VER-029). The fix was a second CI trigger.
3. A rerun of `publish` after the default branch moves can create the tag on a
   commit that was never published, because "this commit is a release" is a
   CHANGELOG heading plus ancestry, true for every later commit.
4. "Already published" is decided by matching six substrings of registry
   stderr.
5. The stack is guessed from the manifest file name in four places, the tag
   shape is set in three, and the default branch comes from five inputs.

The prior art in [research.md](research.md) shows the same failure in the most
used tools: each marks a release done from one signal (a label, a tag, a
registry entry) and loses a step after a partial failure. Changesets also
publishes Bun workspaces through npm, which ships `workspace:` ranges raw, and
Bun's own lockfile, not package.json, decides the dependency versions it
publishes.

Queue row: VER-011. This change replaces the plan held on
`feat/011-refresh-lockfile` and folds in its two approved intents: the version
step refreshes lockfiles itself, and the report field `next` becomes `after`.

## What Changes

1. A release is derived from facts on every run: fragments, manifests, the
   release commit in branch history, lockfile agreement, the registry, the tag,
   the GitHub Release, and its assets. No state file, label, or event payload.
2. Every transition is guarded and repeatable: `propose`, `publish`, `tag`,
   `release`, `asset`, `finalize`. Publish precedes tag for a package with
   an unchecked upload, and a registry-checked package is tagged first
   ([tag-at-merge](../archive/2026-10-24-tag-at-merge/proposal.md)). Tags land on the
   release commit in one atomic push. A release with assets is a draft until
   they are all uploaded.
3. A Version PR is identified by its head repository and its head branch
   `verctl/release/<base>`, which a pull request cannot change, and by its
   base, read live on every run. `check` also verifies that its diff is the
   version step and nothing else. One Version PR per base.
4. A release is built from blocks (Manifest, Lockfile, Artifact, Registry,
   Forge, Served files, Binary asset, Step). Cargo and Bun are built-in
   providers. Any package can replace a block with a step, and any stack can
   declare a whole ecosystem from steps.
5. A step is a mise task or an argv with declared `writes` and, after a
   release, a `check`. Steps run at `version`, `verify`, `publish`, and
   `released`. They replace `prepare.after` and its `stage` globs.
6. mise installs and verifies verctl, runs every step, and owns `mise.lock`.
   No lockfile is ever written by verctl; each is written by its owner and read
   by verctl to check it.
7. bun-release becomes Rust inside verctl: Bun packs, and verctl's own npm
   registry client queries, publishes, exchanges OIDC tokens, and handles
   staging, trust, and provenance.
8. **BREAKING:** commands become `status`, `check`, `version`, `propose`,
   `release`, `release asset`, `release bootstrap`, `render`, `ci`, and
   `instructions`. Not started; `docs` and `semver` shipped since and need a
   place in this list ([audit](audit.md)).
9. **BREAKING:** `.ctl/ver.yaml` keeps one place per concern and refuses
   unknown keys at every level. `drivers`, `publishers`, `prepare`, `pins`,
   `patterns`, and the per-package driver and publisher keys go; `branch`,
   `registries`, `ecosystems`, `steps`, `at`, `proposal`, `tags`, and
   `packages[].assets` replace them. Partly shipped: unknown keys are refused;
   every removed key is still read ([audit](audit.md)).
10. **BREAKING:** two workflows replace three, and the proposal branch becomes
    `verctl/release/<base>`. Partly shipped: the branch and one reusable
    `release.yml` are on main; the three actions remain ([audit](audit.md)).
11. All breaking changes ship in one minor, 0.8.0, held with the rest of the
    minor backlog; later work ships as patches that add without breaking.
    0.4.0 to 0.7.4 shipped before this change, and the parts of it they
    carried are listed in [audit.md](audit.md).

## Capabilities

### New Capabilities

- `release-state`: the facts, the release commit, the two stage axes, the
  transitions with their guards and postconditions, and how a rerun resumes.
- `version-proposal`: what the version step writes, Version PR identity, its
  lifecycle, and `check`.
- `building-blocks`: the blocks, the built-in cargo and bun providers, declared
  ecosystems, steps, lockfile ownership, and mise's part.
- `release-config`: one place per concern in `.ctl/ver.yaml`, refusal of
  unknown and removed keys, and the defaults.

### Modified Capabilities

None. [`versioning`](../../specs/versioning/spec.md) keeps its first-release
rule.

## Decisions for the operator

Agreed already: templates only (design decision 14) and command renames
(design decision 15). Each decision has a recommendation; the design gives
the alternatives and trade-offs. The operator approved all eight as
recommended on 2026-10-12 (VER-011 note) and confirms the 0.8.0 scope after
the [audit](audit.md). The note after each decision says what shipped code
settled.

1. Derive state from facts on every run, with no state file or labels.
   Partly settled: `publish` derives each version commit from first-parent
   history, and no label is read.
2. Release on push to the release branch, then propose. Settled: the
   reusable `release.yml` publishes, then opens the Version PR, on push.
3. Identify the Version PR by head repository, head branch, and base, and
   verify its content too (design 2.2). Identity settled by [#188][pr188]; the content
   check is not built.
4. One Version PR per base on `verctl/release/<base>`; no per-package PRs and
   no version in the branch name (design 2.3). Settled by [#161][pr161].
5. Built-ins as providers of building blocks; extensions as steps run through
   mise tasks or argv; declared ecosystems for other stacks (design 3). Not
   settled by code: drivers and publishers remain.
6. mise installs, verifies, runs steps, and owns `mise.lock`; verctl writes no
   lockfile (design 4). Partly settled: verctl writes no lockfile today.
7. bun-release in Rust as option (b): Bun packs, verctl owns the registry
   protocol (design 3.5). Not settled by code.
8. Ship 0.4.0 from an integration branch `next`, then patches (design 11).
   Contradicted: 0.4.0 to 0.7.4 shipped from `main`, no `next` exists, and
   the 0.8.0 minor is held on a stack instead.

## Impact

1. Code: most of `src/`. `templates.rs` moves onto `ctl_core::served`, agreed
   with the session that owns CTC-008.
2. Consumers: qctl, forkctl, ctl-core, unixd, and yamled change config and
   workflows once, in the 0.8.0 window. bun-release retires once the bun
   ecosystem publishes it.
3. Actions: the three actions shrink to what the two workflows call. VER-043
   moves them to their own repositories.
4. Identity: `propose` needs a token whose pull request events start CI without
   approval. VER-044 decides whether that is a dedicated GitHub App.
5. Docs: README, the instructions, the operator skill, AGENTS.md, and
   `docs/served-files.md` are rewritten for 0.8.0.

[pr161]: https://github.com/victor-software-house/verctl/pull/161
[pr188]: https://github.com/victor-software-house/verctl/pull/188
