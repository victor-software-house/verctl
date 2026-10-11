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

Queue row: VER-011. This change replaces the A/B/C plan held on
`feat/011-refresh-lockfile` and folds in its two approved intents: the version
step refreshes lockfiles itself, and the report field `next` becomes `after`.

## What Changes

1. A release is derived from facts on every run: fragments, manifests, the
   release commit in default-branch history, lockfile agreement, the registry,
   the tag, the GitHub Release, and its assets. No state file, label, or event
   payload.
2. Every transition is guarded and repeatable: `propose`, `publish`, `tag`,
   `release`, `asset`, `finalize`. Publish precedes tag; tags land on the
   release commit in one atomic push; a release with assets is a draft until
   they are all uploaded.
3. Cargo and Bun are native ecosystems: version, dependents, lockfile refresh
   and check, publish order, registry query, publish, and OIDC auth. Anything
   else uses an explicit `command` ecosystem.
4. `check` verifies a Version PR by its content: version changes must equal the
   plan of the fragments it consumes, lockfiles must agree, served files must
   match their templates.
5. **BREAKING:** commands become `status`, `check`, `version`, `propose`,
   `release`, `release asset`, `render`, `ci`, and `instructions`. `prepare`,
   `publish`, `pin`, `assets`, and `check --versions` go.
6. **BREAKING:** `.ctl/ver.yaml` keeps one place per concern and refuses
   unknown keys at every level. `drivers`, `publishers`, `prepare`, `pins`,
   `patterns`, and the per-package driver and publisher keys go; `branch`,
   `registries`, `proposal`, `tags`, and `packages[].assets` replace them.
7. **BREAKING:** two workflows replace three. Release runs on push to the
   default branch, then `propose`.
8. Every workaround in design section 7 is removed, and 13 ledger rows close as
   their cause goes.

## Capabilities

### New Capabilities

- `release-state`: the facts, the release commit, the two stage axes, the
  transitions with their guards and postconditions, and how a rerun resumes.
- `version-proposal`: what the version step writes, the Version PR lifecycle,
  and content verification in `check`.
- `ecosystems`: the cargo, bun, and command contracts per concern.
- `release-config`: one place per concern in `.ctl/ver.yaml`, refusal of
  unknown and removed keys, and the defaults.

### Modified Capabilities

None. [`versioning`](../../specs/versioning/spec.md) keeps its first-release
rule.

## Decisions for the operator

Each has a recommendation; design section 6 gives the alternative that lost.

1. Derive state from facts on every run, with no state file or labels.
   Recommended.
2. Trigger release on push to the default branch, then propose, instead of on
   the Version PR's `closed` event. Recommended.
3. Verify the Version PR by content and drop the label and branch exemption.
   Recommended.
4. Make Cargo and Bun native and put every other stack behind an explicit
   `command` ecosystem, removing `drivers` and `publishers`. Recommended.
5. Let the package manager write lockfiles, and have `check` parse them.
   Recommended over text edits.
6. Have verctl perform the OIDC exchange for crates.io and npm. Recommended,
   because Bun cannot.
7. Make templates the only served-file mechanism and remove `pins`. qctl and
   forkctl move four files. Recommended.
8. Remove hooks (`prepare.after`, `stage`, `stage_ignored`) entirely.
   Recommended.
9. Rename the commands as in What Changes item 5, with no aliases. Recommended.
10. Ship in five breaking minor releases, 0.4.0 to 0.8.0, updating the five
    consumers in each, with no compatibility layer. Recommended.

## Impact

1. Code: most of `src/`. `templates.rs` moves onto `ctl_core::served` in the
   served-files phase, agreed with the session that owns CTC-008.
2. Consumers: qctl, forkctl, ctl-core, unixd, and yamled change config and
   workflows in each phase. bun-release's publish path folds into the bun
   ecosystem in the auth phase.
3. Actions: the three actions shrink to what the two workflows call. VER-043
   moves them to their own repositories.
4. Identity: `propose` needs a token whose pull request events start CI without
   approval. VER-044 decides whether that is a dedicated GitHub App.
5. Docs: README, the instructions, the operator skill, AGENTS.md, and
   `docs/served-files.md` are rewritten per phase.
