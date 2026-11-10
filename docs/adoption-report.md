# Requests from a monorepo consumer

A workspace read verctl at v0.1.1 for its twelve packages, each with its own version.  
Its maintainers measured verctl against the release tooling they already had.  
They then wrote up what stood between them and deleting that tooling.  
This file keeps their report, because the ledger rows it produced cite it: [`VER-032` through `VER-039`][ledger].

The reporter is not named here, and their internal details are not reproduced.  
What remains is the argument, which is what the rows are for.

## The shape of the consumer

This shape matters, because most of the requests appear only at this shape, and none appear in a single-package repo:

- Twelve packages to publish, each **versioned on its own**.  
  They share one scope and a restricted registry.
- Internal dependencies use the workspace protocol, which is rewritten at publish time.
- Most releases bump two or more packages.  
  A release with several versions is their normal case.

They found that these parts already fit:

- The fragment format is identical, so they need no migration.
- The changelog templates render the same fields in the same branch order as theirs, including the internal-author filter.
- The bun publisher runs their exact command.
- verctl discovers `bunfig.toml` instead of requiring `.npmrc`.  
  That matters to them, because their repo forbids `.npmrc` outright.  
  bun gives `.npmrc` precedence and discards a committed scope-token contract without a warning.

## 1. Tags for every shipped artifact

This request blocked adoption.  
`publish::tag_for` removes duplicate versions among the released packages.  
When more than one version remains, it returns the version of the **first declared** package.  
That value has no relation to the release, and no message reports it.

With one distinct version, this is exactly right, which is why this repo has never hit it.  
With two, it tags one package and leaves consumers of the other looking for a tag that does not exist.

### Their tag and Release steps

Their tags and their Releases are two separate steps, never interleaved.

**Tags come first, on the git side.**  
They create one annotated tag per released package at an explicit SHA (`git tag -a <name>@<version> <sha>`).  
Then a single `git push --atomic origin refs/tags/a:refs/tags/a …` pushes the whole set.  
Three rules guard the push:

- Any expected tag already at another commit stops it.
- A *partial* set stops it, and the push does not complete that set.
- A full set at the right SHA is skipped, so a rerun does nothing.

Then they re-read every tag with `git ls-remote --tags origin refs/tags/<t> refs/tags/<t>^{}`.  
They take the peeled form first, so an annotated tag resolves to its commit rather than to the tag object.  
A push that exited zero does not count as proof that it landed.

**Releases come after**, one per tag that already exists.  
Each Release body is cut from that package's own changelog, between its version heading and the next.

That order matters more than it looks.  
Because the tag exists first, the Releases API documents `target_commitish` as unused.  
The question of where a Release puts a tag therefore never comes up.  
Only the **tags** need an atomic step.  
A Release is derived state that a tag and a changelog section can rebuild, and a tag is what consumers resolve.

Filed as the tag-name row [`VER-039`][ledger] and the whole-set row [`VER-038`][ledger].

`VER-039` ships `tags.template`, with the default `v{version}`.  
A `{name}` in the template gives one tag and one Release per package.  
It fills each Release body from that package's own CHANGELOG section.  
verctl refuses a template without `{name}` when versions differ, so it never guesses the first package.  
The atomic tag push on the git side stays with `VER-038`.

## 2. Publish must follow the dependency graph

`publish::plan` walks the declared packages in declaration order.  
With internal dependencies, that leaves a window where a consumer is published and its dependency is not.  
The consumer's pinned version then does not resolve.

They also ask verctl to print the resolved order.  
In their words, a silent topological sort is a thing nobody debugs until it is wrong.

Filed as the dependency-order row [`VER-033`][ledger].

## 3. Before and after steps for publish

They keep two checks that they do **not** ask verctl to absorb.  
Both are worth describing, because they cover what a tool that runs releases cannot see from the inside.

**Before publish**, on the release commit, they pack each package.  
They read the manifest back out of the tarball and assert that no dependency field still names the workspace protocol.

That check proves the package manager rewrote internal dependencies into registry versions.  
A tarball that still holds the protocol installs as garbage for every consumer, and nothing else in a pipeline notices.

They then build a throwaway consumer against the local tarballs and import every specifier in the packed `exports` map.  
That catches export maps and type declarations that resolve only inside the workspace.

**After publish**, they install each published version from the registry into a clean cache.  
They assert that the registry serves the version they pushed.  
This check has caught propagation delay, and a registry that accepted a publish and served the previous version.

Treating "already exists" as success is right for idempotency.  
It also means a retry can pass while nothing downstream was verified.

Neither check belongs in verctl, because both are specific to their stack.  
What they cannot do is wire the checks in: `prepare` has `after`, and publish has no equivalent.

Filed as the before-and-after row [`VER-034`][ledger].

## 4. The commit a release publishes from should be proved

They keep both of verctl's checks and add three:

1. The release SHA is an ancestor of the default branch.
2. The workflow files at that SHA match the default branch.
3. The first-parent branch holds exactly one version commit.

The workflow-drift check is not obvious, and it cost them a release.  
Retrying an old release SHA runs the workflow **at that SHA**.  
If permissions or steps changed since, the run can publish and then fail to create tags.  
That is the worst split, because a registry publish cannot be undone and the tags are the audit trail.

An explicit flag can turn off two of the three, and a run that turns one off prints that.

They warn about one detail of building that flag.  
Bind a dispatch input to a step-level `env:` and quote it, and never interpolate it into a `run:` string.  
Over REST, dispatch inputs arrive as strings and are not coerced to booleans, and the job holds write permissions.

Filed as the commit-proof row [`VER-035`][ledger].

## 5. A check that cannot run must not pass

Two sites break one rule.  
`publisher::expand` drops any argv part that holds an unresolved placeholder.  
A typo then shortens the command line instead of failing, and the error appears in a later step under another name.

`prove_default_history` returns success in four cases: no repo, no head, a head that cannot be peeled, and no upstream.  
Each is defensible alone.  
Together, they let the check pass by not running, in exactly the CI shapes where it matters most.

Filed as the cannot-run row [`VER-036`][ledger].

## 6. Questions the README does not answer

Four questions each cost them time:

1. What `--access` does once a registry resolves, and whether a restricted package can be published as public by accident.
2. What the token means locally, where a `gh` session is not the token.  
   The failure then lands at the Release step, *after* publishing.
3. That a `permission_denied` on write is a package-settings problem rather than a token problem.  
   For GitHub Packages, an existing package needs write permission on the workflow, and that alone is not enough.  
   The package must be linked to the publishing repository, and no public API sets that link.
4. That the tool finalizing a release is the one the workflow pins, rather than the one at the release commit.  
   Their publish job checks out the release commit, which also supplies the tagging code.  
   A retry then lacks any later fix to that code.  
   Build and publish from the release commit, and finalize with the current tooling.

Filed as the README row [`VER-037`][ledger].

## Praise worth keeping

They praised three parts, so keep them through any refactor.

`examples/ver.yaml` is fully live, and `tests/examples.rs` parses it.  
In their words, that is the best thing in the documentation.  
It is why they could answer most of their own questions by reading one file.

The `occurrences` vocabulary (`once`, `many`, `never`, `{exactly: N}`) is better than what they would have built.

verctl decides how many jobs run, and the workflow decides where a fixed job runs.  
They called that split a good line.  
Passing labels through unchanged is what let them keep their own runner labels.

[ledger]: ../tasks.yaml
