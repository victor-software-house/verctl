# Design

## Context

A Version PR's diff is checked today only by a label and a branch name, so a
hand edit can ride a release. This change requires every Version PR's diff to
be the version step applied to its base, and nothing else. It is one part of
[release-state-machine][rsm]; who may open a Version PR is
[`version-pr-identity`][vpi].

## 1. Identity and content are both required

`check` applies two independent rules to every pull request:

1. **Identity.** Only a pull request that is a Version PR by [`version-pr-identity`][vpi] may change a
   manifest version or add a CHANGELOG section. Any pull request may add or
   remove a fragment. A pull request that changes whether a package publishes
   must also add a fragment for that package, so the next Version PR moves it,
   together with its dependents, to a version that follows the new setting.
2. **Content.** A Version PR's diff must be the version step applied to its
   base and nothing else. It changes no path outside what the version step
   writes (manifests, lockfiles, CHANGELOGs, pinned and served files, consumed
   fragments, and the declared `writes` of `version` steps, read from the
   configuration at the merge-base). A manifest equals its merge-base text
   with the planned version written by its driver and its pins applied. Every
   changed version equals the plan of the fragments present at the merge-base
   and absent at HEAD. Every lockfile agrees with its manifests. Its parsed
   entries and header differ from the merge-base only in the versions of
   workspace members whose version the plan moves, directly or through
   workspace inheritance, and in references that resolve to them, with no
   entry added or removed. Every served file equals its template's render. A
   workspace entry is told from a registry entry of the same name by its
   source: no `source` in `Cargo.lock`, a workspace key in `bun.lock`, a
   member path in `package-lock.json`, and an `editable` or `virtual` source
   in `uv.lock`. If an owner command
   re-resolves more than the bump, `propose` fails its own postcondition, and
   the release branch needs a lockfile refresh first. The content of a
   `version` step's declared `writes` is trusted to that step and reviewed on
   the Version PR; `check` constrains only which paths it may change.

Two rules for pull requests that are not Version PRs ship in this change:
a CHANGELOG section, and a change to whether a package publishes.
Each makes `check` stricter, so shipping them after 0.8.0 would force 0.9.0.

Identity alone trusts whoever pushes to the proposal branch. Content alone
lets any branch carry a release. Together, a hand edit fails on any branch,
and the Version PR passes on its first CI run with no label. A repository can
also protect the proposal branch with a ruleset so only the release identity
pushes to it; `check` does not verify that ruleset, and VER-044 decides that
identity and whether verctl should.

## 2. How the content check reads the diff

1. **Manifests by replay.** `check` writes the planned version into the
   merge-base text with the package's driver, applies its pins, and compares
   bytes with HEAD. The alternative, a per-format comparison that ignores
   version fields, needs a differ for every driver and still has to decide
   which requirement lines a pin may move. Replay answers both with the code
   the version step runs. On a mismatch, a TOML or JSON manifest is compared
   key by key, so the error names the key, such as an added dependency.
2. **Members by inheritance.** A lockfile entry may move when its member's
   resolved version moved and the plan explains it: the member is the
   planned package, or it inherits `workspace.package.version` from a planned
   manifest. Naming only planned packages lost: `ctl-core` moves four
   `Cargo.lock` entries for one package, and `shrub` ten.
3. **Pins are written files.** Pinned files are in the write set and must
   equal their merge-base text with the pins applied. Moving every pin to a
   template first lost: `ctl-core` and `shrub` pin requirement lines inside
   `Cargo.toml`, which a template would have to own whole.
4. **Unreadable lockfiles fail.** A changed `yarn.lock` or `bun.lockb` fails
   and names the file. Passing them unread would let a dependency move ride
   the Version PR. No repository using verctl has either today.
5. **Dependent requirements later.** Rewriting a requirement the new version
   no longer admits is VER-105. Until it ships, a requirement change that no
   pin explains fails, which VER-105 then loosens without a break.

The override for each rule is the merge-base configuration: a repository
widens the write set with `prepare.stage` and moves a line with a pin. Merging
without the check means skipping `check --versions` on that pull request.

## 3. Risks

1. [A pull request that adds and applies a fragment in one diff now fails] →
   Intended: fragments land with the change that needs them, and the Version
   PR applies what is on the release branch.

[rsm]: ../release-state-machine/design.md
[vpi]: ../version-pr-identity/design.md
