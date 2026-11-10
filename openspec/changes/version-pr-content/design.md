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
   writes (manifests, lockfiles, CHANGELOGs, served files, consumed fragments,
   and the declared `writes` of `version` steps, read from the configuration
   at the merge-base). A manifest differs only in version fields and in the
   dependency requirements the plan rewrites. Every changed version equals the
   plan of the fragments present at the merge-base and absent at HEAD. Every
   lockfile agrees with its manifests. Its parsed entries and header differ
   from the merge-base only in the versions of the workspace packages the plan
   bumps and in references that resolve to them, with no entry added or
   removed. Every served file equals its template's render. A workspace entry is told
   from a registry entry of the same name by its source: no `source` in
   `Cargo.lock`, a workspace key in `bun.lock`. If an owner command
   re-resolves more than the bump, `propose` fails its own postcondition, and
   the release branch needs a lockfile refresh first. The content of a
   `version` step's declared `writes` is trusted to that step and reviewed on
   the Version PR; `check` constrains only which paths it may change.

Identity alone trusts whoever pushes to the proposal branch. Content alone
lets any branch carry a release. Together, a hand edit fails on any branch,
and the Version PR passes on its first CI run with no label. A repository can
also protect the proposal branch with a ruleset so only the release identity
pushes to it; `check` does not verify that ruleset, and VER-044 decides that
identity and whether verctl should.

## 2. Risks

1. [A pull request that adds and applies a fragment in one diff now fails] →
   Intended: fragments land with the change that needs them, and the Version
   PR applies what is on the release branch.

[rsm]: ../release-state-machine/design.md
[vpi]: ../version-pr-identity/design.md
