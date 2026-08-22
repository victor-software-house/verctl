# What a monorepo consumer asked for

A workspace of twelve independently-versioned packages read verctl at v0.1.1,
measured it against the release tooling they already had, and wrote up what
stood between them and deleting it. This is that report, kept because the rows
it produced cite it: **VER-032** through **VER-039**.

The reporter is not named here and their internal details are not reproduced.
What survives is the argument, because the argument is what the rows are for.

## The shape of the consumer

Worth stating, because most of the asks only appear at this shape and none of
them appear in a single-package repo:

- Twelve publishable packages, **independently versioned**, one scope, a
  restricted registry.
- Internal dependencies are workspace protocol, rewritten at publish time.
- Most releases bump two or more packages. One version per release is not their
  edge case, it is their normal case.

Their read of what already fits: the fragment format is identical, so no
migration; the changelog templates render the same fields in the same branch
order as theirs, including the internal-author filtering; the bun publisher is
their exact invocation, and discovering `bunfig.toml` rather than requiring
`.npmrc` matters to them because their repo forbids `.npmrc` outright — bun
honours it first and silently discards a committed scope-token contract.

## 1. A release must tag every artifact it shipped

The blocking one. `publish::tag_for` dedups the versions of the released
packages; when more than one survives it returns the **first declared**
package's version. That value has no relationship to the release, and nothing
says so.

At one distinct version this is exactly right, which is why it has never bitten
this repo. At two it tags one package and leaves consumers of the other looking
for a tag that does not exist.

### How they do it, which is the interesting part

Their tags and their Releases are two separate steps, never interleaved.

**Tags first, git-side.** One annotated tag per released package at an explicit
SHA — `git tag -a <name>@<version> <sha>` — then a single
`git push --atomic origin refs/tags/a:refs/tags/a …` for the whole set. Three
rules guard it: any expected tag already at another commit refuses, a *partial*
set refuses rather than being completed, and a full set at the right SHA is
skipped, so a rerun is a no-op. Then every tag is re-read with
`git ls-remote --tags origin refs/tags/<t> refs/tags/<t>^{}`, taking the peeled
form first so an annotated tag reports its commit rather than the tag object.
A push that exited zero is not accepted as evidence it landed.

**Releases after**, one per tag that already exists, each body sliced out of that
package's own changelog between its version heading and the next.

That ordering matters more than it looks. Because the tag exists first, the
Releases API's `target_commitish` is documented as unused, so the question of
where a Release puts a tag never arises. The atomicity worth having is over the
**tags**, not the Releases — a Release is derived state that can be recreated
from a tag and a changelog section; a tag is what consumers resolve.

Filed as **VER-039** (the shape) and **VER-038** (creating a set whole).
VER-039 ships `tags.template` (default `v{version}`; `{name}` for one tag and
one Release per package) and fills each Release body from that package's own
CHANGELOG section. The silent first-package guess is gone: a template without
`{name}` refuses when versions differ. Git-side atomic tag push stays with
VER-038.

## 2. Publish must follow the dependency graph

`publish::plan` iterates the declared packages in declaration order. With
internal dependencies that leaves a window where a consumer is published and its
dependency is not, so the consumer's pinned version does not resolve.

Their ask includes printing the resolved order. Their words: a silent
topological sort is a thing nobody debugs until it is wrong.

Filed as **VER-033**.

## 3. Publish needs its own before and after

They keep two checks they are **not** asking verctl to absorb, and both are
worth describing because they say what a release tool cannot see from the
inside.

**Before publish**, on the release commit, for every package: pack it, read the
manifest back out of the tarball, and assert no dependency field still names the
workspace protocol. That one proves the package manager actually rewrote
internal deps into registry versions. A tarball that ships them installs as
garbage for every consumer and nothing else in a pipeline notices. They then
build a throwaway consumer against the local tarballs and import every specifier
in the packed `exports` map, which catches export maps and type declarations
that only resolve inside the workspace.

**After publish**: install each published version from the registry into a clean
cache and assert the version served is the one just pushed. This has caught
propagation delay and a registry that accepted a publish and served the previous
version. Treating "already exists" as success is right for idempotency, and it
means a retry can pass without anything downstream having been verified.

Neither belongs in verctl — they are stack-specific. What they cannot do is wire
them, because `prepare` has `after` and publish has no equivalent.

Filed as **VER-034**.

## 4. The commit a release publishes from should be proved

They keep both of verctl's checks and add three: the release SHA is an ancestor
of the default branch, the workflow files at that SHA do not differ from the
default branch, and the first-parent branch carries exactly one version commit.

The workflow-drift one is not obvious and it cost them a release. Retrying an
old release SHA runs the workflow **at that SHA**. If permissions or steps
changed since, the run can publish to the registry and then fail to create tags
— the worst split available, because the registry is not revertible and the tags
are the audit trail.

Two of the three have explicit waivers, and a waived run prints that it was
waived. Their warning about building that: bind a dispatch input to a
step-level `env:` and quote it, never interpolate it into a `run:` string.
Dispatch inputs arrive as strings over REST and are not coerced to booleans, in
a job holding write permissions.

Filed as **VER-035**.

## 5. A check that cannot run must not pass

Two sites, one law. `publisher::expand` drops any argv part containing an
unresolved placeholder, so a typo shortens the command line instead of failing
and the error surfaces downstream wearing someone else's name.
`prove_default_history` returns success on four conditions — no repo, no head,
unpeelable head, no upstream — each defensible alone, but together the check can
pass by not running, in exactly the CI shapes where it matters most.

Filed as **VER-036**.

## 6. Questions the README does not answer

Four, each of which cost them time:

1. What `--access` does once a registry resolves, and whether a restricted
   package can be published as public by accident.
2. What the token means locally, where a `gh` session is not it — the failure
   lands at the Release step, *after* publishing.
3. That a `permission_denied` on write is a package-settings problem rather than
   a token problem. For GitHub Packages, write permission on the workflow is
   necessary but not sufficient for an existing package; the package must be
   linked to the publishing repository, and there is no public API for that
   association.
4. That the tool finalizing a release is the one the workflow pins, not the one
   at the release commit. Their publish job checks out the release commit, so
   the code that tags afterwards comes from that checkout too, and a fix to the
   tagging logic that landed since is absent on a retry. Build and publish from
   the release commit; finalize with the current tooling.

Filed as **VER-037**.

## What they praised, so it does not get refactored away

`examples/ver.yaml` being fully live and parsed by `tests/examples.rs` is, in
their words, the best thing in the documentation, and the reason they could
answer most of their own questions by reading one file.

The `occurrences` vocabulary — `once`, `many`, `never`, `{exactly: N}` — is
better than what they would have built.

The split where verctl decides how many jobs and the workflow decides where a
fixed job runs is a good line, and passing labels through verbatim is what let
them keep their own runner labels.
